use super::{LocalFileHandle, LocalFsState};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub(super) fn insert_handle(
    state: &LocalFsState,
    handle: LocalFileHandle,
) -> Result<String, String> {
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
    let handle_arc = {
        // Only hold the global map lock for lookup to avoid blocking unrelated handles.
        let handles = state.handles.lock().map_err(|e| e.to_string())?;
        handles
            .get(handle_id)
            .cloned()
            .ok_or_else(|| format!("Unknown local file handle: {}", handle_id))?
    };

    let mut handle = handle_arc.lock().map_err(|e| e.to_string())?;
    f(&mut handle)
}

pub(super) fn close_handle(state: &LocalFsState, handle_id: &str) -> Result<(), String> {
    let mut handles = state.handles.lock().map_err(|e| e.to_string())?;
    handles
        .remove(handle_id)
        .ok_or_else(|| format!("Unknown local file handle: {}", handle_id))?;
    Ok(())
}
