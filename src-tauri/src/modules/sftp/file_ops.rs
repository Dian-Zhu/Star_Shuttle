use russh_sftp::protocol::OpenFlags;
use std::{
    io::SeekFrom,
    sync::{Arc, RwLock},
};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use uuid::Uuid;

use super::common::{
    ensure_max_bytes, ensure_sftp_write_size, MAX_SFTP_READ_BYTES, STREAM_READ_CHUNK_SIZE,
};
use super::generation::SftpSessionLease;
use super::FileEntry;
use crate::modules::connection::DefaultConnectionManager;

pub async fn list_directory(
    session_lease: SftpSessionLease,
    _connection_manager: &Arc<RwLock<DefaultConnectionManager>>,
    _session_id: Uuid,
    path: String,
) -> Result<Vec<FileEntry>, String> {
    session_lease.ensure_valid()?;
    let session = session_lease.lock().await;
    session_lease.ensure_valid()?;

    let path = if path.is_empty() { "." } else { &path };

    let files = session_lease.finish_io(session.read_dir(path).await.map_err(|e| e.to_string()))?;

    drop(session);

    // owner/group 先以数字 uid/gid 填充，立即返回目录列表。
    // 过去这里会同步等待 `getent` 通过额外 SSH 通道把 uid/gid 翻译成名字，
    // 每次 list 目录都串行阻塞最多 2 次 SSH 往返，是初始加载慢的主因；
    // 而前端列表并不展示 owner/group，因此改为不阻塞、按需再解析（见 ssh_bridge::resolve_owner_group_maps）。
    let entries = files
        .into_iter()
        .map(|f| {
            let attrs = f.file_type();
            let meta = f.metadata();

            let owner = meta.uid.map(|u| u.to_string()).unwrap_or_default();
            let group = meta.gid.map(|g| g.to_string()).unwrap_or_default();

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

pub async fn read_file(session_lease: SftpSessionLease, path: String) -> Result<Vec<u8>, String> {
    session_lease.ensure_valid()?;
    let session = session_lease.lock().await;
    session_lease.ensure_valid()?;

    let mut file = session_lease.finish_io(session.open(path).await.map_err(|e| e.to_string()))?;
    let mut contents = Vec::new();
    let mut buf = vec![0u8; STREAM_READ_CHUNK_SIZE];

    loop {
        let n = session_lease.finish_io(file.read(&mut buf).await.map_err(|e| e.to_string()))?;
        if n == 0 {
            break;
        }

        ensure_max_bytes(
            contents.len().saturating_add(n),
            MAX_SFTP_READ_BYTES,
            "SFTP read",
        )?;
        contents.extend_from_slice(&buf[..n]);
    }

    Ok(contents)
}

pub async fn write_file(
    session_lease: SftpSessionLease,
    path: String,
    content: Vec<u8>,
    append: bool,
    offset: Option<u64>,
    truncate: bool,
) -> Result<(), String> {
    ensure_sftp_write_size(content.len(), "SFTP write")?;
    session_lease.ensure_valid()?;
    let session = session_lease.lock().await;
    session_lease.ensure_valid()?;

    let mut file = if let Some(offset) = offset {
        let mut file = if truncate && offset == 0 {
            session_lease.finish_io(session.create(&path).await.map_err(|e| e.to_string()))?
        } else {
            session_lease.finish_io(
                session
                    .open_with_flags(&path, OpenFlags::WRITE | OpenFlags::CREATE)
                    .await
                    .map_err(|e| e.to_string()),
            )?
        };
        if offset > 0 {
            session_lease.finish_io(
                file.seek(SeekFrom::Start(offset))
                    .await
                    .map_err(|e| format!("Failed to seek remote file to {}: {}", offset, e)),
            )?;
        }
        file
    } else if append {
        session_lease.finish_io(
            session
                .open_with_flags(
                    &path,
                    OpenFlags::WRITE | OpenFlags::APPEND | OpenFlags::CREATE,
                )
                .await
                .map_err(|e| e.to_string()),
        )?
    } else {
        session_lease.finish_io(session.create(&path).await.map_err(|e| e.to_string()))?
    };

    session_lease.finish_io(file.write_all(&content).await.map_err(|e| e.to_string()))?;

    Ok(())
}

pub async fn create_directory(session_lease: SftpSessionLease, path: String) -> Result<(), String> {
    session_lease.ensure_valid()?;
    let session = session_lease.lock().await;
    session_lease.ensure_valid()?;
    session_lease.finish_io(session.create_dir(path).await.map_err(|e| e.to_string()))?;
    Ok(())
}

pub async fn remove_file(session_lease: SftpSessionLease, path: String) -> Result<(), String> {
    session_lease.ensure_valid()?;
    let session = session_lease.lock().await;
    session_lease.ensure_valid()?;
    session_lease.finish_io(session.remove_file(path).await.map_err(|e| e.to_string()))?;
    Ok(())
}

pub async fn remove_directory(session_lease: SftpSessionLease, path: String) -> Result<(), String> {
    session_lease.ensure_valid()?;
    let session = session_lease.lock().await;
    session_lease.ensure_valid()?;
    session_lease.finish_io(session.remove_dir(path).await.map_err(|e| e.to_string()))?;
    Ok(())
}

pub async fn rename(
    session_lease: SftpSessionLease,
    old_path: String,
    new_path: String,
) -> Result<(), String> {
    session_lease.ensure_valid()?;
    let session = session_lease.lock().await;
    session_lease.ensure_valid()?;
    session_lease.finish_io(
        session
            .rename(old_path, new_path)
            .await
            .map_err(|e| e.to_string()),
    )?;
    Ok(())
}
