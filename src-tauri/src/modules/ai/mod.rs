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
use crate::{ensure_app_unlocked_runtime, AppLockRuntimeState};
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
pub async fn ai_get_config(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
) -> Result<AiConfig, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    load_config(db.inner())
}

#[tauri::command]
pub async fn ai_save_config(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    config: AiConfig,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
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
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    chat_manager: State<'_, Arc<ChatManager>>,
    config: Option<AiConfig>,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    chat_manager.test_connection(config).await
}

#[tauri::command]
pub async fn ai_list_skills(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    mode: Option<String>,
) -> Result<Vec<AiSkillSummary>, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    let mode = parse_skill_mode(mode.as_deref())?;
    list_skills(db.inner(), mode)
}

#[tauri::command]
pub async fn ai_list_installed_skills(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    mode: Option<String>,
) -> Result<Vec<AiSkillSummary>, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    let mode = parse_skill_mode(mode.as_deref())?;
    list_installed_skills(db.inner(), mode)
}

#[tauri::command]
pub async fn ai_install_skill_from_dir(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    path: String,
) -> Result<AiSkillSummary, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    install_skill_from_dir(db.inner(), std::path::Path::new(&path))
}

#[tauri::command]
pub async fn ai_set_skill_enabled(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    skill_id: String,
    enabled: bool,
) -> Result<AiSkillSummary, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    set_skill_enabled(db.inner(), &skill_id, enabled)
}

#[tauri::command]
pub async fn ai_set_skill_trusted(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    skill_id: String,
    trusted: bool,
) -> Result<AiSkillSummary, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    set_skill_trusted(db.inner(), &skill_id, trusted)
}

#[tauri::command]
pub async fn ai_remove_skill(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    skill_id: String,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    remove_skill(db.inner(), &skill_id)
}

#[tauri::command]
pub async fn ai_reload_skills(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    mode: Option<String>,
) -> Result<Vec<AiSkillSummary>, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    let mode = parse_skill_mode(mode.as_deref())?;
    reload_skills(db.inner(), mode)
}

#[tauri::command]
pub async fn ai_match_skills(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    input: String,
    mode: Option<String>,
) -> Result<AiSkillMatchResult, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    let mode = parse_skill_mode(mode.as_deref())?.unwrap_or(SkillMode::Chat);
    match_skills(db.inner(), &input, mode)
}

// ── Chat Commands ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ai_chat_new(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    chat_manager: State<'_, Arc<ChatManager>>,
    session_id: Option<Uuid>,
) -> Result<Uuid, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    chat_manager.new_conversation(session_id)
}

#[tauri::command]
pub async fn ai_chat_list(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    chat_manager: State<'_, Arc<ChatManager>>,
) -> Result<Vec<Conversation>, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    chat_manager.list_conversations()
}

#[tauri::command]
pub async fn ai_chat_messages(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    chat_manager: State<'_, Arc<ChatManager>>,
    conversation_id: Uuid,
) -> Result<Vec<StoredMessage>, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    chat_manager.get_messages(conversation_id)
}

#[tauri::command]
pub async fn ai_chat_send(
    app: AppHandle,
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    chat_manager: State<'_, Arc<ChatManager>>,
    manager: State<'_, Arc<RwLock<DefaultConnectionManager>>>,
    conversation_id: Uuid,
    content: String,
    session_id: Option<Uuid>,
    include_terminal_context: bool,
    skill_id: Option<String>,
) -> Result<String, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
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
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    chat_manager: State<'_, Arc<ChatManager>>,
    conversation_id: Uuid,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    chat_manager.cancel_active_request(conversation_id)
}

#[tauri::command]
pub async fn ai_chat_clear(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    chat_manager: State<'_, Arc<ChatManager>>,
    conversation_id: Uuid,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    chat_manager.clear_messages(conversation_id)
}

#[tauri::command]
pub async fn ai_chat_delete(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    chat_manager: State<'_, Arc<ChatManager>>,
    conversation_id: Uuid,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    chat_manager.delete_conversation(conversation_id)
}

// ── 终端上下文 Command ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ai_get_terminal_context(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    manager: State<'_, Arc<RwLock<DefaultConnectionManager>>>,
    session_id: Uuid,
    lines: Option<u32>,
) -> Result<crate::modules::ai::types::TerminalContext, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    collect_terminal_context(manager.inner(), session_id, lines.unwrap_or(100))
}

// ── Agent Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn ai_agent_start(
    app: AppHandle,
    agent_manager: State<'_, Arc<AgentManager>>,
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    session_id: Uuid,
    instruction: String,
    sandbox_mode: Option<String>,
    skill_id: Option<String>,
) -> Result<Uuid, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
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
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    agent_manager: State<'_, Arc<AgentManager>>,
    task_id: Uuid,
    confirmed: bool,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    agent_manager.confirm_step(task_id, confirmed)
}

#[tauri::command]
pub async fn ai_agent_cancel(
    app: AppHandle,
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    agent_manager: State<'_, Arc<AgentManager>>,
    task_id: Uuid,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    agent_manager.cancel_task(task_id)?;
    if let Some(task) = agent_manager.get_task(task_id)? {
        use tauri::Emitter;
        let _ = app.emit(&format!("ai-agent-task-{}", task_id), task);
    }
    Ok(())
}

#[tauri::command]
pub async fn ai_agent_get_task(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    agent_manager: State<'_, Arc<AgentManager>>,
    task_id: Uuid,
) -> Result<Option<AgentTaskSnapshot>, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    agent_manager.get_task(task_id)
}

#[tauri::command]
pub async fn ai_agent_list_tasks(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    agent_manager: State<'_, Arc<AgentManager>>,
    session_id: Option<Uuid>,
    limit: Option<u32>,
) -> Result<Vec<AgentTaskSummary>, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    agent_manager.list_tasks(session_id, limit.unwrap_or(20))
}

#[tauri::command]
pub async fn ai_agent_get_task_events(
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    agent_manager: State<'_, Arc<AgentManager>>,
    task_id: Uuid,
) -> Result<Vec<AgentEvent>, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    agent_manager.get_task_events(task_id)
}
