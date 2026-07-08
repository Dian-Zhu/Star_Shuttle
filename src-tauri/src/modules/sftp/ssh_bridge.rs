use crate::modules::connection::DefaultConnectionManager;
use russh::client::Msg;
use russh::Channel;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

// 保留供“按需解析 owner/group 名字”复用：当前 list_directory 已改为不阻塞地
// 直接返回数字 uid/gid，这几个函数暂无调用者，但后续做异步补名时会用到。
#[allow(dead_code)]
pub(crate) async fn exec_ssh_command(
    connection_manager: &Arc<RwLock<DefaultConnectionManager>>,
    session_id: Uuid,
    command: String,
) -> Result<String, String> {
    let ssh_conn = {
        let cm = connection_manager.read().map_err(|e| e.to_string())?;
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

pub(crate) async fn open_exec_channel(
    connection_manager: &Arc<RwLock<DefaultConnectionManager>>,
    session_id: Uuid,
    command: String,
) -> Result<Channel<Msg>, String> {
    let ssh_conn = {
        let cm = connection_manager.read().map_err(|e| e.to_string())?;
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

#[allow(dead_code)]
pub(crate) fn parse_id_map(output: &str) -> HashMap<u32, String> {
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

#[allow(dead_code)]
pub(crate) async fn resolve_owner_group_maps(
    connection_manager: &Arc<RwLock<DefaultConnectionManager>>,
    session_id: Uuid,
    uids: &HashSet<u32>,
    gids: &HashSet<u32>,
) -> (HashMap<u32, String>, HashMap<u32, String>) {
    let mut uid_map = HashMap::new();
    let mut gid_map = HashMap::new();

    if !uids.is_empty() {
        let mut uid_list: Vec<u32> = uids.iter().copied().collect();
        uid_list.sort_unstable();
        // Limit batch size to prevent excessively long commands
        uid_list.truncate(200);
        let uid_args = uid_list
            .into_iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let cmd = format!(
            "getent passwd -- {} 2>/dev/null | awk -F: '{{print $3\":\"$1}}'",
            uid_args
        );
        if let Ok(out) = exec_ssh_command(connection_manager, session_id, cmd).await {
            uid_map = parse_id_map(&out);
        }
    }

    if !gids.is_empty() {
        let mut gid_list: Vec<u32> = gids.iter().copied().collect();
        gid_list.sort_unstable();
        gid_list.truncate(200);
        let gid_args = gid_list
            .into_iter()
            .map(|g| g.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        let cmd = format!(
            "getent group -- {} 2>/dev/null | awk -F: '{{print $3\":\"$1}}'",
            gid_args
        );
        if let Ok(out) = exec_ssh_command(connection_manager, session_id, cmd).await {
            gid_map = parse_id_map(&out);
        }
    }

    (uid_map, gid_map)
}
