use crate::modules::ai::sandbox::{Sandbox, SandboxMode, SandboxVerdict};
use crate::modules::ai::tools::{AgentTool, ToolAuthorization, ToolExecutionResult};
use crate::modules::connection::{ConnectionManager, DefaultConnectionManager};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub struct ExecuteCommandTool {
    connection_manager: Option<Arc<RwLock<DefaultConnectionManager>>>,
    sandbox: Sandbox,
}

impl ExecuteCommandTool {
    pub fn new(
        connection_manager: Arc<RwLock<DefaultConnectionManager>>,
        sandbox_mode: SandboxMode,
    ) -> Self {
        Self {
            connection_manager: Some(connection_manager),
            sandbox: Sandbox::new(sandbox_mode),
        }
    }
}

#[async_trait]
impl AgentTool for ExecuteCommandTool {
    fn name(&self) -> &'static str {
        "execute_command"
    }

    fn schema(&self) -> Value {
        json!({
            "name": self.name(),
            "description": "Execute a shell command on the remote terminal session.",
            "parameters": {
                "type": "object",
                "properties": {
                    "command": { "type": "string" },
                    "reason": { "type": "string" }
                },
                "required": ["command", "reason"]
            }
        })
    }

    async fn authorize(&self, args: &Value) -> Result<ToolAuthorization, String> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("execute_command 缺少 command 参数".to_string())?;

        Ok(match self.sandbox.check(command) {
            SandboxVerdict::Allow => ToolAuthorization::Allow,
            SandboxVerdict::Deny { reason } => ToolAuthorization::Deny { reason },
            SandboxVerdict::NeedConfirm {
                reason,
                risk_level,
                matched_command: _,
            } => ToolAuthorization::NeedConfirm {
                reason,
                risk_level: format!("{:?}", risk_level).to_lowercase(),
                command: command.to_string(),
            },
        })
    }

    async fn execute(&self, session_id: Uuid, args: &Value) -> Result<ToolExecutionResult, String> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or("execute_command 缺少 command 参数".to_string())?
            .to_string();

        let cm = self
            .connection_manager
            .clone()
            .ok_or("execute_command 缺少连接管理器".to_string())?;
        let cmd = command.clone();
        let output = tokio::task::spawn_blocking(move || {
            let mgr = cm.read().map_err(|e| e.to_string())?;
            mgr.exec_command(&session_id, &cmd).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;

        Ok(ToolExecutionResult {
            title: format!("执行：{}", command),
            output,
            command: Some(command),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tool(mode: SandboxMode) -> ExecuteCommandTool {
        ExecuteCommandTool {
            connection_manager: None,
            sandbox: Sandbox::new(mode),
        }
    }

    #[test]
    fn authorize_allows_safe_command() {
        let tool = tool(SandboxMode::Standard);
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
        let result = runtime
            .block_on(tool.authorize(&json!({ "command": "ls -la", "reason": "inspect directory" })))
            .expect("authorize");

        assert!(matches!(result, ToolAuthorization::Allow));
    }

    #[test]
    fn authorize_requests_confirmation_for_sensitive_command() {
        let tool = tool(SandboxMode::Standard);
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
        let result = runtime
            .block_on(tool.authorize(&json!({ "command": "apt install nginx", "reason": "install package" })))
            .expect("authorize");

        match result {
            ToolAuthorization::NeedConfirm { command, .. } => {
                assert_eq!(command, "apt install nginx");
            }
            _ => panic!("expected confirmation"),
        }
    }

    #[test]
    fn authorize_denies_destructive_command() {
        let tool = tool(SandboxMode::Standard);
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().expect("runtime");
        let result = runtime
            .block_on(tool.authorize(&json!({ "command": "ls $(cat /etc/shadow)", "reason": "bad input" })))
            .expect("authorize");

        assert!(matches!(result, ToolAuthorization::Deny { .. }));
    }
}
