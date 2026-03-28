use super::{LocalFileHandle, LocalFsState, LOCAL_FS_HANDLE_IDLE_TTL_SECONDS};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub(super) fn insert_handle(
    state: &LocalFsState,
    handle: LocalFileHandle,
) -> Result<String, String> {
    cleanup_expired_handles(state)?;
    let handle_id = Uuid::new_v4().to_string();
    let mut handles = state.handles.lock().map_err(|e| e.to_string())?;
    handles.insert(handle_id.clone(), Arc::new(Mutex::new(handle)));
    Ok(handle_id)
}

pub(super) fn with_handle<R>(
    state: &LocalFsState,
    handle_id: &str,
    f: impl FnOnce(&mut LocalFileHandle) -> Result<R, String>,
) -> Result<R, String> {
    cleanup_expired_handles(state)?;
    let handle_arc = {
        // Only hold the global map lock for lookup to avoid blocking unrelated handles.
        let handles = state.handles.lock().map_err(|e| e.to_string())?;
        handles
            .get(handle_id)
            .cloned()
            .ok_or_else(|| format!("Unknown local file handle: {}", handle_id))?
    };

    let mut handle = handle_arc.lock().map_err(|e| e.to_string())?;
    handle.last_touched = Instant::now();
    f(&mut handle)
}

pub(super) fn close_handle(state: &LocalFsState, handle_id: &str) -> Result<(), String> {
    cleanup_expired_handles(state)?;
    let mut handles = state.handles.lock().map_err(|e| e.to_string())?;
    handles
        .remove(handle_id)
        .ok_or_else(|| format!("Unknown local file handle: {}", handle_id))?;
    Ok(())
}

pub(super) fn cleanup_expired_handles(state: &LocalFsState) -> Result<(), String> {
    let now = Instant::now();
    let ttl = Duration::from_secs(LOCAL_FS_HANDLE_IDLE_TTL_SECONDS);
    let mut handles = state.handles.lock().map_err(|e| e.to_string())?;
    handles.retain(|_, handle| {
        let Ok(handle) = handle.lock() else {
            // Keep poisoned handles to avoid accidental data loss on lock poisoning.
            return true;
        };
        now.duration_since(handle.last_touched) <= ttl
    });
    Ok(())
}
