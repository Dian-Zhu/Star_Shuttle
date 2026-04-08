use crate::modules::ai::{
    client::LlmClient,
    config::load_config,
    skills::{resolve_skill, AiSkill, SkillMode},
    types::{AiConfig, ChatMessage, Conversation, StoredMessage, StreamEvent, TerminalContext},
};
use crate::modules::db::DatabaseManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use uuid::Uuid;

struct InflightRequest {
    conversation_id: Uuid,
    cancel_tx: tokio::sync::oneshot::Sender<()>,
}

const SYSTEM_PROMPT_BASE: &str = r#"You are an expert DevOps and system administration assistant embedded in Star Shuttle, an SSH remote management tool.

You help users:
- Diagnose errors and issues in terminal output
- Suggest and explain shell commands
- Analyze logs and system states
- Provide security and performance recommendations

Guidelines:
- Be concise and actionable
- Always explain what a command does before suggesting it
- Flag potentially destructive operations clearly
- When terminal context is provided, analyze it carefully"#;

/// Chat 会话管理器（线程安全，作为 Tauri managed state）
pub struct ChatManager {
    db: Arc<Mutex<DatabaseManager>>,
    client: LlmClient,
    inflight_requests: Mutex<HashMap<Uuid, InflightRequest>>,
}

impl ChatManager {
    pub fn new(db: Arc<Mutex<DatabaseManager>>) -> Self {
        Self {
            db,
            client: LlmClient::new(),
            inflight_requests: Mutex::new(HashMap::new()),
        }
    }

    pub fn cancel_active_request(&self, conversation_id: Uuid) -> Result<(), String> {
        let inflight = self.inflight_requests.lock().map_err(|e| e.to_string())?;
        let request_id = inflight.iter().find_map(|(request_id, request)| {
            (request.conversation_id == conversation_id).then_some(*request_id)
        });
        drop(inflight);

        let Some(request_id) = request_id else {
            return Ok(());
        };

        let mut inflight = self.inflight_requests.lock().map_err(|e| e.to_string())?;
        if let Some(request) = inflight.remove(&request_id) {
            let _ = request.cancel_tx.send(());
        }
        Ok(())
    }

    /// 新建对话
    pub fn new_conversation(&self, session_id: Option<Uuid>) -> Result<Uuid, String> {
        let id = Uuid::new_v4();
        let db = self.db.lock().map_err(|e| e.to_string())?;
        crate::modules::db::ai_store::create_conversation(
            db.conn(),
            &id,
            "New Chat",
            session_id.as_ref(),
        )
        .map_err(|e| e.to_string())?;
        Ok(id)
    }

    /// 获取所有对话列表
    pub fn list_conversations(&self) -> Result<Vec<Conversation>, String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        let rows = crate::modules::db::ai_store::get_all_conversations(db.conn())
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(
                |(id, title, session_id, created_at, updated_at)| Conversation {
                    id: Uuid::parse_str(&id).unwrap_or_default(),
                    title,
                    session_id: session_id.and_then(|s| Uuid::parse_str(&s).ok()),
                    created_at,
                    updated_at,
                },
            )
            .collect())
    }

    /// 获取对话的消息历史
    pub fn get_messages(&self, conversation_id: Uuid) -> Result<Vec<StoredMessage>, String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        let rows = crate::modules::db::ai_store::get_messages(db.conn(), &conversation_id)
            .map_err(|e| e.to_string())?;
        Ok(rows
            .into_iter()
            .map(
                |(id, role, content, context_snapshot, skill_id, created_at)| StoredMessage {
                    id: Uuid::parse_str(&id).unwrap_or_default(),
                    conversation_id,
                    role,
                    content,
                    context_snapshot,
                    skill_id,
                    created_at,
                },
            )
            .collect())
    }

    /// 清除对话消息（保留对话本身）
    pub fn clear_messages(&self, conversation_id: Uuid) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        crate::modules::db::ai_store::delete_messages(db.conn(), &conversation_id)
            .map_err(|e| e.to_string())
    }

    /// 删除整个对话
    pub fn delete_conversation(&self, conversation_id: Uuid) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        crate::modules::db::ai_store::delete_conversation(db.conn(), &conversation_id)
            .map_err(|e| e.to_string())
    }

    /// 发送消息并流式返回（通过 Tauri event 推送）
    pub async fn send_message(
        &self,
        app: &AppHandle,
        conversation_id: Uuid,
        user_content: String,
        terminal_context: Option<TerminalContext>,
        skill_id: Option<String>,
    ) -> Result<String, String> {
        use tauri::Emitter;

        let config = load_config(&self.db)?;
        let skill = resolve_skill(&self.db, skill_id.as_deref(), SkillMode::Chat)?;

        // 构建历史消息
        let history = self.get_messages(conversation_id)?;

        // 保存用户消息
        let user_msg_id = Uuid::new_v4();
        let context_snapshot = terminal_context.as_ref().map(|c| c.content.clone());
        {
            let db = self.db.lock().map_err(|e| e.to_string())?;
            crate::modules::db::ai_store::save_message(
                db.conn(),
                &user_msg_id,
                &conversation_id,
                "user",
                &user_content,
                context_snapshot.as_deref(),
                skill.as_ref().map(|item| item.summary.id.as_str()),
            )
            .map_err(|e| e.to_string())?;
        }

        // 构建发送给 LLM 的消息列表
        let messages = build_messages(
            &history,
            &user_content,
            &config,
            terminal_context.as_ref(),
            skill.as_ref(),
        );

        // 注册可取消请求
        let request_id = Uuid::new_v4();
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        {
            let mut inflight = self.inflight_requests.lock().map_err(|e| e.to_string())?;
            inflight.insert(
                request_id,
                InflightRequest {
                    conversation_id,
                    cancel_tx,
                },
            );
        }

        // 流式调用
        let event_name = format!("ai-chat-stream-{}", conversation_id);
        let app_clone = app.clone();
        let event_name_clone = event_name.clone();

        let full_response = self
            .client
            .stream_chat(
                &config,
                &messages,
                move |event| {
                    let _ = app_clone.emit(&event_name_clone, &event);
                },
                Some(cancel_rx),
            )
            .await;

        {
            let mut inflight = self.inflight_requests.lock().map_err(|e| e.to_string())?;
            inflight.remove(&request_id);
        }

        let full_response = full_response?;

        // 发送 Done 事件
        let _ = app.emit(&event_name, StreamEvent::Done { conversation_id });

        // 保存 AI 回复
        let ai_msg_id = Uuid::new_v4();
        {
            let db = self.db.lock().map_err(|e| e.to_string())?;
            crate::modules::db::ai_store::save_message(
                db.conn(),
                &ai_msg_id,
                &conversation_id,
                "assistant",
                &full_response,
                None,
                skill.as_ref().map(|item| item.summary.id.as_str()),
            )
            .map_err(|e| e.to_string())?;
        }

        // 如果是第一条消息，自动生成标题
        if history.is_empty() {
            let title = generate_title(&user_content);
            let db = self.db.lock().map_err(|e| e.to_string())?;
            let _ = crate::modules::db::ai_store::update_conversation_title(
                db.conn(),
                &conversation_id,
                &title,
            );
        }

        Ok(full_response)
    }

    /// 测试当前配置的 AI 连接
    pub async fn test_connection(&self) -> Result<(), String> {
        let config = load_config(&self.db)?;
        self.client.test_connection(&config).await
    }
}

/// 构建发送给 LLM 的完整消息列表
fn build_messages(
    history: &[StoredMessage],
    user_content: &str,
    _config: &AiConfig,
    terminal_context: Option<&TerminalContext>,
    skill: Option<&AiSkill>,
) -> Vec<ChatMessage> {
    let mut messages = Vec::new();

    // 系统提示
    let mut system_prompt = SYSTEM_PROMPT_BASE.to_string();
    if let Some(skill) = skill {
        system_prompt.push_str(&format!(
            "\n\n=== Active Skill ===\nName: {}\nDescription: {}\n{}\n=== End Skill ===",
            skill.summary.name, skill.summary.description, skill.system_prompt_fragment
        ));
    }
    if let Some(ctx) = terminal_context {
        system_prompt.push_str(&format!(
            "\n\n=== Terminal Context ===\nHost: {}\n\n{}\n=== End Context ===",
            ctx.host, ctx.content
        ));
    }
    messages.push(ChatMessage::system(system_prompt));

    // 历史消息（最多保留最近 20 条防止超 token）
    let skip = history.len().saturating_sub(20);
    for msg in &history[skip..] {
        let role = match msg.role.as_str() {
            "user" => crate::modules::ai::types::MessageRole::User,
            "assistant" => crate::modules::ai::types::MessageRole::Assistant,
            _ => continue,
        };
        messages.push(ChatMessage {
            role,
            content: msg.content.clone(),
            tool_call_id: None,
            name: None,
        });
    }

    // 当前用户消息
    messages.push(ChatMessage::user(user_content));
    messages
}

/// 从第一条用户消息自动生成对话标题
fn generate_title(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.len() <= 40 {
        trimmed.to_string()
    } else {
        format!("{}...", &trimmed[..37])
    }
}

#[cfg(test)]
mod tests {
    use super::build_messages;
    use crate::modules::ai::{
        skills::{resolve_skill, SkillMode},
        types::{AiConfig, StoredMessage},
    };
    use crate::modules::db::DatabaseManager;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    #[test]
    fn build_messages_includes_skill_prompt_fragment() {
        let history: Vec<StoredMessage> = Vec::new();
        let db = Arc::new(Mutex::new(
            DatabaseManager::new(":memory:").expect("in-memory db"),
        ));
        let skill = resolve_skill(&db, Some("log_diagnostics"), SkillMode::Chat)
            .expect("resolve")
            .expect("skill");

        let messages = build_messages(
            &history,
            "check logs",
            &AiConfig::default(),
            None,
            Some(&skill),
        );

        assert!(messages[0].content.contains("Active Skill"));
        assert!(messages[0].content.contains("日志诊断"));
    }

    #[test]
    fn build_messages_keeps_history_and_user_message() {
        let history = vec![StoredMessage {
            id: Uuid::new_v4(),
            conversation_id: Uuid::new_v4(),
            role: "assistant".to_string(),
            content: "previous".to_string(),
            context_snapshot: None,
            skill_id: None,
            created_at: "2026-04-08T00:00:00Z".to_string(),
        }];

        let messages = build_messages(&history, "next", &AiConfig::default(), None, None);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1].content, "previous");
        assert_eq!(messages[2].content, "next");
    }
}
