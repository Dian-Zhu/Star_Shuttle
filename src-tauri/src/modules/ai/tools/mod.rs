pub mod execute_command;
pub mod get_system_info;

use crate::modules::ai::sandbox::SandboxMode;
use crate::modules::connection::DefaultConnectionManager;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub enum ToolAuthorization {
    Allow,
    NeedConfirm {
        reason: String,
        risk_level: String,
        command: String,
    },
    Deny {
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct ToolExecutionResult {
    pub title: String,
    pub output: String,
    pub command: Option<String>,
}

#[async_trait]
pub trait AgentTool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> Value;
    async fn authorize(&self, args: &Value) -> Result<ToolAuthorization, String>;
    async fn execute(&self, session_id: Uuid, args: &Value) -> Result<ToolExecutionResult, String>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn AgentTool>>,
}

impl ToolRegistry {
    pub fn new(
        connection_manager: Arc<RwLock<DefaultConnectionManager>>,
        sandbox_mode: SandboxMode,
        allowed_tools: Option<&[String]>,
    ) -> Self {
        let mut tools: HashMap<String, Arc<dyn AgentTool>> = HashMap::new();
        let execute_command = Arc::new(execute_command::ExecuteCommandTool::new(
            connection_manager.clone(),
            sandbox_mode,
        ));
        let get_system_info = Arc::new(get_system_info::GetSystemInfoTool::new(connection_manager));

        let tool_list = [
            (
                execute_command.name().to_string(),
                execute_command as Arc<dyn AgentTool>,
            ),
            (
                get_system_info.name().to_string(),
                get_system_info as Arc<dyn AgentTool>,
            ),
        ];

        for (name, tool) in tool_list {
            if allowed_tools
                .map(|items| items.iter().any(|item| item == &name))
                .unwrap_or(true)
            {
                tools.insert(name, tool);
            }
        }

        Self { tools }
    }

    pub fn schemas(&self) -> Vec<Value> {
        self.tools.values().map(|tool| tool.schema()).collect()
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn AgentTool>> {
        self.tools.get(name).cloned()
    }
}
