use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// LLM 消息角色
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// 单条消息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn system(content: impl Into<String>) -> Self {
        Self { role: MessageRole::System, content: content.into(), tool_call_id: None, name: None }
    }
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: MessageRole::User, content: content.into(), tool_call_id: None, name: None }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: MessageRole::Assistant, content: content.into(), tool_call_id: None, name: None }
    }
}

/// 流式事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// 文本增量
    Delta { content: String },
    /// 流结束
    Done { conversation_id: Uuid },
    /// 错误
    Error { message: String },
}

/// AI 提供商
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AiProvider {
    OpenAI,
    Claude,
    DeepSeek,
    Ollama,
    Custom,
}

impl Default for AiProvider {
    fn default() -> Self {
        AiProvider::OpenAI
    }
}

/// AI 配置（对应数据库 ai_config 表）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    pub provider: AiProvider,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_context_lines")]
    pub context_lines: u32,
}

fn default_temperature() -> f32 { 0.7 }
fn default_max_tokens() -> u32 { 4096 }
fn default_context_lines() -> u32 { 100 }

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: AiProvider::OpenAI,
            api_key: String::new(),
            model: "gpt-4o".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            temperature: 0.7,
            max_tokens: 4096,
            context_lines: 100,
        }
    }
}

/// 对话记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: Uuid,
    pub title: String,
    pub session_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

/// 持久化消息记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredMessage {
    pub id: Uuid,
    pub conversation_id: Uuid,
    pub role: String,
    pub content: String,
    pub context_snapshot: Option<String>,
    pub created_at: String,
}

/// 终端上下文快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalContext {
    pub session_id: Uuid,
    pub host: String,
    pub content: String,
    pub lines_count: usize,
}
