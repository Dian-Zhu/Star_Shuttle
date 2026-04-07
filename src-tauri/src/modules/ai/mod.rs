pub mod agent;
pub mod chat;
pub mod client;
pub mod command_parser;
pub mod config;
pub mod context_collector;
pub mod sandbox;
pub mod types;

use crate::modules::ai::{
    agent::{AgentManager, AgentTask},
    chat::ChatManager,
    config::{
        default_base_url, default_model, load_config, save_config,
        validate_agent_compatibility,
    },
    context_collector::collect_terminal_context,
    sandbox::SandboxMode,
    types::{AiConfig, AiProvider, Conversation, StoredMessage},
};
use crate::modules::connection::DefaultConnectionManager;
use crate::modules::db::DatabaseManager;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, State};
use uuid::Uuid;

// ── AI 配置 Commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ai_get_config(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
) -> Result<AiConfig, String> {
    load_config(db.inner())
}

#[tauri::command]
pub async fn ai_save_config(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    config: AiConfig,
) -> Result<(), String> {
    save_config(db.inner(), &config)
}

#[tauri::command]
pub async fn ai_get_provider_defaults(provider: String) -> Result<serde_json::Value, String> {
    let p = match provider.as_str() {
        "openai" => AiProvider::OpenAI,
        "claude" => AiProvider::Claude,
        "deepseek" => AiProvider::DeepSeek,
        "ollama" => AiProvider::Ollama,
        _ => AiProvider::Custom,
    };
    Ok(serde_json::json!({
        "base_url": default_base_url(&p),
        "model": default_model(&p),
    }))
}

#[tauri::command]
pub async fn ai_test_connection(
    chat_manager: State<'_, Arc<ChatManager>>,
) -> Result<(), String> {
    chat_manager.test_connection().await
}

// ── Chat Commands ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ai_chat_new(
    chat_manager: State<'_, Arc<ChatManager>>,
    session_id: Option<Uuid>,
) -> Result<Uuid, String> {
    chat_manager.new_conversation(session_id)
}

#[tauri::command]
pub async fn ai_chat_list(
    chat_manager: State<'_, Arc<ChatManager>>,
) -> Result<Vec<Conversation>, String> {
    chat_manager.list_conversations()
}

#[tauri::command]
pub async fn ai_chat_messages(
    chat_manager: State<'_, Arc<ChatManager>>,
    conversation_id: Uuid,
) -> Result<Vec<StoredMessage>, String> {
    chat_manager.get_messages(conversation_id)
}

#[tauri::command]
pub async fn ai_chat_send(
    app: AppHandle,
    chat_manager: State<'_, Arc<ChatManager>>,
    manager: State<'_, Arc<RwLock<DefaultConnectionManager>>>,
    conversation_id: Uuid,
    content: String,
    session_id: Option<Uuid>,
    include_terminal_context: bool,
) -> Result<String, String> {
    let terminal_context = if include_terminal_context {
        session_id.and_then(|sid| {
            collect_terminal_context(manager.inner(), sid, 100).ok()
        })
    } else {
        None
    };

    chat_manager
        .send_message(&app, conversation_id, content, terminal_context)
        .await
}

#[tauri::command]
pub async fn ai_chat_cancel(
    chat_manager: State<'_, Arc<ChatManager>>,
    conversation_id: Uuid,
) -> Result<(), String> {
    chat_manager.cancel_active_request(conversation_id)
}

#[tauri::command]
pub async fn ai_chat_clear(
    chat_manager: State<'_, Arc<ChatManager>>,
    conversation_id: Uuid,
) -> Result<(), String> {
    chat_manager.clear_messages(conversation_id)
}

#[tauri::command]
pub async fn ai_chat_delete(
    chat_manager: State<'_, Arc<ChatManager>>,
    conversation_id: Uuid,
) -> Result<(), String> {
    chat_manager.delete_conversation(conversation_id)
}

// ── 终端上下文 Command ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ai_get_terminal_context(
    manager: State<'_, Arc<RwLock<DefaultConnectionManager>>>,
    session_id: Uuid,
    lines: Option<u32>,
) -> Result<crate::modules::ai::types::TerminalContext, String> {
    collect_terminal_context(manager.inner(), session_id, lines.unwrap_or(100))
}

// ── Agent Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ai_agent_start(
    app: AppHandle,
    agent_manager: State<'_, Arc<AgentManager>>,
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    session_id: Uuid,
    instruction: String,
    sandbox_mode: Option<String>,
) -> Result<Uuid, String> {
    let mode = match sandbox_mode.as_deref() {
        Some("full") => SandboxMode::Full,
        _ => SandboxMode::Standard,
    };

    let config = load_config(db.inner())?;
    validate_agent_compatibility(&config)?;

    agent_manager
        .inner()
        .clone()
        .start_task(app, session_id, instruction, mode)
        .await
}

#[tauri::command]
pub async fn ai_agent_confirm(
    agent_manager: State<'_, Arc<AgentManager>>,
    task_id: Uuid,
    confirmed: bool,
) -> Result<(), String> {
    agent_manager.confirm_step(task_id, confirmed)
}

#[tauri::command]
pub async fn ai_agent_cancel(
    agent_manager: State<'_, Arc<AgentManager>>,
    task_id: Uuid,
) -> Result<(), String> {
    agent_manager.cancel_task(task_id)
}

#[tauri::command]
pub async fn ai_agent_status(
    agent_manager: State<'_, Arc<AgentManager>>,
    task_id: Uuid,
) -> Result<Option<AgentTask>, String> {
    Ok(agent_manager.get_task(task_id))
}
