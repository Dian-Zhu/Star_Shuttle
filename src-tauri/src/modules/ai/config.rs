use crate::modules::ai::types::{AiConfig, AiProvider};
use crate::modules::db::DatabaseManager;
use std::sync::{Arc, Mutex};

const CONFIG_KEY: &str = "ai_config";

/// 从数据库加载 AI 配置，不存在则返回默认值
pub fn load_config(db: &Arc<Mutex<DatabaseManager>>) -> Result<AiConfig, String> {
    let db = db.lock().map_err(|e| e.to_string())?;
    match db.get_setting(CONFIG_KEY).map_err(|e| e.to_string())? {
        Some(json) => serde_json::from_str(&json).map_err(|e| e.to_string()),
        None => Ok(AiConfig::default()),
    }
}

/// 保存 AI 配置到数据库（API Key 明文存储，后续可升级为 keyring）
pub fn save_config(db: &Arc<Mutex<DatabaseManager>>, config: &AiConfig) -> Result<(), String> {
    let json = serde_json::to_string(config).map_err(|e| e.to_string())?;
    let db = db.lock().map_err(|e| e.to_string())?;
    db.save_setting(CONFIG_KEY, &json).map_err(|e| e.to_string())
}

/// 根据 provider 返回默认 base_url
pub fn default_base_url(provider: &AiProvider) -> &'static str {
    match provider {
        AiProvider::OpenAI => "https://api.openai.com/v1",
        AiProvider::Claude => "https://api.anthropic.com/v1",
        AiProvider::DeepSeek => "https://api.deepseek.com/v1",
        AiProvider::Ollama => "http://localhost:11434/v1",
        AiProvider::Custom => "",
    }
}

/// 根据 provider 返回默认 model
pub fn default_model(provider: &AiProvider) -> &'static str {
    match provider {
        AiProvider::OpenAI => "gpt-4o",
        AiProvider::Claude => "claude-3-5-sonnet-20241022",
        AiProvider::DeepSeek => "deepseek-chat",
        AiProvider::Ollama => "llama3.2",
        AiProvider::Custom => "",
    }
}
