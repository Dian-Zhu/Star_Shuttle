use crate::modules::connection::DefaultConnectionManager;
use russh::Channel;
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::sync::Arc;
use tauri::State;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: u64,
    pub permissions: u32,
    pub owner: String,
    pub group: String,
}

pub struct SftpManager {
    sessions: Arc<Mutex<HashMap<Uuid, Arc<Mutex<SftpSession>>>>>,
    connection_manager: Arc<std::sync::RwLock<DefaultConnectionManager>>,
}

struct ScpChannel {
    channel: Channel<russh::client::Msg>,
    buffer: VecDeque<u8>,
}

impl ScpChannel {
    fn new(channel: Channel<russh::client::Msg>) -> Self {
        Self {
            channel,
            buffer: VecDeque::new(),
        }
    }

    async fn write_all(&mut self, data: &[u8]) -> Result<(), String> {
        self.channel.data(data).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn read_more(&mut self) -> Result<(), String> {
        while self.buffer.is_empty() {
            let msg = self.channel.wait().await.ok_or("SCP channel closed")?;
            match msg {
                russh::ChannelMsg::Data { data } => {
                    for b in data.as_ref().iter().copied() {
                        self.buffer.push_back(b);
                    }
                }
                russh::ChannelMsg::ExtendedData { data, .. } => {
                    for b in data.as_ref().iter().copied() {
                        self.buffer.push_back(b);
                    }
                }
                russh::ChannelMsg::Eof => return Err("SCP channel EOF".to_string()),
                _ => {}
            }
        }
        Ok(())
    }

    async fn read_u8(&mut self) -> Result<u8, String> {
        self.read_more().await?;
        self.buffer
            .pop_front()
            .ok_or("SCP buffer underflow".to_string())
    }

    async fn read_exact(&mut self, n: usize) -> Result<Vec<u8>, String> {
        while self.buffer.len() < n {
            self.read_more().await?;
        }
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            out.push(
                self.buffer
                    .pop_front()
                    .ok_or("SCP buffer underflow".to_string())?,
            );
        }
        Ok(out)
    }

    async fn read_line(&mut self) -> Result<String, String> {
        let mut bytes = Vec::new();
        loop {
            let b = self.read_u8().await?;
            if b == b'\n' {
                break;
            }
            bytes.push(b);
        }
        String::from_utf8(bytes).map_err(|e| e.to_string())
    }
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let mut out = String::from("'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("'\"'\"'");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

fn split_remote_path(path: &str) -> (String, String) {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return (".".to_string(), "".to_string());
    }
    if let Some(idx) = trimmed.rfind('/') {
        let (dir, rest) = trimmed.split_at(idx);
        let name = rest.trim_start_matches('/');
        let dir = if dir.is_empty() { "/" } else { dir };
        return (dir.to_string(), name.to_string());
    }
    (".".to_string(), trimmed.to_string())
}

async fn scp_read_ack(io: &mut ScpChannel) -> Result<(), String> {
    let code = io.read_u8().await?;
    match code {
        0 => Ok(()),
        1 | 2 => {
            let msg = io
                .read_line()
                .await
                .unwrap_or_else(|_| "SCP error".to_string());
            Err(msg)
        }
        other => Err(format!("SCP invalid ack: {}", other)),
    }
}

impl SftpManager {
    pub fn new(connection_manager: Arc<std::sync::RwLock<DefaultConnectionManager>>) -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
            connection_manager,
        }
    }
    // ...

    async fn get_session(&self, session_id: Uuid) -> Result<Arc<Mutex<SftpSession>>, String> {
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get(&session_id) {
            return Ok(session.clone());
        }

        // Create new
        let ssh_conn = {
            let cm = self.connection_manager.read().map_err(|e| e.to_string())?;
            cm.get_ssh_connection(&session_id)
                .ok_or("SSH session not found")?
        };

        let handle = ssh_conn.handle.lock().await;
        let channel = handle
            .channel_open_session()
            .await
            .map_err(|e| e.to_string())?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| e.to_string())?;

        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| e.to_string())?;

        let sftp_arc = Arc::new(Mutex::new(sftp));
        sessions.insert(session_id, sftp_arc.clone());

        Ok(sftp_arc)
    }

    async fn exec_ssh_command(&self, session_id: Uuid, command: String) -> Result<String, String> {
        let ssh_conn = {
            let cm = self.connection_manager.read().map_err(|e| e.to_string())?;
            cm.get_ssh_connection(&session_id)
                .ok_or("SSH session not found")?
        };

        let mut channel = {
            let handle = ssh_conn.handle.lock().await;
            handle
                .channel_open_session()
                .await
                .map_err(|e| e.to_string())?
        };

        channel
            .exec(true, command.as_bytes().to_vec())
            .await
            .map_err(|e| e.to_string())?;

        let mut output = String::new();
        while let Some(msg) = channel.wait().await {
            match msg {
                russh::ChannelMsg::Data { ref data } => {
                    output.push_str(&String::from_utf8_lossy(data));
                }
                russh::ChannelMsg::ExtendedData { ref data, .. } => {
                    output.push_str(&String::from_utf8_lossy(data));
                }
                russh::ChannelMsg::Eof => {
                    break;
                }
                _ => {}
            }
        }

        let _ = channel.close().await;
        Ok(output)
    }

    async fn open_exec_channel(
        &self,
        session_id: Uuid,
        command: String,
    ) -> Result<Channel<russh::client::Msg>, String> {
        let ssh_conn = {
            let cm = self.connection_manager.read().map_err(|e| e.to_string())?;
            cm.get_ssh_connection(&session_id)
                .ok_or("SSH session not found")?
        };

        let channel = {
            let handle = ssh_conn.handle.lock().await;
            handle
                .channel_open_session()
                .await
                .map_err(|e| e.to_string())?
        };

        channel
            .exec(true, command.as_bytes().to_vec())
            .await
            .map_err(|e| e.to_string())?;
        Ok(channel)
    }

    fn parse_id_map(output: &str) -> HashMap<u32, String> {
        let mut map = HashMap::new();
        for line in output.lines() {
            let mut parts = line.trim().splitn(2, ':');
            let Some(id_str) = parts.next() else { continue };
            let Some(name) = parts.next() else { continue };
            let Ok(id) = id_str.parse::<u32>() else {
                continue;
            };
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            map.insert(id, name.to_string());
        }
        map
    }

    async fn resolve_owner_group_maps(
        &self,
        session_id: Uuid,
        uids: &HashSet<u32>,
        gids: &HashSet<u32>,
    ) -> (HashMap<u32, String>, HashMap<u32, String>) {
        let mut uid_map = HashMap::new();
        let mut gid_map = HashMap::new();

        if !uids.is_empty() {
            let mut uid_list: Vec<u32> = uids.iter().copied().collect();
            uid_list.sort_unstable();
            let uid_args = uid_list
                .into_iter()
                .map(|u| u.to_string())
                .collect::<Vec<_>>()
                .join(" ");

            let cmd = format!(
                "getent passwd {} 2>/dev/null | awk -F: '{{print $3\":\"$1}}'",
                uid_args
            );
            if let Ok(out) = self.exec_ssh_command(session_id, cmd).await {
                uid_map = Self::parse_id_map(&out);
            }
        }

        if !gids.is_empty() {
            let mut gid_list: Vec<u32> = gids.iter().copied().collect();
            gid_list.sort_unstable();
            let gid_args = gid_list
                .into_iter()
                .map(|g| g.to_string())
                .collect::<Vec<_>>()
                .join(" ");

            let cmd = format!(
                "getent group {} 2>/dev/null | awk -F: '{{print $3\":\"$1}}'",
                gid_args
            );
            if let Ok(out) = self.exec_ssh_command(session_id, cmd).await {
                gid_map = Self::parse_id_map(&out);
            }
        }

        (uid_map, gid_map)
    }

    pub async fn list_directory(
        &self,
        session_id: Uuid,
        path: String,
    ) -> Result<Vec<FileEntry>, String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;

        let path = if path.is_empty() { "." } else { &path };

        let files = session.read_dir(path).await.map_err(|e| e.to_string())?;

        let mut uids = HashSet::new();
        let mut gids = HashSet::new();
        let mut raw_entries = Vec::new();

        for f in files {
            let meta = f.metadata();
            let uid = meta.uid;
            let gid = meta.gid;
            if let Some(u) = uid {
                uids.insert(u);
            }
            if let Some(g) = gid {
                gids.insert(g);
            }
            raw_entries.push((f, uid, gid));
        }

        drop(session);

        let (uid_map, gid_map) = self
            .resolve_owner_group_maps(session_id, &uids, &gids)
            .await;

        let entries = raw_entries
            .into_iter()
            .map(|(f, uid, gid)| {
                let attrs = f.file_type();
                let meta = f.metadata();

                let owner = uid
                    .and_then(|u| uid_map.get(&u).cloned().or(Some(u.to_string())))
                    .unwrap_or_default();
                let group = gid
                    .and_then(|g| gid_map.get(&g).cloned().or(Some(g.to_string())))
                    .unwrap_or_default();

                FileEntry {
                    name: f.file_name(),
                    is_dir: attrs.is_dir(),
                    size: meta.size.unwrap_or(0),
                    modified: meta
                        .modified()
                        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                        .duration_since(std::time::SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                    permissions: meta.permissions.unwrap_or(0),
                    owner,
                    group,
                }
            })
            .collect();

        Ok(entries)
    }

    pub async fn read_file(&self, session_id: Uuid, path: String) -> Result<Vec<u8>, String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;

        let mut file = session.open(path).await.map_err(|e| e.to_string())?;
        let mut contents = Vec::new();
        file.read_to_end(&mut contents)
            .await
            .map_err(|e| e.to_string())?;

        Ok(contents)
    }

    pub async fn write_file(
        &self,
        session_id: Uuid,
        path: String,
        content: Vec<u8>,
        append: bool,
    ) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;

        let mut file = if append {
            use russh_sftp::protocol::OpenFlags;
            session
                .open_with_flags(
                    &path,
                    OpenFlags::WRITE | OpenFlags::APPEND | OpenFlags::CREATE,
                )
                .await
                .map_err(|e| e.to_string())?
        } else {
            session.create(&path).await.map_err(|e| e.to_string())?
        };

        file.write_all(&content).await.map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn create_directory(&self, session_id: Uuid, path: String) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        session.create_dir(path).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn remove_file(&self, session_id: Uuid, path: String) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        session.remove_file(path).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn remove_directory(&self, session_id: Uuid, path: String) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        session.remove_dir(path).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn rename(
        &self,
        session_id: Uuid,
        old_path: String,
        new_path: String,
    ) -> Result<(), String> {
        let session_arc = self.get_session(session_id).await?;
        let session = session_arc.lock().await;
        session
            .rename(old_path, new_path)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn scp_upload(
        &self,
        session_id: Uuid,
        remote_path: String,
        content: Vec<u8>,
    ) -> Result<(), String> {
        let (dir, file_name) = split_remote_path(&remote_path);
        if file_name.is_empty() {
            return Err("Invalid remote path".to_string());
        }

        let command = format!("scp -t -- {}", shell_quote(&dir));
        let channel = self.open_exec_channel(session_id, command).await?;
        let mut io = ScpChannel::new(channel);

        let header = format!("C0644 {} {}\n", content.len(), file_name);
        io.write_all(header.as_bytes()).await?;
        scp_read_ack(&mut io).await?;

        if !content.is_empty() {
            io.write_all(&content).await?;
        }
        io.write_all(&[0]).await?;
        scp_read_ack(&mut io).await?;

        let _ = io.channel.close().await;
        Ok(())
    }

    pub async fn scp_download(
        &self,
        session_id: Uuid,
        remote_path: String,
    ) -> Result<Vec<u8>, String> {
        let command = format!("scp -f -- {}", shell_quote(&remote_path));
        let channel = self.open_exec_channel(session_id, command).await?;
        let mut io = ScpChannel::new(channel);

        io.write_all(&[0]).await?;

        loop {
            let b = io.read_u8().await?;
            match b {
                b'T' => {
                    let _ = io.read_line().await?;
                    io.write_all(&[0]).await?;
                }
                b'C' => {
                    let header = io.read_line().await?;
                    let mut parts = header.splitn(3, ' ');
                    let _mode = parts.next().ok_or("SCP missing mode")?;
                    let size_str = parts.next().ok_or("SCP missing size")?;
                    let _name = parts.next().ok_or("SCP missing filename")?;
                    let size = size_str.parse::<usize>().map_err(|e| e.to_string())?;

                    io.write_all(&[0]).await?;
                    let data = io.read_exact(size).await?;
                    scp_read_ack(&mut io).await?;
                    io.write_all(&[0]).await?;

                    let _ = io.channel.close().await;
                    return Ok(data);
                }
                0 => {}
                1 | 2 => {
                    let msg = io
                        .read_line()
                        .await
                        .unwrap_or_else(|_| "SCP error".to_string());
                    return Err(msg);
                }
                other => return Err(format!("SCP unexpected response byte: {}", other)),
            }
        }
    }
}

#[tauri::command]
pub async fn sftp_ls(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<Vec<FileEntry>, String> {
    state.list_directory(session_id, path).await
}

#[tauri::command]
pub async fn sftp_read(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<Vec<u8>, String> {
    state.read_file(session_id, path).await
}

#[tauri::command]
pub async fn sftp_write(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
    content: Vec<u8>,
    append: Option<bool>,
) -> Result<(), String> {
    state
        .write_file(session_id, path, content, append.unwrap_or(false))
        .await
}

#[tauri::command]
pub async fn sftp_mkdir(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<(), String> {
    state.create_directory(session_id, path).await
}

#[tauri::command]
pub async fn sftp_rm(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<(), String> {
    state.remove_file(session_id, path).await
}

#[tauri::command]
pub async fn sftp_rmdir(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<(), String> {
    state.remove_directory(session_id, path).await
}

#[tauri::command]
pub async fn sftp_rename(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    old_path: String,
    new_path: String,
) -> Result<(), String> {
    state.rename(session_id, old_path, new_path).await
}

#[tauri::command]
pub async fn scp_upload(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    remote_path: String,
    content: Vec<u8>,
) -> Result<(), String> {
    state.scp_upload(session_id, remote_path, content).await
}

#[tauri::command]
pub async fn scp_download(
    state: State<'_, SftpManager>,
    session_id: Uuid,
    remote_path: String,
) -> Result<Vec<u8>, String> {
    state.scp_download(session_id, remote_path).await
}
