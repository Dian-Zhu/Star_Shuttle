use log::{LevelFilter, Metadata, Record}; use std::sync::Mutex;

static LOGGER: SimpleLogger = SimpleLogger { inner: Mutex::new(None) };

struct SimpleLogger { inner: Mutex<Option<LogState>> }

struct LogState {
    level: LevelFilter,
    logs: Vec<String>,
}

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.inner.lock().unwrap().as_ref().map_or(LevelFilter::Info, |s| s.level)
    }
    
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let log_entry = format!("{} - [{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"), record.level(), record.args());
            println!("{}", log_entry);
            
            if let Ok(mut guard) = self.inner.lock() {
                if let Some(state) = &mut *guard {
                    state.logs.push(log_entry);
                    // Keep only the last 1000 logs
                    if state.logs.len() > 1000 {
                        state.logs.remove(0);
                    }
                }
            }
        }
    }
    
    fn flush(&self) {}
}

pub struct LogManager;

impl LogManager {
    pub fn init(level: LevelFilter) -> Result<(), log::SetLoggerError> {
        log::set_logger(&LOGGER)?;
        log::set_max_level(level);
        
        if let Ok(mut guard) = LOGGER.inner.lock() {
            *guard = Some(LogState {
                level,
                logs: Vec::new(),
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