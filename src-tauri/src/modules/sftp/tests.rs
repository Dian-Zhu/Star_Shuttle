#![cfg(test)]

use super::common::{
    ensure_max_bytes, ensure_scp_upload_size, ensure_sftp_write_size, validate_remote_leaf_name,
    validate_scp_file_name, MAX_SCP_UPLOAD_BYTES, MAX_SFTP_CHUNK_BYTES,
};
use super::*;

#[test]
fn test_generation_validation_detects_invalidation() {
    let generations = StdMutex::new(HashMap::new());
    let session_id = Uuid::new_v4();

    assert!(is_generation_valid(&generations, session_id, 0).unwrap());

    {
        let mut guard = generations.lock().unwrap();
        guard.insert(session_id, 1);
    }

    assert!(!is_generation_valid(&generations, session_id, 0).unwrap());
    assert!(is_generation_valid(&generations, session_id, 1).unwrap());
}

#[test]
fn test_finalize_io_result_accepts_success_when_generation_valid() {
    let generations = StdMutex::new(HashMap::new());
    let session_id = Uuid::new_v4();
    let result = finalize_io_result(Ok::<u32, String>(7), &generations, session_id, 0)
        .expect("success should be preserved when generation is valid");
    assert_eq!(result, 7);
}

#[test]
fn test_finalize_io_result_rejects_success_when_generation_invalidated() {
    let generations = StdMutex::new(HashMap::new());
    let session_id = Uuid::new_v4();
    {
        let mut guard = generations.lock().expect("lock generations");
        guard.insert(session_id, 1);
    }

    let error = finalize_io_result(Ok::<u32, String>(7), &generations, session_id, 0)
        .expect_err("stale generation success must be rejected");
    assert!(error.contains("has been invalidated"));
}

#[test]
fn test_finalize_io_result_keeps_io_error() {
    let generations = StdMutex::new(HashMap::new());
    let session_id = Uuid::new_v4();
    {
        let mut guard = generations.lock().expect("lock generations");
        guard.insert(session_id, 1);
    }

    let error = finalize_io_result::<u32>(
        Err("remote write failed".to_string()),
        &generations,
        session_id,
        0,
    )
    .expect_err("io error should be returned");
    assert_eq!(error, "remote write failed");
}

#[test]
fn test_remove_session_bumps_generation() {
    let connection_manager = Arc::new(std::sync::RwLock::new(DefaultConnectionManager::new()));
    let sftp_manager = SftpManager::new(connection_manager);
    let session_id = Uuid::new_v4();

    assert_eq!(sftp_manager.current_generation(session_id).unwrap(), 0);

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("failed to create runtime");
    runtime.block_on(async {
        sftp_manager.remove_session(session_id).await;
    });

    assert_eq!(sftp_manager.current_generation(session_id).unwrap(), 1);
}

#[test]
fn test_ensure_max_bytes_allows_values_within_limit() {
    assert!(ensure_max_bytes(1024, 2048, "read").is_ok());
}

#[test]
fn test_ensure_max_bytes_rejects_values_above_limit() {
    let error = ensure_max_bytes(4097, 4096, "chunk").expect_err("expected limit error");
    assert!(error.contains("chunk exceeds limit"));
}

#[test]
fn test_ensure_generation_current_rejects_stale_generation() {
    let generations = StdMutex::new(HashMap::new());
    let session_id = Uuid::new_v4();
    {
        let mut guard = generations.lock().expect("lock generations");
        guard.insert(session_id, 2);
    }

    let error = ensure_generation_current(&generations, session_id, 1)
        .expect_err("stale generation should be rejected");
    assert!(error.contains("has been invalidated"));
}

#[test]
fn test_ensure_sftp_write_size_rejects_values_above_limit() {
    let error = ensure_sftp_write_size(MAX_SFTP_CHUNK_BYTES + 1, "upload")
        .expect_err("SFTP write above limit should be rejected");
    assert!(error.contains("upload exceeds limit"));
}

#[test]
fn test_ensure_scp_upload_size_rejects_values_above_limit() {
    let error = ensure_scp_upload_size(MAX_SCP_UPLOAD_BYTES + 1, "upload")
        .expect_err("SCP upload above limit should be rejected");
    assert!(error.contains("upload exceeds limit"));
}

#[test]
fn test_validate_scp_file_name_accepts_regular_name() {
    assert!(validate_scp_file_name("normal file.txt").is_ok());
}

#[test]
fn test_validate_scp_file_name_rejects_control_chars() {
    assert!(validate_scp_file_name("bad\nname").is_err());
    assert!(validate_scp_file_name("bad\rname").is_err());
    assert!(validate_scp_file_name("bad\u{0000}name").is_err());
}

#[test]
fn test_validate_scp_file_name_rejects_path_separators() {
    assert!(validate_scp_file_name("dir/file.txt").is_err());
}

#[test]
fn test_validate_remote_leaf_name_rejects_dot_segments() {
    assert!(validate_remote_leaf_name(".").is_err());
    assert!(validate_remote_leaf_name("..").is_err());
}

#[test]
fn test_validate_remote_leaf_name_rejects_whitespace_only_names() {
    assert!(validate_remote_leaf_name("   ").is_err());
}
