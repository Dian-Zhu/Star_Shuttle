pub const MAX_SFTP_READ_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_SFTP_CHUNK_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_SCP_UPLOAD_BYTES: usize = 8 * 1024 * 1024;
pub const STREAM_READ_CHUNK_SIZE: usize = 64 * 1024;

pub fn ensure_max_bytes(size: usize, limit: usize, label: &str) -> Result<(), String> {
    if size > limit {
        return Err(format!("{} exceeds limit of {} bytes", label, limit));
    }
    Ok(())
}

pub fn ensure_sftp_write_size(size: usize, label: &str) -> Result<(), String> {
    ensure_max_bytes(size, MAX_SFTP_CHUNK_BYTES, label)
}

pub fn ensure_scp_upload_size(size: usize, label: &str) -> Result<(), String> {
    ensure_max_bytes(size, MAX_SCP_UPLOAD_BYTES, label)
}

pub fn validate_remote_leaf_name(file_name: &str) -> Result<(), String> {
    if file_name.trim().is_empty() {
        return Err("Invalid remote path".to_string());
    }
    if file_name == "." || file_name == ".." {
        return Err("Remote file name must not be . or ..".to_string());
    }
    if file_name.contains('/') {
        return Err("Remote file name must not contain path separators".to_string());
    }
    if file_name.chars().any(|ch| ch.is_control()) {
        return Err("Remote file name contains unsupported control characters".to_string());
    }
    Ok(())
}

pub fn validate_scp_file_name(file_name: &str) -> Result<(), String> {
    validate_remote_leaf_name(file_name)
}
