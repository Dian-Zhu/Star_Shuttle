use std::path::{Path, PathBuf}; use std::fs;

pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(home_dir) = dirs::home_dir() {
        if path.starts_with('~') {
            return home_dir.join(path.strip_prefix('~').unwrap_or(""));
        }
    }
    PathBuf::from(path)
}

pub fn is_valid_path(path: &str) -> bool {
    Path::new(path).exists()
}

pub fn get_file_size(path: &Path) -> Result<u64, std::io::Error> {
    let metadata = fs::metadata(path)?;
    Ok(metadata.len())
}

pub fn format_file_size(size: u64) -> String {
    if size < 1024 {
        return format!("{} B", size);
    } else if size < 1024 * 1024 {
        return format!("{:.2} KB", size as f64 / 1024.0);
    } else if size < 1024 * 1024 * 1024 {
        return format!("{:.2} MB", size as f64 / (1024.0 * 1024.0));
    } else {
        return format!("{:.2} GB", size as f64 / (1024.0 * 1024.0 * 1024.0));
    }
}

pub fn format_datetime(timestamp: u64) -> String {
    let seconds = timestamp as i64 / 1000;
    let nanoseconds = (timestamp % 1000) as u32 * 1_000_000;
    
    let datetime = chrono::DateTime::from_timestamp(seconds, nanoseconds)
        .map(|dt| dt.with_timezone(&chrono::Local))
        .unwrap_or_else(|| chrono::Local::now());
    
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn generate_random_string(length: usize) -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    
    let mut rng = rand::thread_rng();
    let mut result = String::with_capacity(length);
    
    for _ in 0..length {
        let idx = rng.gen_range(0..CHARSET.len());
        result.push(CHARSET[idx] as char);
    }
    
    result
}

pub fn parse_ssh_url(url: &str) -> Option<(String, String, u16)> {
    // Format: ssh://user@host:port or user@host:port
    let url = url.trim_start_matches("ssh://");
    
    if let Some(at_idx) = url.find('@') {
        let username = &url[..at_idx];
        let rest = &url[at_idx + 1..];
        
        if let Some(colon_idx) = rest.rfind(':') {
            let host = &rest[..colon_idx];
            if let Ok(port) = rest[colon_idx + 1..].parse::<u16>() {
                return Some((username.to_string(), host.to_string(), port));
            }
        }
        
        // No port specified, use default 22
        return Some((username.to_string(), rest.to_string(), 22));
    }
    
    None
}

pub fn escape_shell_string(s: &str) -> String {
    // Basic shell string escaping
    s.replace('"', "\\\"").replace('\'', "\\\'")
}

pub fn unescape_shell_string(s: &str) -> String {
    // Basic shell string unescaping
    s.replace("\\\"", "\"").replace("\\\'", "\'")
}