pub mod agent;
pub mod agent_store;
pub mod agent_types;
pub mod chat;
pub mod client;
pub mod command_parser;
pub mod config;
pub mod context_collector;
pub mod orchestrator;
pub mod planner;
pub mod sandbox;
pub mod skills;
pub mod tools;
pub mod types;

use crate::modules::ai::{
    agent::AgentManager,
    agent_types::{AgentEvent, AgentTaskSnapshot, AgentTaskSummary},
    chat::ChatManager,
    config::{
        default_base_url, default_model, load_config, save_config, validate_agent_compatibility,
    },
    context_collector::collect_terminal_context,
    sandbox::SandboxMode,
    skills::{
        install_skill_from_dir, list_installed_skills, list_skills, match_skills, reload_skills,
        remove_skill, set_skill_enabled, set_skill_trusted, AiSkillMatchResult, AiSkillSummary,
        SkillMode,
    },
    types::{AiConfig, AiProvider, Conversation, StoredMessage},
};
use crate::modules::connection::DefaultConnectionManager;
use crate::modules::db::DatabaseManager;
use std::sync::{Arc, Mutex, RwLock};
use tauri::{AppHandle, State};
use uuid::Uuid;

// ── AI 配置 Commands ──────────────────────────────────────────────────────────

fn parse_skill_mode(mode: Option<&str>) -> Result<Option<SkillMode>, String> {
    match mode {
        Some("chat") => Ok(Some(SkillMode::Chat)),
        Some("agent") => Ok(Some(SkillMode::Agent)),
        Some(other) => Err(format!("未知 skill mode: {}", other)),
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn ai_get_config(db: State<'_, Arc<Mutex<DatabaseManager>>>) -> Result<AiConfig, String> {
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
pub async fn ai_test_connection(chat_manager: State<'_, Arc<ChatManager>>) -> Result<(), String> {
    chat_manager.test_connection().await
}

#[tauri::command]
pub async fn ai_list_skills(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    mode: Option<String>,
) -> Result<Vec<AiSkillSummary>, String> {
    let mode = parse_skill_mode(mode.as_deref())?;
    list_skills(db.inner(), mode)
}

#[tauri::command]
pub async fn ai_list_installed_skills(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    mode: Option<String>,
) -> Result<Vec<AiSkillSummary>, String> {
    let mode = parse_skill_mode(mode.as_deref())?;
    list_installed_skills(db.inner(), mode)
}

#[tauri::command]
pub async fn ai_install_skill_from_dir(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    path: String,
) -> Result<AiSkillSummary, String> {
    install_skill_from_dir(db.inner(), std::path::Path::new(&path))
}

#[tauri::command]
pub async fn ai_set_skill_enabled(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    skill_id: String,
    enabled: bool,
) -> Result<AiSkillSummary, String> {
    set_skill_enabled(db.inner(), &skill_id, enabled)
}

#[tauri::command]
pub async fn ai_set_skill_trusted(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    skill_id: String,
    trusted: bool,
) -> Result<AiSkillSummary, String> {
    set_skill_trusted(db.inner(), &skill_id, trusted)
}

#[tauri::command]
pub async fn ai_remove_skill(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    skill_id: String,
) -> Result<(), String> {
    remove_skill(db.inner(), &skill_id)
}

#[tauri::command]
pub async fn ai_reload_skills(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    mode: Option<String>,
) -> Result<Vec<AiSkillSummary>, String> {
    let mode = parse_skill_mode(mode.as_deref())?;
    reload_skills(db.inner(), mode)
}

#[tauri::command]
pub async fn ai_match_skills(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    input: String,
    mode: Option<String>,
) -> Result<AiSkillMatchResult, String> {
    let mode = parse_skill_mode(mode.as_deref())?.unwrap_or(SkillMode::Chat);
    match_skills(db.inner(), &input, mode)
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
    skill_id: Option<String>,
) -> Result<String, String> {
    let terminal_context = if include_terminal_context {
        session_id.and_then(|sid| collect_terminal_context(manager.inner(), sid, 100).ok())
    } else {
        None
    };

    chat_manager
        .send_message(&app, conversation_id, content, terminal_context, skill_id)
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
    skill_id: Option<String>,
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
        .start_task(app, session_id, instruction, mode, skill_id)
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
    app: AppHandle,
    agent_manager: State<'_, Arc<AgentManager>>,
    task_id: Uuid,
) -> Result<(), String> {
    agent_manager.cancel_task(task_id)?;
    if let Some(task) = agent_manager.get_task(task_id)? {
        use tauri::Emitter;
        let _ = app.emit(&format!("ai-agent-task-{}", task_id), task);
    }
    Ok(())
}

#[tauri::command]
pub async fn ai_agent_get_task(
    agent_manager: State<'_, Arc<AgentManager>>,
    task_id: Uuid,
) -> Result<Option<AgentTaskSnapshot>, String> {
    agent_manager.get_task(task_id)
}

#[tauri::command]
pub async fn ai_agent_list_tasks(
    agent_manager: State<'_, Arc<AgentManager>>,
    session_id: Option<Uuid>,
    limit: Option<u32>,
) -> Result<Vec<AgentTaskSummary>, String> {
    agent_manager.list_tasks(session_id, limit.unwrap_or(20))
}

#[tauri::command]
pub async fn ai_agent_get_task_events(
    agent_manager: State<'_, Arc<AgentManager>>,
    task_id: Uuid,
) -> Result<Vec<AgentEvent>, String> {
    agent_manager.get_task_events(task_id)
}
