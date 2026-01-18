use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionInfo {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_type: AuthType,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AuthType {
    Password,
    PrivateKey,
    Certificate,
    Agent,
    Kerberos,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerminalSettings {
    pub cols: u16,
    pub rows: u16,
    pub font_size: u8,
    pub font_family: String,
    pub theme: TerminalTheme,
    pub cursor_blink: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TerminalTheme {
    pub foreground: String,
    pub background: String,
    pub cursor: String,
    pub selection: String,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub bright_black: String,
    pub bright_red: String,
    pub bright_green: String,
    pub bright_yellow: String,
    pub bright_blue: String,
    pub bright_magenta: String,
    pub bright_cyan: String,
    pub bright_white: String,
}

impl Default for TerminalSettings {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            font_size: 14,
            font_family: "Monaco, 'Courier New', monospace".to_string(),
            theme: TerminalTheme::default(),
            cursor_blink: true,
        }
    }
}

impl Default for TerminalTheme {
    fn default() -> Self {
        Self {
            foreground: "#ffffff".to_string(),
            background: "#000000".to_string(),
            cursor: "#ffffff".to_string(),
            selection: "#ffffff20".to_string(),
            black: "#000000".to_string(),
            red: "#ff0000".to_string(),
            green: "#00ff00".to_string(),
            yellow: "#ffff00".to_string(),
            blue: "#0000ff".to_string(),
            magenta: "#ff00ff".to_string(),
            cyan: "#00ffff".to_string(),
            white: "#ffffff".to_string(),
            bright_black: "#808080".to_string(),
            bright_red: "#ff8080".to_string(),
            bright_green: "#80ff80".to_string(),
            bright_yellow: "#ffff80".to_string(),
            bright_blue: "#8080ff".to_string(),
            bright_magenta: "#ff80ff".to_string(),
            bright_cyan: "#80ffff".to_string(),
            bright_white: "#ffffff".to_string(),
        }
    }
}

impl Default for ConnectionInfo {
    fn default() -> Self {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            id: Uuid::new_v4(),
            name: "New Connection".to_string(),
            host: String::new(), // 改为空字符串，避免默认连接localhost
            port: 0,             // 改为0，避免默认使用22端口
            username: "".to_string(),
            auth_type: AuthType::Password,
            created_at: now,
            updated_at: now,
        }
    }
}
