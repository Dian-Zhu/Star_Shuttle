use crate::modules::ai::tools::{AgentTool, ToolAuthorization, ToolExecutionResult};
use crate::modules::connection::{ConnectionManager, DefaultConnectionManager};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use uuid::Uuid;

pub struct GetSystemInfoTool {
    connection_manager: Arc<RwLock<DefaultConnectionManager>>,
}

impl GetSystemInfoTool {
    pub fn new(connection_manager: Arc<RwLock<DefaultConnectionManager>>) -> Self {
        Self { connection_manager }
    }
}

#[async_trait]
impl AgentTool for GetSystemInfoTool {
    fn name(&self) -> &'static str {
        "get_system_info"
    }

    fn schema(&self) -> Value {
        json!({
            "name": self.name(),
            "description": "Collect basic OS, CPU, memory and disk information from the remote session.",
            "parameters": {
                "type": "object",
                "properties": {}
            }
        })
    }

    async fn authorize(&self, _args: &Value) -> Result<ToolAuthorization, String> {
        Ok(ToolAuthorization::Allow)
    }

    async fn execute(
        &self,
        session_id: Uuid,
        _args: &Value,
    ) -> Result<ToolExecutionResult, String> {
        let command = "echo '=== OS ===' && uname -a; echo '=== CPU ===' && nproc; echo '=== MEM ===' && free -h; echo '=== DISK ===' && df -h".to_string();
        let cm = self.connection_manager.clone();
        let cmd = command.clone();
        let output = tokio::task::spawn_blocking(move || {
            let mgr = cm.read().map_err(|e| e.to_string())?;
            mgr.exec_command(&session_id, &cmd)
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;

        Ok(ToolExecutionResult {
            title: "获取系统信息".to_string(),
            output,
            command: Some(command),
        })
    }
}
