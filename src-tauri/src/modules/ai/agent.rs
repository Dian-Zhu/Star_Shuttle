use crate::modules::ai::{
    client::LlmClient,
    config::load_config,
    sandbox::{Sandbox, SandboxMode, SandboxVerdict},
    types::{AiConfig, ChatMessage},
};
use crate::modules::connection::{ConnectionManager, DefaultConnectionManager};
use crate::modules::db::DatabaseManager;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tauri::AppHandle;
use uuid::Uuid;

// ── 类型定义 ──────────────────────────────────────────────────────────────────

/// Agent 任务状态
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Running,
    WaitingConfirm,
    Completed,
    Failed,
    Cancelled,
}

/// 单步操作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    Thinking,
    ExecuteCommand,
    ReadFile,
    ListDirectory,
    GetSystemInfo,
    AwaitingConfirm,
    Result,
}

/// Agent 执行步骤（前端展示用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub id: Uuid,
    pub kind: StepKind,
    pub description: String,
    pub command: Option<String>,
    pub output: Option<String>,
    pub status: StepStatus,
    pub risk_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    WaitingConfirm,
    Confirmed,
    Rejected,
    Completed,
    Failed,
}

/// 待确认命令信息（传给前端的弹窗数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingConfirm {
    pub task_id: Uuid,
    pub step_id: Uuid,
    pub command: String,
    pub reason: String,
    pub risk_level: String,
}

/// Agent 任务
#[derive(Debug, Clone, Serialize)]
pub struct AgentTask {
    pub id: Uuid,
    pub session_id: Uuid,
    pub instruction: String,
    pub sandbox_mode: SandboxMode,
    pub status: AgentStatus,
    pub steps: Vec<AgentStep>,
    pub pending_confirm: Option<PendingConfirm>,
    pub error: Option<String>,
}

// ── LLM Tool Definitions ──────────────────────────────────────────────────────

const TOOLS_SCHEMA: &str = r#"[
  {
    "type": "function",
    "function": {
      "name": "execute_command",
      "description": "Execute a shell command on the remote terminal. Always prefer read-only commands. Destructive commands require user confirmation.",
      "parameters": {
        "type": "object",
        "properties": {
          "command": { "type": "string", "description": "The shell command to execute" },
          "reason":  { "type": "string", "description": "Why this command is needed" }
        },
        "required": ["command", "reason"]
      }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "get_system_info",
      "description": "Get basic system information (OS, memory, disk, CPU)",
      "parameters": { "type": "object", "properties": {} }
    }
  },
  {
    "type": "function",
    "function": {
      "name": "task_complete",
      "description": "Mark the task as complete with a final summary",
      "parameters": {
        "type": "object",
        "properties": {
          "summary": { "type": "string", "description": "Final summary of what was done" }
        },
        "required": ["summary"]
      }
    }
  }
]"#;

// ── AgentManager ──────────────────────────────────────────────────────────────

pub struct AgentManager {
    db: Arc<Mutex<DatabaseManager>>,
    connection_manager: Arc<RwLock<DefaultConnectionManager>>,
    tasks: Mutex<HashMap<Uuid, AgentTask>>,
    // channel to send confirm results back to running tasks
    confirm_senders: Mutex<HashMap<Uuid, tokio::sync::oneshot::Sender<bool>>>,
    // channel to cancel a running task (drops = cancelled)
    cancel_senders: Mutex<HashMap<Uuid, tokio::sync::oneshot::Sender<()>>>,
}

impl AgentManager {
    pub fn new(
        db: Arc<Mutex<DatabaseManager>>,
        connection_manager: Arc<RwLock<DefaultConnectionManager>>,
    ) -> Self {
        Self {
            db,
            connection_manager,
            tasks: Mutex::new(HashMap::new()),
            confirm_senders: Mutex::new(HashMap::new()),
            cancel_senders: Mutex::new(HashMap::new()),
        }
    }

    pub fn get_task(&self, task_id: Uuid) -> Option<AgentTask> {
        self.tasks.lock().ok()?.get(&task_id).cloned()
    }

    pub fn cancel_task(&self, task_id: Uuid) -> Result<(), String> {
        // Mark status
        {
            let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
            if let Some(task) = tasks.get_mut(&task_id) {
                task.status = AgentStatus::Cancelled;
                task.pending_confirm = None;
            }
        }
        // Unblock any pending confirm (send false so loop can exit)
        if let Ok(mut senders) = self.confirm_senders.lock() {
            if let Some(tx) = senders.remove(&task_id) {
                let _ = tx.send(false);
            }
        }
        // Signal cancellation to the run loop
        if let Ok(mut senders) = self.cancel_senders.lock() {
            if let Some(tx) = senders.remove(&task_id) {
                let _ = tx.send(());
            }
        }
        Ok(())
    }

    /// 用户确认/拒绝某步骤的高危命令
    pub fn confirm_step(&self, task_id: Uuid, confirmed: bool) -> Result<(), String> {
        let mut senders = self.confirm_senders.lock().map_err(|e| e.to_string())?;
        if let Some(sender) = senders.remove(&task_id) {
            let _ = sender.send(confirmed);
        }
        Ok(())
    }

    /// 启动 Agent 任务（异步，后台运行）
    pub async fn start_task(
        self: Arc<Self>,
        app: AppHandle,
        session_id: Uuid,
        instruction: String,
        sandbox_mode: SandboxMode,
    ) -> Result<Uuid, String> {
        let task_id = Uuid::new_v4();
        let task = AgentTask {
            id: task_id,
            session_id,
            instruction: instruction.clone(),
            sandbox_mode,
            status: AgentStatus::Running,
            steps: Vec::new(),
            pending_confirm: None,
            error: None,
        };

        {
            let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
            tasks.insert(task_id, task);
        }

        // 注册 cancel channel
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        {
            let mut senders = self.cancel_senders.lock().map_err(|e| e.to_string())?;
            senders.insert(task_id, cancel_tx);
        }

        // 发布初始状态事件
        self.emit_status(&app, task_id);

        // 异步执行 Agent 循环
        let manager = self.clone();
        let app_clone = app.clone();
        tokio::spawn(async move {
            if let Err(e) = manager.run_agent_loop(&app_clone, task_id, &instruction, sandbox_mode, session_id, cancel_rx).await {
                let mut tasks = manager.tasks.lock().unwrap();
                if let Some(task) = tasks.get_mut(&task_id) {
                    task.status = AgentStatus::Failed;
                    task.error = Some(e.clone());
                }
                manager.emit_status(&app_clone, task_id);
            }
        });

        Ok(task_id)
    }

    /// Agent 主执行循环
    async fn run_agent_loop(
        &self,
        app: &AppHandle,
        task_id: Uuid,
        instruction: &str,
        sandbox_mode: SandboxMode,
        session_id: Uuid,
        mut cancel_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<(), String> {
        let config = load_config(&self.db)?;
        let client = LlmClient::new();
        let sandbox = Sandbox::new(sandbox_mode);

        // 构建初始消息
        let system_prompt = format!(
            r#"You are an autonomous DevOps agent embedded in Star Shuttle SSH manager.
You have tools to execute commands on remote servers. 

Rules:
1. Always explain what you are doing before calling a tool
2. Prefer read-only commands first to gather information
3. Break complex tasks into small steps
4. Stop and summarize when the task is complete
5. If a command fails, analyze the error and try an alternative approach

Current session: {session_id}
Sandbox mode: {mode}"#,
            session_id = session_id,
            mode = match sandbox_mode {
                SandboxMode::Standard => "Standard (whitelist-based)",
                SandboxMode::Strict => "Strict (blacklist-based)",
            }
        );

        let mut messages: Vec<serde_json::Value> = vec![
            serde_json::json!({ "role": "system", "content": system_prompt }),
            serde_json::json!({ "role": "user", "content": instruction }),
        ];

        let tools: serde_json::Value = serde_json::from_str(TOOLS_SCHEMA)
            .map_err(|e| format!("Failed to parse tools schema: {}", e))?;

        let max_steps = 20usize;
        let mut step_count = 0;

        loop {
            // 检查取消信号（非阻塞）
            if cancel_rx.try_recv().is_ok() {
                self.set_task_status(task_id, AgentStatus::Cancelled)?;
                self.emit_status(app, task_id);
                return Ok(());
            }
            // 检查任务状态
            {
                let tasks = self.tasks.lock().map_err(|e| e.to_string())?;
                if let Some(t) = tasks.get(&task_id) {
                    if t.status == AgentStatus::Cancelled {
                        return Ok(());
                    }
                }
            }

            if step_count >= max_steps {
                self.add_step(task_id, StepKind::Result, "已达最大步骤限制，停止执行".to_string(), None, None, StepStatus::Completed);
                self.set_task_status(task_id, AgentStatus::Completed)?;
                self.emit_status(app, task_id);
                return Ok(());
            }

            // 思考步骤
            let think_step_id = self.add_step(task_id, StepKind::Thinking, "AI 正在分析...".to_string(), None, None, StepStatus::Running);
            self.emit_status(app, task_id);

            // 调用 LLM（带 tools），同时监听取消信号
            let llm_fut = self.call_llm_with_tools(&client, &config, &messages, &tools);
            let response = tokio::select! {
                res = llm_fut => res?,
                _ = &mut cancel_rx => {
                    self.update_step(task_id, think_step_id, StepStatus::Failed, Some("已取消".to_string()));
                    self.set_task_status(task_id, AgentStatus::Cancelled)?;
                    self.emit_status(app, task_id);
                    return Ok(());
                }
            };

            // 解析响应
            let response_msg = response.clone();
            messages.push(response_msg);

            // 检查是否有 tool_calls
            if let Some(tool_calls) = response.get("tool_calls").and_then(|v| v.as_array()) {
                if tool_calls.is_empty() {
                    // 纯文本回复，视为任务总结
                    let content = response.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    self.update_step(task_id, think_step_id, StepStatus::Completed, Some(content.clone()));
                    self.add_step(task_id, StepKind::Result, content, None, None, StepStatus::Completed);
                    self.set_task_status(task_id, AgentStatus::Completed)?;
                    self.emit_status(app, task_id);
                    return Ok(());
                }

                self.update_step(task_id, think_step_id, StepStatus::Completed, None);

                // 逐个处理 tool calls
                for tool_call in tool_calls {
                    let tool_name = tool_call.get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    let args_str = tool_call.get("function")
                        .and_then(|f| f.get("arguments"))
                        .and_then(|a| a.as_str())
                        .unwrap_or("{}");
                    let tool_call_id = tool_call.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let args: serde_json::Value = serde_json::from_str(args_str).unwrap_or(serde_json::json!({}));

                    let tool_result = match tool_name {
                        "execute_command" => {
                            let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                            let reason = args.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                            self.handle_execute_command(app, task_id, session_id, command, reason, &sandbox).await?
                        }
                        "get_system_info" => {
                            self.handle_get_system_info(task_id, session_id).await?
                        }
                        "task_complete" => {
                            let summary = args.get("summary").and_then(|v| v.as_str()).unwrap_or("Task completed");
                            self.add_step(task_id, StepKind::Result, summary.to_string(), None, Some(summary.to_string()), StepStatus::Completed);
                            self.set_task_status(task_id, AgentStatus::Completed)?;
                            self.emit_status(app, task_id);
                            return Ok(());
                        }
                        _ => format!("Unknown tool: {}", tool_name),
                    };

                    // 任务可能被取消/失败
                    {
                        let tasks = self.tasks.lock().map_err(|e| e.to_string())?;
                        if let Some(t) = tasks.get(&task_id) {
                            if t.status == AgentStatus::Cancelled || t.status == AgentStatus::Failed {
                                return Ok(());
                            }
                        }
                    }

                    self.emit_status(app, task_id);

                    // 将工具结果加入消息历史
                    messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": tool_call_id,
                        "content": tool_result,
                    }));
                }
            } else {
                // 纯文本回复
                let content = response.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                self.update_step(task_id, think_step_id, StepStatus::Completed, Some(content.clone()));
                self.add_step(task_id, StepKind::Result, content, None, None, StepStatus::Completed);
                self.set_task_status(task_id, AgentStatus::Completed)?;
                self.emit_status(app, task_id);
                return Ok(());
            }

            step_count += 1;
        }
    }

    /// 处理 execute_command 工具调用（含沙箱检查）
    async fn handle_execute_command(
        &self,
        app: &AppHandle,
        task_id: Uuid,
        session_id: Uuid,
        command: &str,
        reason: &str,
        sandbox: &Sandbox,
    ) -> Result<String, String> {
        // 沙箱检查
        let verdict = sandbox.check(command);

        match verdict {
            SandboxVerdict::Deny { reason: deny_reason } => {
                let step_id = self.add_step(
                    task_id,
                    StepKind::ExecuteCommand,
                    format!("拒绝执行：{}", deny_reason),
                    Some(command.to_string()),
                    Some(format!("DENIED: {}", deny_reason)),
                    StepStatus::Rejected,
                );
                return Ok(format!("Command was denied by sandbox: {}", deny_reason));
            }

            SandboxVerdict::NeedConfirm { reason: confirm_reason, risk_level, matched_command } => {
                let step_id = Uuid::new_v4();
                let risk_str = format!("{:?}", risk_level).to_lowercase();

                // 记录等待确认的步骤
                let confirm_info = PendingConfirm {
                    task_id,
                    step_id,
                    command: command.to_string(),
                    reason: confirm_reason.clone(),
                    risk_level: risk_str.clone(),
                };

                {
                    let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
                    if let Some(task) = tasks.get_mut(&task_id) {
                        task.status = AgentStatus::WaitingConfirm;
                        task.pending_confirm = Some(confirm_info.clone());
                        task.steps.push(AgentStep {
                            id: step_id,
                            kind: StepKind::AwaitingConfirm,
                            description: format!("等待确认：{}", confirm_reason),
                            command: Some(command.to_string()),
                            output: None,
                            status: StepStatus::WaitingConfirm,
                            risk_level: Some(risk_str),
                        });
                    }
                }

                // 通知前端显示确认弹窗
                self.emit_status(app, task_id);
                self.emit_confirm_request(app, &confirm_info);

                // 等待用户确认（最多 5 分钟）
                let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
                {
                    let mut senders = self.confirm_senders.lock().map_err(|e| e.to_string())?;
                    senders.insert(task_id, tx);
                }

                let confirmed = tokio::time::timeout(
                    tokio::time::Duration::from_secs(300),
                    rx,
                )
                .await
                .unwrap_or(Ok(false))
                .unwrap_or(false);

                // 恢复状态（检查是否被取消）
                {
                    let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
                    if let Some(task) = tasks.get_mut(&task_id) {
                        if task.status == AgentStatus::Cancelled {
                            return Ok("Cancelled".to_string());
                        }
                        task.status = AgentStatus::Running;
                        task.pending_confirm = None;
                        if let Some(step) = task.steps.iter_mut().find(|s| s.id == step_id) {
                            step.status = if confirmed { StepStatus::Confirmed } else { StepStatus::Rejected };
                        }
                    }
                }

                if !confirmed {
                    return Ok("User rejected the command. Do not retry this command.".to_string());
                }

                // 执行命令
                self.execute_command_on_session(task_id, session_id, command).await
            }

            SandboxVerdict::Allow => {
                self.execute_command_on_session(task_id, session_id, command).await
            }
        }
    }

    /// 实际执行命令（沙箱已放行）
    async fn execute_command_on_session(
        &self,
        task_id: Uuid,
        session_id: Uuid,
        command: &str,
    ) -> Result<String, String> {
        let step_id = self.add_step(
            task_id,
            StepKind::ExecuteCommand,
            format!("执行：{}", command),
            Some(command.to_string()),
            None,
            StepStatus::Running,
        );

        // exec_command is sync and uses block_on internally — must run on a blocking thread
        let cm = self.connection_manager.clone();
        let cmd = command.to_string();
        let result = tokio::task::spawn_blocking(move || {
            let mgr = cm.read().map_err(|e| e.to_string())?;
            mgr.exec_command(&session_id, &cmd)
                .map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())?;

        // 记录审计日志
        let _ = self.log_command_audit(task_id, session_id, command, result.as_deref().ok());

        match result {
            Ok(output) => {
                self.update_step(task_id, step_id, StepStatus::Completed, Some(output.clone()));
                Ok(output)
            }
            Err(e) => {
                self.update_step(task_id, step_id, StepStatus::Failed, Some(e.clone()));
                Ok(format!("Command failed: {}", e))
            }
        }
    }

    async fn handle_get_system_info(&self, task_id: Uuid, session_id: Uuid) -> Result<String, String> {
        let cmd = "echo '=== OS ===' && uname -a; echo '=== CPU ===' && nproc; echo '=== MEM ===' && free -h; echo '=== DISK ===' && df -h";
        self.execute_command_on_session(task_id, session_id, cmd).await
    }

    // ── LLM 调用 ──────────────────────────────────────────────────────────────

    async fn call_llm_with_tools(
        &self,
        client: &LlmClient,
        config: &AiConfig,
        messages: &[serde_json::Value],
        tools: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let url = format!("{}/chat/completions", config.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": config.model,
            "messages": messages,
            "tools": tools,
            "tool_choice": "auto",
            "temperature": config.temperature,
            "max_tokens": config.max_tokens,
        });

        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| e.to_string())?;

        let mut req = http.post(&url).header("Content-Type", "application/json").json(&body);
        if !config.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", config.api_key));
        }

        let resp = req.send().await.map_err(|e| format!("HTTP error: {}", e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("LLM API error {}: {}", status, text));
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        let choice = json.get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .and_then(|c| c.get("message"))
            .cloned()
            .ok_or("Empty response from LLM")?;
        Ok(choice)
    }

    // ── 辅助方法 ──────────────────────────────────────────────────────────────

    fn add_step(
        &self,
        task_id: Uuid,
        kind: StepKind,
        description: String,
        command: Option<String>,
        output: Option<String>,
        status: StepStatus,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(&task_id) {
            task.steps.push(AgentStep { id, kind, description, command, output, status, risk_level: None });
        }
        id
    }

    fn update_step(&self, task_id: Uuid, step_id: Uuid, status: StepStatus, output: Option<String>) {
        let mut tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.get_mut(&task_id) {
            if let Some(step) = task.steps.iter_mut().find(|s| s.id == step_id) {
                step.status = status;
                if output.is_some() {
                    step.output = output;
                }
            }
        }
    }

    fn set_task_status(&self, task_id: Uuid, status: AgentStatus) -> Result<(), String> {
        let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
        if let Some(task) = tasks.get_mut(&task_id) {
            task.status = status;
        }
        Ok(())
    }

    fn emit_status(&self, app: &AppHandle, task_id: Uuid) {
        use tauri::Emitter;
        let tasks = self.tasks.lock().unwrap();
        if let Some(task) = tasks.get(&task_id) {
            let _ = app.emit(&format!("ai-agent-status-{}", task_id), task);
        }
    }

    fn emit_confirm_request(&self, app: &AppHandle, info: &PendingConfirm) {
        use tauri::Emitter;
        let _ = app.emit("ai-agent-confirm-request", info);
    }

    fn log_command_audit(
        &self,
        task_id: Uuid,
        session_id: Uuid,
        command: &str,
        output: Option<&str>,
    ) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        crate::modules::db::ai_store::save_command_audit(
            db.conn(),
            &Uuid::new_v4(),
            &task_id,
            &session_id,
            command,
            output,
        )
        .map_err(|e| e.to_string())
    }
}
