pub struct AppConfig {
    pub database_path: String,
    pub log_level: String,
    pub default_terminal_size: (u16, u16),
    pub auto_save_connections: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_path: "~/.ssh_remote_manager.db".to_string(),
            log_level: "info".to_string(),
            default_terminal_size: (80, 24),
            auto_save_connections: true,
        }
    }
}

pub struct ConfigManager {
    config: AppConfig,
}

impl ConfigManager {
    pub fn new() -> Self {
        Self {
            config: AppConfig::default(),
        }
    }
    
    pub fn get_config(&self) -> &AppConfig {
        &self.config
    }
    
    pub fn update_config(&mut self, config: AppConfig) {
        self.config = config;
    }
}