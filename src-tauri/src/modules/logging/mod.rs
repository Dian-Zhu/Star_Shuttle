use log::{LevelFilter, Metadata, Record};
use serde_json::json;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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
                .map(|guard| guard.as_ref().map_or(LevelFilter::Info, |s| s.level))
                .unwrap_or(LevelFilter::Info)
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // Get module path or use "unknown" if not available
            let module = record.module_path().unwrap_or("unknown");

            // Format log entry for console (human readable)
            let console_entry = format!(
                "{} [{}] [{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.level(),
                module,
                record.args()
            );
            eprintln!("{}", console_entry);

            // Format log entry for file/storage (JSON)
            let log_entry = json!({
                "timestamp": chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%z").to_string(),
                "level": record.level().to_string(),
                "module": module,
                "message": record.args().to_string(),
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

                        if let Ok(mut file) =
                            OpenOptions::new().create(true).append(true).open(path)
                        {
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
            let _ = fs::rename(src, dst);
        }
    }

    // Rotate base -> .1
    let dst = get_path(1);
    if base_path.exists() {
        let _ = fs::rename(base_path, dst);
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

        let _ = fs::create_dir_all(&log_dir);
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
                    let _ = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(path);
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
