use crate::modules::db::DatabaseManager;
use crate::{ensure_app_unlocked_runtime, AppLockRuntimeState};
use log::{debug, warn};
use russh_sftp::protocol::OpenFlags;
use std::sync::{Arc, Mutex as StdMutex};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::State;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use uuid::Uuid;

use super::common::{
    ensure_max_bytes, ensure_scp_upload_size, ensure_sftp_write_size, MAX_SFTP_CHUNK_BYTES,
};
use super::{FileEntry, SftpManager};

fn classify_sftp_error(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("not found") || lower.contains("no such file") {
        "not_found"
    } else if lower.contains("permission denied") || lower.contains("denied") {
        "permission_denied"
    } else if lower.contains("timeout") || lower.contains("timed out") {
        "timeout"
    } else if lower.contains("disconnect") || lower.contains("connection") {
        "connection_error"
    } else {
        "operation_failed"
    }
}

#[tauri::command]
pub async fn sftp_ls(
    db: State<'_, Arc<StdMutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<StdMutex<AppLockRuntimeState>>>,
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<Vec<FileEntry>, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    debug!(
        "SFTP ls requested: session={}, path_len={}",
        session_id,
        path.len()
    );
    match state.list_directory(session_id, path).await {
        Ok(entries) => {
            debug!(
                "SFTP ls succeeded: session={}, entries={}",
                session_id,
                entries.len()
            );
            Ok(entries)
        }
        Err(e) => {
            warn!(
                "SFTP ls failed: session={}, error_class={}, error_len={}",
                session_id,
                classify_sftp_error(&e),
                e.len()
            );
            Err(e)
        }
    }
}

fn header_string(request: &Request, key: &str) -> Result<String, String> {
    request
        .headers()
        .get(key)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
        .ok_or_else(|| format!("Missing header: {}", key))
}

fn header_uuid(request: &Request) -> Result<Uuid, String> {
    let value = header_string(request, "session-id")?;
    Uuid::parse_str(&value).map_err(|e| e.to_string())
}

fn body_bytes(request: &Request<'_>) -> Result<Vec<u8>, String> {
    match request.body() {
        InvokeBody::Raw(bytes) => Ok(bytes.clone()),
        InvokeBody::Json(value) => {
            serde_json::from_value::<Vec<u8>>(value.clone()).map_err(|e| e.to_string())
        }
    }
}

fn header_u64(request: &Request, key: &str) -> Result<u64, String> {
    let s = header_string(request, key)?;
    s.parse::<u64>().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn sftp_read(
    db: State<'_, Arc<StdMutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<StdMutex<AppLockRuntimeState>>>,
    state: State<'_, SftpManager>,
    request: Request<'_>,
) -> Result<Response, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    let session_id = header_uuid(&request)?;
    let path = header_string(&request, "path")?;
    let data = state.read_file(session_id, path).await?;
    Ok(Response::new(data))
}

#[tauri::command]
pub async fn sftp_read_chunk(
    db: State<'_, Arc<StdMutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<StdMutex<AppLockRuntimeState>>>,
    state: State<'_, SftpManager>,
    request: Request<'_>,
) -> Result<Response, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    let session_id = header_uuid(&request)?;
    let path = header_string(&request, "path")?;
    let offset = header_u64(&request, "offset")?;
    let length = header_u64(&request, "length")?;
    if length > usize::MAX as u64 {
        return Err("Requested chunk length exceeds system address space".to_string());
    }
    let length = length as usize;
    ensure_max_bytes(length, MAX_SFTP_CHUNK_BYTES, "SFTP chunk read")?;

    let session_lease = state.get_session(session_id).await?;
    session_lease.ensure_valid()?;
    let session = session_lease.lock().await;
    session_lease.ensure_valid()?;
    let mut file = session_lease.finish_io(
        session
            .open_with_flags(&path, OpenFlags::READ)
            .await
            .map_err(|e| e.to_string()),
    )?;

    if offset > 0 {
        session_lease.finish_io(
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|e| format!("Failed to seek remote file to {}: {}", offset, e)),
        )?;
    }

    let mut buf = vec![0u8; length];
    let n = session_lease.finish_io(file.read(&mut buf).await.map_err(|e| e.to_string()))?;
    buf.truncate(n);
    Ok(Response::new(buf))
}

#[tauri::command]
pub async fn sftp_write(
    db: State<'_, Arc<StdMutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<StdMutex<AppLockRuntimeState>>>,
    state: State<'_, SftpManager>,
    request: Request<'_>,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    let session_id = header_uuid(&request)?;
    let path = header_string(&request, "path")?;
    let offset = request
        .headers()
        .get("offset")
        .and_then(|value: &tauri::http::HeaderValue| value.to_str().ok())
        .and_then(|value: &str| value.parse::<u64>().ok());
    let truncate = request
        .headers()
        .get("truncate")
        .and_then(|value: &tauri::http::HeaderValue| value.to_str().ok())
        .and_then(|value: &str| value.parse::<bool>().ok())
        .unwrap_or(false);
    let append = request
        .headers()
        .get("append")
        .and_then(|value: &tauri::http::HeaderValue| value.to_str().ok())
        .and_then(|value: &str| value.parse::<bool>().ok())
        .unwrap_or(false);
    let content = body_bytes(&request)?;
    ensure_sftp_write_size(content.len(), "SFTP write body")?;
    state
        .write_file(session_id, path, content, append, offset, truncate)
        .await
}

#[tauri::command]
pub async fn sftp_mkdir(
    db: State<'_, Arc<StdMutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<StdMutex<AppLockRuntimeState>>>,
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    state.create_directory(session_id, path).await
}

#[tauri::command]
pub async fn sftp_rm(
    db: State<'_, Arc<StdMutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<StdMutex<AppLockRuntimeState>>>,
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    state.remove_file(session_id, path).await
}

#[tauri::command]
pub async fn sftp_rmdir(
    db: State<'_, Arc<StdMutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<StdMutex<AppLockRuntimeState>>>,
    state: State<'_, SftpManager>,
    session_id: Uuid,
    path: String,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    state.remove_directory(session_id, path).await
}

#[tauri::command]
pub async fn sftp_rename(
    db: State<'_, Arc<StdMutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<StdMutex<AppLockRuntimeState>>>,
    state: State<'_, SftpManager>,
    session_id: Uuid,
    old_path: String,
    new_path: String,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    state.rename(session_id, old_path, new_path).await
}

#[tauri::command]
pub async fn scp_upload(
    db: State<'_, Arc<StdMutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<StdMutex<AppLockRuntimeState>>>,
    state: State<'_, SftpManager>,
    request: Request<'_>,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    let session_id = header_uuid(&request)?;
    let remote_path = header_string(&request, "remote-path")?;
    let content = body_bytes(&request)?;
    ensure_scp_upload_size(content.len(), "SCP upload body")?;
    state.scp_upload(session_id, remote_path, content).await
}

#[tauri::command]
pub async fn scp_download(
    db: State<'_, Arc<StdMutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<StdMutex<AppLockRuntimeState>>>,
    state: State<'_, SftpManager>,
    request: Request<'_>,
) -> Result<Response, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    let session_id = header_uuid(&request)?;
    let remote_path = header_string(&request, "remote-path")?;
    let data = state.scp_download(session_id, remote_path).await?;
    Ok(Response::new(data))
}
