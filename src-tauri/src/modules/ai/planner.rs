use crate::modules::ai::{
    agent_types::{PlannerAction, PlannerContext},
    config::load_config,
    types::AiConfig,
};
use crate::modules::db::DatabaseManager;
use reqwest::Client;
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub enum PlannerError {
    EmptyResponse,
    InvalidSchema(String),
    Transport(String),
}

impl PlannerError {
    pub fn message(&self) -> String {
        match self {
            Self::EmptyResponse => "模型返回了空响应".to_string(),
            Self::InvalidSchema(message) => format!("模型返回了无效动作: {}", message),
            Self::Transport(message) => message.clone(),
        }
    }
}

pub struct Planner {
    db: Arc<Mutex<DatabaseManager>>,
    http: Client,
}

impl Planner {
    pub fn new(db: Arc<Mutex<DatabaseManager>>) -> Result<Self, String> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { db, http })
    }

    pub fn load_config(&self) -> Result<AiConfig, String> {
        load_config(&self.db)
    }

    pub async fn plan(
        &self,
        config: &AiConfig,
        context: &PlannerContext,
        tool_schemas: &[serde_json::Value],
    ) -> Result<PlannerAction, PlannerError> {
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
        let system_prompt = "You are the planning brain for an SSH desktop agent.\n\
Return exactly one JSON object with no markdown fences.\n\
Allowed actions:\n\
1. {\"type\":\"tool_call\",\"tool_name\":\"...\",\"args\":{...},\"rationale\":\"...\"}\n\
2. {\"type\":\"complete\",\"summary\":\"...\"}\n\
3. {\"type\":\"fail\",\"reason\":\"...\"}\n\
Rules:\n\
- Use complete only when the task is actually finished and summary is non-empty.\n\
- Use fail when the task cannot continue safely or meaningfully.\n\
- Do not repeat a rejected command without materially changing it.\n\
- Prefer read-only inspection first.";

        let user_prompt = json!({
            "task": {
                "instruction": context.instruction,
                "session_id": context.session_id,
                "sandbox_mode": context.sandbox_mode,
                "status": context.status,
            },
            "available_tools": tool_schemas,
            "prior_steps": context.steps,
            "pending_confirm": context.pending_confirm,
        });

        let body = json!({
            "model": config.model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": user_prompt.to_string() }
            ],
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
            "stream": false
        });

        let mut req = self.http.post(&url).header("Content-Type", "application/json").json(&body);
        if !config.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", config.api_key));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| PlannerError::Transport(format!("HTTP error: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(PlannerError::Transport(format!(
                "LLM API error {}: {}",
                status, text
            )));
        }

        let json: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PlannerError::Transport(format!("响应解析失败: {}", e)))?;
        let content = extract_message_content(
            json.get("choices")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|c| c.get("message"))
                .ok_or(PlannerError::EmptyResponse)?,
        );

        parse_planner_action(&content)
    }
}

fn extract_message_content(message: &serde_json::Value) -> String {
    if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
        return content.to_string();
    }

    if let Some(parts) = message.get("content").and_then(|v| v.as_array()) {
        let mut combined = String::new();
        for part in parts {
            if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                combined.push_str(text);
            }
        }
        return combined;
    }

    String::new()
}

fn strip_code_fence(input: &str) -> &str {
    let trimmed = input.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        return stripped.trim().trim_end_matches("```").trim();
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        return stripped.trim().trim_end_matches("```").trim();
    }
    trimmed
}

fn parse_planner_action(content: &str) -> Result<PlannerAction, PlannerError> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Err(PlannerError::EmptyResponse);
    }

    let candidate = strip_code_fence(trimmed);
    serde_json::from_str::<PlannerAction>(candidate)
        .map_err(|e| PlannerError::InvalidSchema(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_json_code_fence() {
        let text = "```json\n{\"type\":\"fail\",\"reason\":\"x\"}\n```";
        assert_eq!(strip_code_fence(text), "{\"type\":\"fail\",\"reason\":\"x\"}");
    }

    #[test]
    fn parses_complete_action() {
        let action = parse_planner_action("{\"type\":\"complete\",\"summary\":\"done\"}")
            .expect("parse action");
        match action {
            PlannerAction::Complete { summary } => assert_eq!(summary, "done"),
            _ => panic!("unexpected action"),
        }
    }

    #[test]
    fn rejects_empty_action_payload() {
        let err = parse_planner_action("   ").expect_err("should fail");
        assert!(matches!(err, PlannerError::EmptyResponse));
    }

    #[test]
    fn rejects_invalid_schema_payload() {
        let err = parse_planner_action("{\"type\":\"complete\"}").expect_err("should fail");
        match err {
            PlannerError::InvalidSchema(message) => {
                assert!(message.contains("summary"));
            }
            _ => panic!("unexpected error"),
        }
    }
}
