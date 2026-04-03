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

pub fn validate_scp_file_name(file_name: &str) -> Result<(), String> {
    if file_name.is_empty() {
        return Err("Invalid remote path".to_string());
    }
    if file_name.contains('/') {
        return Err("SCP file name must not contain path separators".to_string());
    }
    if file_name.chars().any(|ch| ch.is_control()) {
        return Err("SCP file name contains unsupported control characters".to_string());
    }
    Ok(())
}

/// Validates the directory component of an SCP remote path to prevent path traversal attacks.
pub fn validate_scp_directory(dir: &str) -> Result<(), String> {
    if dir.is_empty() {
        return Err("SCP directory path cannot be empty".to_string());
    }
    if dir.contains('\0') {
        return Err("SCP directory path contains null byte".to_string());
    }
    if dir.chars().any(|ch| ch.is_control()) {
        return Err("SCP directory path contains unsupported control characters".to_string());
    }
    // Reject path traversal sequences
    for component in dir.split('/') {
        if component == ".." {
            return Err("Path traversal (..) is not allowed in SCP directory".to_string());
        }
    }
    // Only allow absolute paths or "." (current directory)
    if dir != "." && !dir.starts_with('/') {
        return Err("SCP directory must be an absolute path or '.'".to_string());
    }
    if dir.len() > 4096 {
        return Err("SCP directory path exceeds maximum length".to_string());
    }
    Ok(())
}
