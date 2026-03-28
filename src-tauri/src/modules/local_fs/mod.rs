use crate::modules::db::DatabaseManager;
use crate::{ensure_app_unlocked_runtime, AppLockRuntimeState};
mod grants;
mod handles;

pub(crate) use self::grants::issue_dialog_path_grant;
use self::grants::{
    authorize_path, default_allowed_roots, normalize_path, stat_with_optional_access, AccessMode,
    PathGrant,
};
#[cfg(test)]
use self::grants::{ensure_path_in_allowed_roots, issue_path_grant, PathGrantSource};
use self::handles::{close_handle, insert_handle, with_handle};
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::State;

pub struct LocalFsState {
    handles: Mutex<HashMap<String, Arc<Mutex<LocalFileHandle>>>>,
    allowed_roots: Vec<PathBuf>,
    path_grants: Mutex<HashMap<String, PathGrant>>,
}

struct LocalFileHandle {
    file: File,
    writable: bool,
    size: u64,
    last_touched: Instant,
}

const MAX_LOCAL_FS_CHUNK_BYTES: usize = 4 * 1024 * 1024;
const MAX_LOCAL_FS_TEXT_BYTES: u64 = 8 * 1024 * 1024;
const PATH_GRANT_TTL_SECONDS: u64 = 180;
const LOCAL_FS_HANDLE_IDLE_TTL_SECONDS: u64 = 300;
const LOCAL_FS_TEXT_TOO_LARGE_ERR: &str = "LOCAL_FS_TEXT_TOO_LARGE";

#[derive(Serialize)]
pub struct LocalOpenHandle {
    pub handle_id: String,
    pub size: u64,
}

#[derive(Serialize)]
pub struct LocalFileStat {
    pub size: u64,
    pub access_token: Option<String>,
}

impl Default for LocalFsState {
    fn default() -> Self {
        Self {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: default_allowed_roots(),
            path_grants: Mutex::new(HashMap::new()),
        }
    }
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

fn ensure_text_limit(size_bytes: u64) -> Result<(), String> {
    if size_bytes > MAX_LOCAL_FS_TEXT_BYTES {
        return Err(format!(
            "{}|{}",
            LOCAL_FS_TEXT_TOO_LARGE_ERR,
            json!({
                "size_bytes": size_bytes,
                "max_bytes": MAX_LOCAL_FS_TEXT_BYTES
            })
            .to_string()
        ));
    }
    Ok(())
}

fn open_authorized_path_for_write(path: &Path, truncate: bool) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(truncate);
    apply_no_follow_flag(&mut options);
    options.open(path).map_err(|e| e.to_string())
}

fn open_authorized_path_for_read(path: &Path) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    apply_no_follow_flag(&mut options);
    options.open(path).map_err(|e| e.to_string())
}

#[cfg(unix)]
fn apply_no_follow_flag(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(unix_o_nofollow_flag());
}

#[cfg(windows)]
fn apply_no_follow_flag(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;

    // Open the reparse point itself instead of transparently following it.
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
}

#[cfg(not(any(unix, windows)))]
fn apply_no_follow_flag(_options: &mut OpenOptions) {}

#[cfg(unix)]
fn unix_o_nofollow_flag() -> i32 {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        // linux/include/uapi/asm-generic/fcntl.h
        return 0o400000;
    }
    #[cfg(any(
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    ))]
    {
        // BSD/macOS fcntl.h
        return 0x00000100;
    }
    #[cfg(not(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly"
    )))]
    {
        // Keep behavior for unknown Unix targets.
        0
    }
}

#[tauri::command]
pub fn local_fs_open_read(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    path: String,
    access_token: Option<String>,
) -> Result<LocalOpenHandle, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    let path = normalize_path(&path);
    let authorized_path = authorize_path(&state, &path, AccessMode::Read, access_token.as_deref())?;
    let file = open_authorized_path_for_read(&authorized_path)?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    let handle_id = insert_handle(
        &state,
        LocalFileHandle {
            file,
            writable: false,
            size,
            last_touched: Instant::now(),
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
    access_mode: Option<String>,
) -> Result<LocalFileStat, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    let path = normalize_path(&path);
    stat_with_optional_access(&state, &path, access_mode)
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
    access_token: Option<String>,
) -> Result<String, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    let path = normalize_path(&path);
    let authorized_path =
        authorize_path(&state, &path, AccessMode::Write, access_token.as_deref())?;
    let file = open_authorized_path_for_write(&authorized_path, truncate)?;
    let handle_id = insert_handle(
        &state,
        LocalFileHandle {
            file,
            writable: true,
            size: 0,
            last_touched: Instant::now(),
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
    close_handle(&state, &handle_id)
}

#[tauri::command]
pub fn local_fs_read_text(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    path: String,
    access_token: Option<String>,
) -> Result<String, String> {
    ensure_unlocked(&db, &app_lock_state)?;
    let path = normalize_path(&path);
    let authorized_path = authorize_path(&state, &path, AccessMode::Read, access_token.as_deref())?;

    // Prevent huge or unbounded streams from being loaded into memory in one shot.
    // Read at most MAX+1 bytes, then error if the file is larger than the limit.
    let file = open_authorized_path_for_read(&authorized_path)?;
    let mut limited = file.take(MAX_LOCAL_FS_TEXT_BYTES + 1);
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    ensure_text_limit(bytes.len() as u64)?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn local_fs_write_text(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, LocalFsState>,
    path: String,
    content: String,
    access_token: Option<String>,
) -> Result<(), String> {
    ensure_unlocked(&db, &app_lock_state)?;
    ensure_chunk_limit(content.len(), "Local FS text write")?;
    let path = normalize_path(&path);
    let authorized_path =
        authorize_path(&state, &path, AccessMode::Write, access_token.as_deref())?;
    let mut file = open_authorized_path_for_write(&authorized_path, true)?;
    file.write_all(content.as_bytes())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use uuid::Uuid;

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
                last_touched: Instant::now(),
            },
        )
        .expect("failed to insert handle");
        close_handle(&state, &handle_id).expect("failed to close handle");
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
            last_touched: Instant::now(),
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
            path_grants: Mutex::new(HashMap::new()),
        };

        let err = ensure_path_in_allowed_roots(&state, &blocked_file)
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
            path_grants: Mutex::new(HashMap::new()),
        };

        assert!(ensure_path_in_allowed_roots(&state, &file).is_ok());

        let _ = fs::remove_dir_all(allowed_root);
    }

    #[test]
    fn unauthorized_path_is_rejected_without_token() {
        let outside_root = tempfile_path("outside");
        fs::create_dir_all(&outside_root).expect("failed to create outside root");
        let blocked_file = outside_root.join("blocked.txt");

        let state = LocalFsState {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: vec![],
            path_grants: Mutex::new(HashMap::new()),
        };

        let err = authorize_path(&state, &blocked_file, AccessMode::Write, None)
            .expect_err("path should be denied without token");
        assert!(err.contains("denied"));

        let _ = fs::remove_dir_all(outside_root);
    }

    #[test]
    fn issued_token_allows_exact_path_once() {
        let outside_root = tempfile_path("outside-token");
        fs::create_dir_all(&outside_root).expect("failed to create outside root");
        let target_file = outside_root.join("allowed.txt");

        let state = LocalFsState {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: vec![],
            path_grants: Mutex::new(HashMap::new()),
        };

        let read_only_token = issue_path_grant(
            &state,
            &target_file,
            AccessMode::Read,
            PathGrantSource::TrustedDialog,
        )
        .expect("grant should issue");
        let err = authorize_path(
            &state,
            &target_file,
            AccessMode::Write,
            Some(&read_only_token),
        )
        .expect_err("read-only token must not allow write access");
        assert!(err.contains("denied"));

        let write_token = issue_path_grant(
            &state,
            &target_file,
            AccessMode::Write,
            PathGrantSource::TrustedDialog,
        )
        .expect("grant should issue");
        authorize_path(&state, &target_file, AccessMode::Write, Some(&write_token))
            .expect("token should allow first access");

        let err = authorize_path(&state, &target_file, AccessMode::Write, Some(&write_token))
            .expect_err("token should be consumed after first use");
        assert!(err.contains("denied"));

        let _ = fs::remove_dir_all(outside_root);
    }

    #[test]
    fn allowed_roots_token_cannot_be_issued_for_outside_path() {
        let outside_root = tempfile_path("outside-allowed-only");
        fs::create_dir_all(&outside_root).expect("failed to create outside root");
        let target_file = outside_root.join("blocked.txt");

        let state = LocalFsState {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: vec![],
            path_grants: Mutex::new(HashMap::new()),
        };

        let result = issue_path_grant(
            &state,
            &target_file,
            AccessMode::Write,
            PathGrantSource::AllowedRootsOnly,
        );
        assert!(result.is_err());

        let _ = fs::remove_dir_all(outside_root);
    }

    #[test]
    fn default_state_has_no_ambient_allowed_roots() {
        let state = LocalFsState::default();
        assert!(state.allowed_roots.is_empty());
    }

    #[test]
    fn authorize_path_returns_resolved_target_path() {
        let outside_root = tempfile_path("outside-symlink");
        let target_root = tempfile_path("target-root");
        fs::create_dir_all(&outside_root).expect("failed to create outside root");
        fs::create_dir_all(&target_root).expect("failed to create target root");

        let target_file = target_root.join("real.txt");
        fs::write(&target_file, "hello").expect("failed to write target file");
        let link_path = outside_root.join("link.txt");

        #[cfg(unix)]
        std::os::unix::fs::symlink(&target_file, &link_path).expect("failed to create symlink");
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&target_file, &link_path)
            .expect("failed to create symlink");

        let state = LocalFsState {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: vec![],
            path_grants: Mutex::new(HashMap::new()),
        };

        let token = issue_path_grant(
            &state,
            &link_path,
            AccessMode::Read,
            PathGrantSource::TrustedDialog,
        )
        .expect("grant should issue");

        let authorized =
            authorize_path(&state, &link_path, AccessMode::Read, Some(&token)).expect("authorize");
        assert_eq!(
            authorized,
            fs::canonicalize(&target_file).expect("canonical target file")
        );

        let _ = fs::remove_file(&link_path);
        let _ = fs::remove_file(&target_file);
        let _ = fs::remove_dir_all(outside_root);
        let _ = fs::remove_dir_all(target_root);
    }

    fn tempfile_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("star-shuttle-{}-{}", name, Uuid::new_v4()))
    }

    #[test]
    fn text_limit_rejects_oversized_files() {
        let err = ensure_text_limit(MAX_LOCAL_FS_TEXT_BYTES + 1)
            .expect_err("oversized text should be rejected");
        assert!(err.starts_with(LOCAL_FS_TEXT_TOO_LARGE_ERR));
        assert!(err.contains("max_bytes"));
    }

    #[cfg(unix)]
    #[test]
    fn read_open_rejects_symlink_swapped_after_authorization() {
        let outside_root = tempfile_path("outside-swap-read");
        fs::create_dir_all(&outside_root).expect("failed to create outside root");

        let granted_path = outside_root.join("granted-read.txt");
        let victim_path = outside_root.join("victim-read.txt");
        fs::write(&victim_path, "victim-read").expect("failed to create victim file");

        let state = LocalFsState {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: vec![],
            path_grants: Mutex::new(HashMap::new()),
        };

        let token = issue_path_grant(
            &state,
            &granted_path,
            AccessMode::Read,
            PathGrantSource::TrustedDialog,
        )
        .expect("grant should issue");
        let authorized =
            authorize_path(&state, &granted_path, AccessMode::Read, Some(&token)).expect("auth");

        std::os::unix::fs::symlink(&victim_path, &authorized).expect("failed to create symlink");
        let err = open_authorized_path_for_read(&authorized)
            .expect_err("symlink target must be rejected");

        assert!(!err.is_empty());
        assert_eq!(
            fs::read_to_string(&victim_path).expect("failed to read victim"),
            "victim-read"
        );

        let _ = fs::remove_file(&granted_path);
        let _ = fs::remove_file(&victim_path);
        let _ = fs::remove_dir_all(outside_root);
    }

    #[cfg(unix)]
    #[test]
    fn read_token_is_consumed_even_when_open_rejected_after_symlink_swap() {
        let outside_root = tempfile_path("outside-swap-token-read");
        fs::create_dir_all(&outside_root).expect("failed to create outside root");

        let granted_path = outside_root.join("granted-consume-read.txt");
        let victim_path = outside_root.join("victim-consume-read.txt");
        fs::write(&victim_path, "victim-consume-read").expect("failed to create victim file");

        let state = LocalFsState {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: vec![],
            path_grants: Mutex::new(HashMap::new()),
        };

        let token = issue_path_grant(
            &state,
            &granted_path,
            AccessMode::Read,
            PathGrantSource::TrustedDialog,
        )
        .expect("grant should issue");

        let authorized =
            authorize_path(&state, &granted_path, AccessMode::Read, Some(&token)).expect("auth");
        std::os::unix::fs::symlink(&victim_path, &authorized).expect("failed to create symlink");
        let _ = open_authorized_path_for_read(&authorized)
            .expect_err("symlink target must be rejected");

        let retry_err = authorize_path(&state, &granted_path, AccessMode::Read, Some(&token))
            .expect_err("token should still be consumed even when open fails");
        assert!(retry_err.contains("denied"));

        let _ = fs::remove_file(&granted_path);
        let _ = fs::remove_file(&victim_path);
        let _ = fs::remove_dir_all(outside_root);
    }

    #[cfg(unix)]
    #[test]
    fn write_open_rejects_symlink_swapped_after_authorization() {
        let outside_root = tempfile_path("outside-swap");
        fs::create_dir_all(&outside_root).expect("failed to create outside root");

        let granted_path = outside_root.join("granted.txt");
        let victim_path = outside_root.join("victim.txt");
        fs::write(&victim_path, "victim").expect("failed to create victim file");

        let state = LocalFsState {
            handles: Mutex::new(HashMap::new()),
            allowed_roots: vec![],
            path_grants: Mutex::new(HashMap::new()),
        };

        let token = issue_path_grant(
            &state,
            &granted_path,
            AccessMode::Write,
            PathGrantSource::TrustedDialog,
        )
        .expect("grant should issue");
        let authorized =
            authorize_path(&state, &granted_path, AccessMode::Write, Some(&token)).expect("auth");

        std::os::unix::fs::symlink(&victim_path, &authorized).expect("failed to create symlink");
        let err = open_authorized_path_for_write(&authorized, true)
            .expect_err("symlink target must be rejected");

        assert!(!err.is_empty());
        assert_eq!(
            fs::read_to_string(&victim_path).expect("failed to read victim"),
            "victim"
        );

        let _ = fs::remove_file(&granted_path);
        let _ = fs::remove_file(&victim_path);
        let _ = fs::remove_dir_all(outside_root);
    }

    #[test]
    fn stale_handles_are_pruned_on_next_handle_access() {
        use std::time::Duration;

        let state = LocalFsState::default();
        let stale_file = tempfile_path("stale-handle");
        let fresh_file = tempfile_path("fresh-handle");
        fs::write(&stale_file, "stale").expect("failed to create stale file");
        fs::write(&fresh_file, "fresh").expect("failed to create fresh file");

        let stale = LocalFileHandle {
            file: File::open(&stale_file).expect("open stale file"),
            writable: false,
            size: 5,
            last_touched: Instant::now()
                - Duration::from_secs(LOCAL_FS_HANDLE_IDLE_TTL_SECONDS + 1),
        };
        let stale_handle_id = insert_handle(&state, stale).expect("insert stale handle");

        let fresh = LocalFileHandle {
            file: File::open(&fresh_file).expect("open fresh file"),
            writable: false,
            size: 5,
            last_touched: Instant::now(),
        };
        let fresh_handle_id = insert_handle(&state, fresh).expect("insert fresh handle");

        let fresh_data = with_handle(&state, &fresh_handle_id, |handle| {
            let mut buf = Vec::new();
            handle
                .file
                .read_to_end(&mut buf)
                .map_err(|e| e.to_string())?;
            Ok(buf)
        })
        .expect("fresh handle should still work");
        assert_eq!(fresh_data, b"fresh");

        let stale_err = with_handle(&state, &stale_handle_id, |_handle| Ok(()))
            .expect_err("stale handle should be evicted");
        assert!(stale_err.contains("Unknown local file handle"));

        let _ = fs::remove_file(stale_file);
        let _ = fs::remove_file(fresh_file);
    }

    #[test]
    fn stale_handles_are_pruned_on_insert_path() {
        use std::time::Duration;

        let state = LocalFsState::default();
        let stale_file = tempfile_path("stale-handle-insert");
        let fresh_file = tempfile_path("fresh-handle-insert");
        fs::write(&stale_file, "stale").expect("failed to create stale file");
        fs::write(&fresh_file, "fresh").expect("failed to create fresh file");

        let stale = LocalFileHandle {
            file: File::open(&stale_file).expect("open stale file"),
            writable: false,
            size: 5,
            last_touched: Instant::now()
                - Duration::from_secs(LOCAL_FS_HANDLE_IDLE_TTL_SECONDS + 1),
        };
        let stale_handle_id = insert_handle(&state, stale).expect("insert stale handle");

        let fresh = LocalFileHandle {
            file: File::open(&fresh_file).expect("open fresh file"),
            writable: false,
            size: 5,
            last_touched: Instant::now(),
        };
        let _fresh_handle_id = insert_handle(&state, fresh).expect("insert fresh handle");

        let stale_err = with_handle(&state, &stale_handle_id, |_handle| Ok(()))
            .expect_err("stale handle should be evicted by insert path cleanup");
        assert!(stale_err.contains("Unknown local file handle"));

        let _ = fs::remove_file(stale_file);
        let _ = fs::remove_file(fresh_file);
    }

    #[test]
    fn stale_handles_are_pruned_on_close_path() {
        use std::time::Duration;

        let state = LocalFsState::default();
        let stale_file = tempfile_path("stale-handle-close");
        let fresh_file = tempfile_path("fresh-handle-close");
        fs::write(&stale_file, "stale").expect("failed to create stale file");
        fs::write(&fresh_file, "fresh").expect("failed to create fresh file");

        let stale = LocalFileHandle {
            file: File::open(&stale_file).expect("open stale file"),
            writable: false,
            size: 5,
            last_touched: Instant::now()
                - Duration::from_secs(LOCAL_FS_HANDLE_IDLE_TTL_SECONDS + 1),
        };
        let stale_handle_id = insert_handle(&state, stale).expect("insert stale handle");

        let fresh = LocalFileHandle {
            file: File::open(&fresh_file).expect("open fresh file"),
            writable: false,
            size: 5,
            last_touched: Instant::now(),
        };
        let fresh_handle_id = insert_handle(&state, fresh).expect("insert fresh handle");

        close_handle(&state, &fresh_handle_id).expect("close fresh handle");

        let stale_err = with_handle(&state, &stale_handle_id, |_handle| Ok(()))
            .expect_err("stale handle should be evicted by close path cleanup");
        assert!(stale_err.contains("Unknown local file handle"));

        let _ = fs::remove_file(stale_file);
        let _ = fs::remove_file(fresh_file);
    }
}
