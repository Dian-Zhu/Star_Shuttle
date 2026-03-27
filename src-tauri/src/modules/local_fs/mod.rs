use crate::modules::db::DatabaseManager;
use crate::{ensure_app_unlocked_runtime, AppLockRuntimeState};
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::State;
use uuid::Uuid;

pub struct LocalFsState {
    handles: Mutex<HashMap<String, LocalFileHandle>>,
    allowed_roots: Vec<PathBuf>,
}

struct LocalFileHandle {
    file: File,
    writable: bool,
    size: u64,
}

const MAX_LOCAL_FS_CHUNK_BYTES: usize = 4 * 1024 * 1024;

#[derive(Serialize)]
pub struct LocalOpenHandle {
    pub handle_id: String,
    pub size: u64,
}

#[derive(Serialize)]
pub struct LocalFileStat {
    pub size: u64,
}

fn normalize_path(path: &str) -> PathBuf {
    PathBuf::from(path)
}

fn default_allowed_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for candidate in [
        dirs::home_dir(),
        dirs::desktop_dir(),
        dirs::download_dir(),
        dirs::document_dir(),
        Some(std::env::temp_dir()),
    ]
    .into_iter()
    .flatten()
    {
        let canonical = fs::canonicalize(&candidate).unwrap_or(candidate);
        if !roots.iter().any(|root| root == &canonical) {
            roots.push(canonical);
        }
    }
    roots
}

impl Default for LocalFsState {
    fn default() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: default_allowed_roots(),
        }
    }
}

fn path_access_anchor(path: &Path) -> Result<PathBuf, String> {
    let mut current = Some(path);
    while let Some(candidate) = current {
        if candidate.exists() {
            return fs::canonicalize(candidate).map_err(|e| e.to_string());
        }
        current = candidate.parent();
    }

    Err(format!(
        "Path is outside allowed roots or has no existing parent: {}",
        path.display()
    ))
}

fn ensure_path_allowed(state: &LocalFsState, path: &Path) -> Result<(), String> {
    let anchor = path_access_anchor(path)?;
    if state
        .allowed_roots
        .iter()
        .any(|root| anchor.starts_with(root))
    {
        return Ok(());
    }

    Err(format!(
        "Access to local path is denied: {}",
        path.to_string_lossy()
    ))
}

fn ensure_unlocked(
    db: &State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: &State<'_, Arc<Mutex<AppLockRuntimeState>>>,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())
}

fn ensure_chunk_limit(length: usize, label: &str) -> Result<(), String> {
    if length > MAX_LOCAL_FS_CHUNK_BYTES {
        return Err(format!(
            "{} exceeds limit of {} bytes",
            label, MAX_LOCAL_FS_CHUNK_BYTES
        ));
    }
    Ok(())
}

fn insert_handle(state: &LocalFsState, handle: LocalFileHandle) -> Result<String, String> {
    let handle_id = Uuid::new_v4().to_string();
    let mut handles = state.handles.lock().map_err(|e| e.to_string())?;
    handles.insert(handle_id.clone(), handle);
    Ok(handle_id)
}

fn with_handle<R>(
    state: &LocalFsState,
    handle_id: &str,
    f: impl FnOnce(&mut LocalFileHandle) -> Result<R, String>,
) -> Result<R, String> {
    let mut handles = state.handles.lock().map_err(|e| e.to_string())?;
    let handle = handles
        .get_mut(handle_id)
        .ok_or_else(|| format!("Unknown local file handle: {}", handle_id))?;
    f(handle)
}

#[tauri::command]
pub fn local_fs_open_read(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    path: String,
) -> Result<LocalOpenHandle, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    let path = normalize_path(&path);
    ensure_path_allowed(&state, &path)?;
    let file = File::open(&path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let handle_id = insert_handle(
        &state,
        LocalFileHandle {
            file,
            writable: false,
            size,
        },
    )?;
    Ok(LocalOpenHandle { handle_id, size })
}

#[tauri::command]
pub fn local_fs_stat(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    path: String,
) -> Result<LocalFileStat, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    let path = normalize_path(&path);
    ensure_path_allowed(&state, &path)?;
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    Ok(LocalFileStat {
        size: metadata.len(),
    })
}

#[tauri::command]
pub fn local_fs_read_chunk(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    handle_id: String,
    length: usize,
) -> Result<Vec<u8>, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    ensure_chunk_limit(length, "Local FS read chunk")?;
    with_handle(&state, &handle_id, |handle| {
        let mut buffer = vec![0u8; length];
        let bytes_read = handle.file.read(&mut buffer).map_err(|e| e.to_string())?;
        buffer.truncate(bytes_read);
        Ok(buffer)
    })
}

#[tauri::command]
pub fn local_fs_open_write(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    path: String,
    truncate: bool,
) -> Result<String, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    let path = normalize_path(&path);
    ensure_path_allowed(&state, &path)?;
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(truncate)
        .open(&path)
        .map_err(|e| e.to_string())?;
    let handle_id = insert_handle(
        &state,
        LocalFileHandle {
            file,
            writable: true,
            size: 0,
        },
    )?;
    Ok(handle_id)
}

#[tauri::command]
pub fn local_fs_write_chunk(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    handle_id: String,
    content: Vec<u8>,
) -> Result<usize, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    ensure_chunk_limit(content.len(), "Local FS write chunk")?;
    with_handle(&state, &handle_id, |handle| {
        if !handle.writable {
            return Err("Handle is not writable".to_string());
        }
        handle.file.write_all(&content).map_err(|e| e.to_string())?;
        let pos = handle.file.stream_position().map_err(|e| e.to_string())?;
        handle.size = handle.size.max(pos);
        Ok(content.len())
    })
}

#[tauri::command]
pub fn local_fs_seek(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    handle_id: String,
    offset: i64,
    whence: u8,
) -> Result<u64, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    with_handle(&state, &handle_id, |handle| {
        let seek_from = match whence {
            0 => {
                if offset < 0 {
                    return Err("Negative offset is invalid for SeekFrom::Start".to_string());
                }
                SeekFrom::Start(offset as u64)
            }
            1 => SeekFrom::Current(offset),
            2 => SeekFrom::End(offset),
            _ => return Err(format!("Unsupported whence value: {}", whence)),
        };
        handle.file.seek(seek_from).map_err(|e| e.to_string())
    })
}

#[tauri::command]
pub fn local_fs_close(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    handle_id: String,
) -> Result<(), String> {
    ensure_unlocked(&db, &app_lock_state)?;
    let mut handles = state.handles.lock().map_err(|e| e.to_string())?;
    handles
        .remove(&handle_id)
        .ok_or_else(|| format!("Unknown local file handle: {}", handle_id))?;
    Ok(())
}

#[tauri::command]
pub fn local_fs_read_text(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    path: String,
) -> Result<String, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    let path = normalize_path(&path);
    ensure_path_allowed(&state, &path)?;
    fs::read_to_string(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn local_fs_write_text(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    path: String,
    content: String,
) -> Result<(), String> {
    ensure_unlocked(&db, &app_lock_state)?;
    ensure_chunk_limit(content.len(), "Local FS text write")?;
    let path = normalize_path(&path);
    ensure_path_allowed(&state, &path)?;
    fs::write(path, content).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn file_permissions_string(metadata: &fs::Metadata) -> String {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            return format!("{:o}", metadata.permissions().mode() & 0o7777);
        }

        #[cfg(not(unix))]
        {
            let _ = metadata;
            "0".to_string()
        }
    }

    #[test]
    fn permissions_string_is_non_empty() {
        let file = tempfile_path("perm-test");
        fs::write(&file, "x").expect("failed to create file");
        let metadata = fs::metadata(&file).expect("failed to stat file");
        let permissions = file_permissions_string(&metadata);
        assert!(!permissions.is_empty());
        let _ = fs::remove_file(file);
    }

    #[test]
    fn handle_insert_and_close_round_trip() {
        let state = LocalFsState::default();
        let file = tempfile_path("handle-test");
        fs::write(&file, "hello").expect("failed to create file");
        let file_handle = File::open(&file).expect("failed to open file");
        let handle_id = insert_handle(
            &state,
            LocalFileHandle {
                file: file_handle,
                writable: false,
                size: 5,
            },
        )
        .expect("failed to insert handle");
        let mut handles = state.handles.lock().expect("failed to lock handles");
        assert!(handles.remove(&handle_id).is_some());
        let _ = fs::remove_file(file);
    }

    #[test]
    fn chunk_limit_rejects_oversized_requests() {
        let err = ensure_chunk_limit(MAX_LOCAL_FS_CHUNK_BYTES + 1, "test chunk")
            .expect_err("oversized chunk should be rejected");
        assert!(err.contains("exceeds limit"));
    }

    #[test]
    fn write_all_persists_complete_chunk() {
        let file = tempfile_path("write-all-test");
        let mut handle = LocalFileHandle {
            file: OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&file)
                .expect("failed to open file"),
            writable: true,
            size: 0,
        };

        let content = b"hello world";
        handle
            .file
            .write_all(content)
            .expect("failed to write all bytes");
        handle.size = handle
            .file
            .stream_position()
            .expect("failed to read position");

        assert_eq!(handle.size, content.len() as u64);
        assert_eq!(fs::read(&file).expect("failed to read file"), content);
        let _ = fs::remove_file(file);
    }

    #[test]
    fn allowed_root_check_rejects_outside_path() {
        let allowed_root = tempfile_path("allowed-root");
        fs::create_dir_all(&allowed_root).expect("failed to create allowed root");
        let blocked_root = tempfile_path("blocked-root");
        fs::create_dir_all(&blocked_root).expect("failed to create blocked root");
        let blocked_file = blocked_root.join("file.txt");

        let state = LocalFsState {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: vec![allowed_root.clone()],
        };

        let err = ensure_path_allowed(&state, &blocked_file)
            .expect_err("path outside allowed roots should be rejected");
        assert!(err.contains("denied"));

        let _ = fs::remove_dir_all(allowed_root);
        let _ = fs::remove_dir_all(blocked_root);
    }

    #[test]
    fn allowed_root_check_accepts_inside_path() {
        let allowed_root = tempfile_path("allowed-ok");
        let nested = allowed_root.join("nested");
        fs::create_dir_all(&nested).expect("failed to create nested root");
        let file = nested.join("file.txt");

        let state = LocalFsState {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: vec![fs::canonicalize(&allowed_root).expect("canonical root")],
        };

        assert!(ensure_path_allowed(&state, &file).is_ok());

        let _ = fs::remove_dir_all(allowed_root);
    }

    fn tempfile_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("star-shuttle-{}-{}", name, Uuid::new_v4()))
    }
}
