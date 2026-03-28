use log::{LevelFilter, Metadata, Record};
use serde_json::json;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MAX_LOG_MESSAGE_CHARS: usize = 2048;
#[cfg(unix)]
const UNIX_LOG_DIR_MODE: u32 = 0o700;
#[cfg(unix)]
const UNIX_LOG_FILE_MODE: u32 = 0o600;

static LOGGER: SimpleLogger = SimpleLogger {
    inner: Mutex::new(None),
};

struct SimpleLogger {
    inner: Mutex<Option<LogState>>,
}

struct LogState {
    level: LevelFilter,
    logs: VecDeque<String>,
    log_file: Option<PathBuf>,
}

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level()
            <= self
                .inner
                .lock()
                .map(|guard| guard.as_ref().map_or(LevelFilter::Off, |s| s.level))
                .unwrap_or(LevelFilter::Off)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // Get module path or use "unknown" if not available
            let module = record.module_path().unwrap_or("unknown");
            let message = sanitize_log_message(&record.args().to_string());

            // Format log entry for console (human readable)
            let console_entry = format!(
                "{} [{}] [{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.level(),
                module,
                message
            );
            eprintln!("{}", console_entry);

            // Format log entry for file/storage (JSON)
            let log_entry = json!({
                "timestamp": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%z").to_string(),
                "level": record.level().to_string(),
                "module": module,
                "message": message,
                "file": record.file().unwrap_or("unknown"),
                "line": record.line().unwrap_or(0),
            })
            .to_string();

            if let Ok(mut guard) = self.inner.lock() {
                if let Some(state) = &mut *guard {
                    state.logs.push_back(log_entry.clone());
                    // Keep only the last 1000 logs
                    if state.logs.len() > 1000 {
                        state.logs.pop_front();
                    }

                    // Write to file with rotation
                    if let Some(path) = &state.log_file {
                        // Check file size (5MB limit)
                        if let Ok(metadata) = fs::metadata(path) {
                            if metadata.len() > 5 * 1024 * 1024 {
                                let _ = rotate_logs(path);
                            }
                        }

                        if let Ok(mut file) = open_log_file_for_append(path) {
                            let _ = writeln!(file, "{}", log_entry);
                        }
                    }
                }
            }
        }
    }

    fn flush(&self) {}
}

fn rotate_logs(base_path: &Path) -> std::io::Result<()> {
    let max_backups = 3;

    let get_path = |i: usize| -> PathBuf {
        let mut p = base_path.as_os_str().to_owned();
        p.push(format!(".{}", i));
        PathBuf::from(p)
    };

    // Rotate .2 -> .3, .1 -> .2
    for i in (1..max_backups).rev() {
        let src = get_path(i);
        let dst = get_path(i + 1);
        if src.exists() {
            let _ = fs::rename(&src, &dst);
            #[cfg(unix)]
            if dst.exists() {
                enforce_unix_permissions(&dst, UNIX_LOG_FILE_MODE);
            }
        }
    }

    // Rotate base -> .1
    let dst = get_path(1);
    if base_path.exists() {
        let _ = fs::rename(base_path, &dst);
        #[cfg(unix)]
        if dst.exists() {
            enforce_unix_permissions(&dst, UNIX_LOG_FILE_MODE);
        }
    }

    Ok(())
}

pub struct LogManager;

impl LogManager {
    pub fn init(level: LevelFilter) -> Result<(), log::SetLoggerError> {
        log::set_logger(&LOGGER)?;
        log::set_max_level(level);

        let log_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("star_shuttle")
            .join("logs");

        ensure_log_directory(&log_dir);
        let log_file = log_dir.join("app.log");

        let logs = read_last_lines(&log_file, 1000).unwrap_or_default();
        if let Ok(mut guard) = LOGGER.inner.lock() {
            *guard = Some(LogState {
                level,
                logs,
                log_file: Some(log_file),
            });
        }

        Ok(())
    }

    pub fn get_logs() -> Vec<String> {
        if let Ok(guard) = LOGGER.inner.lock() {
            if let Some(state) = &*guard {
                return state.logs.iter().cloned().collect();
            }
        }
        Vec::new()
    }

    pub fn clear_logs() {
        if let Ok(mut guard) = LOGGER.inner.lock() {
            if let Some(state) = &mut *guard {
                state.logs.clear();
                if let Some(path) = &state.log_file {
                    let _ = open_log_file_for_truncate(path);
                }
            }
        }
    }

    pub fn get_log_file_path() -> Option<String> {
        if let Ok(guard) = LOGGER.inner.lock() {
            if let Some(state) = &*guard {
                return state
                    .log_file
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string());
            }
        }
        None
    }

    pub fn set_level(level: LevelFilter) {
        log::set_max_level(level);
        if let Ok(mut guard) = LOGGER.inner.lock() {
            if let Some(state) = &mut *guard {
                state.level = level;
            }
        }
    }
}

fn read_last_lines(path: &Path, max_lines: usize) -> std::io::Result<VecDeque<String>> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(VecDeque::new()),
        Err(e) => return Err(e),
    };

    let mut lines: VecDeque<String> = VecDeque::new();
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        lines.push_back(line);
        if lines.len() > max_lines {
            lines.pop_front();
        }
    }

    Ok(lines)
}

fn sanitize_log_message(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    enum EscapeState {
        None,
        Started,
        Csi,
    }
    let mut escape_state = EscapeState::None;

    for ch in input.chars() {
        match escape_state {
            EscapeState::None => {
                if ch == '\u{1b}' {
                    escape_state = EscapeState::Started;
                    continue;
                }
            }
            EscapeState::Started => {
                if ch == '[' {
                    escape_state = EscapeState::Csi;
                } else {
                    escape_state = EscapeState::None;
                }
                continue;
            }
            EscapeState::Csi => {
                if ('@'..='~').contains(&ch) {
                    escape_state = EscapeState::None;
                }
                continue;
            }
        }

        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ if ch.is_control() => out.push('?'),
            _ => out.push(ch),
        }
    }

    if out.chars().count() <= MAX_LOG_MESSAGE_CHARS {
        return out;
    }

    let truncated: String = out.chars().take(MAX_LOG_MESSAGE_CHARS).collect();
    format!("{}...(truncated)", truncated)
}

fn ensure_log_directory(path: &Path) {
    if fs::create_dir_all(path).is_ok() {
        #[cfg(unix)]
        enforce_unix_permissions(path, UNIX_LOG_DIR_MODE);
    }
}

fn open_log_file_for_append(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(UNIX_LOG_FILE_MODE);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    enforce_unix_permissions(path, UNIX_LOG_FILE_MODE);
    Ok(file)
}

fn open_log_file_for_truncate(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(UNIX_LOG_FILE_MODE);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    enforce_unix_permissions(path, UNIX_LOG_FILE_MODE);
    Ok(file)
}

#[cfg(unix)]
fn enforce_unix_permissions(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;

    if let Ok(metadata) = fs::metadata(path) {
        let mut permissions = metadata.permissions();
        if permissions.mode() & 0o777 != mode {
            permissions.set_mode(mode);
            let _ = fs::set_permissions(path, permissions);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{sanitize_log_message, MAX_LOG_MESSAGE_CHARS};

    #[test]
    fn sanitize_log_message_strips_control_and_ansi() {
        let input = "ok\u{1b}[31mRED\u{1b}[0m\nnext\tline\r\u{0007}";
        let sanitized = sanitize_log_message(input);
        assert_eq!(sanitized, "okRED\\nnext\\tline\\r?");
    }

    #[test]
    fn sanitize_log_message_truncates_long_messages() {
        let input = "a".repeat(MAX_LOG_MESSAGE_CHARS + 32);
        let sanitized = sanitize_log_message(&input);
        assert!(sanitized.ends_with("...(truncated)"));
        assert!(sanitized.len() > MAX_LOG_MESSAGE_CHARS);
    }
}
