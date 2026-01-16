use log::{LevelFilter, Metadata, Record};
use std::sync::Mutex;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

static LOGGER: SimpleLogger = SimpleLogger { inner: Mutex::new(None) };

struct SimpleLogger { inner: Mutex<Option<LogState>> }

struct LogState {
    level: LevelFilter,
    logs: Vec<String>,
    log_file: Option<PathBuf>,
}

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.inner.lock()
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
            let log_entry = format!(
                "{{\"timestamp\":\"{}\",\"level\":\"{}\",\"module\":\"{}\",\"message\":\"{}\",\"file\":\"{}\",\"line\":{}}}",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%z"),
                record.level(),
                module,
                record.args(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0)
            );
            
            if let Ok(mut guard) = self.inner.lock() {
                if let Some(state) = &mut *guard {
                    state.logs.push(log_entry.clone());
                    // Keep only the last 1000 logs
                    if state.logs.len() > 1000 {
                        state.logs.remove(0);
                    }

                    // Write to file with rotation
                    if let Some(path) = &state.log_file {
                        // Check file size (5MB limit)
                        if let Ok(metadata) = fs::metadata(path) {
                            if metadata.len() > 5 * 1024 * 1024 {
                                let _ = rotate_logs(path);
                            }
                        }

                        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
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

        if let Ok(mut guard) = LOGGER.inner.lock() {
            *guard = Some(LogState {
                level,
                logs: Vec::new(),
                log_file: Some(log_file),
            });
        }
        
        Ok(())
    }
    
    pub fn get_logs() -> Vec<String> {
        if let Ok(guard) = LOGGER.inner.lock() {
            if let Some(state) = &*guard {
                return state.logs.clone();
            }
        }
        Vec::new()
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