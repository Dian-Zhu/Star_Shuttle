use crate::modules::ai::agent::AgentManager;
use crate::modules::ai::agent_types::{
    AgentStepKind, AgentStepStatus, AgentTaskStatus, PlannerAction,
};
use crate::modules::ai::planner::{Planner, PlannerError};
use crate::modules::ai::sandbox::SandboxMode;
use crate::modules::ai::skills::AiSkill;
use crate::modules::ai::tools::{AgentTool, ToolAuthorization, ToolRegistry};
use std::sync::Arc;
use tauri::AppHandle;
use uuid::Uuid;

const MAX_PLANNER_STEPS: usize = 20;
const MAX_RETRIES: usize = 2;

pub struct Orchestrator {
    manager: Arc<AgentManager>,
    planner: Planner,
    tools: ToolRegistry,
    app: AppHandle,
    task_id: Uuid,
    session_id: Uuid,
    skill: Option<AiSkill>,
    cancel_rx: tokio::sync::oneshot::Receiver<()>,
}

impl Orchestrator {
    pub fn new(
        manager: Arc<AgentManager>,
        app: AppHandle,
        task_id: Uuid,
        session_id: Uuid,
        sandbox_mode: SandboxMode,
        skill: Option<AiSkill>,
        cancel_rx: tokio::sync::oneshot::Receiver<()>,
    ) -> Result<Self, String> {
        let planner = Planner::new(manager.db.clone())?;
        let tools = ToolRegistry::new(
            manager.connection_manager.clone(),
            sandbox_mode,
            skill
                .as_ref()
                .map(|item| item.summary.allowed_tools.as_slice()),
        );
        Ok(Self {
            manager,
            planner,
            tools,
            app,
            task_id,
            session_id,
            skill,
            cancel_rx,
        })
    }

    pub async fn run(mut self) -> Result<(), String> {
        let config = self.planner.load_config()?;
        let mut planner_steps = 0usize;
        let mut retries = 0usize;

        loop {
            if self.cancel_rx.try_recv().is_ok() {
                self.manager.cancel_task_final(&self.app, self.task_id)?;
                return Ok(());
            }

            self.manager.check_cancelled(self.task_id, &self.app)?;

            if planner_steps >= MAX_PLANNER_STEPS {
                self.manager.fail_task(
                    &self.app,
                    self.task_id,
                    "max_steps_exceeded",
                    "已达最大规划步骤限制，任务未能生成最终结果。",
                )?;
                return Ok(());
            }

            self.manager.set_status(
                &self.app,
                self.task_id,
                AgentTaskStatus::Planning,
                "task_status_changed",
                serde_json::json!({ "status": "planning" }),
            )?;

            let planning_step = self.manager.add_step(
                self.task_id,
                AgentStepKind::Planning,
                "规划下一步".to_string(),
                None,
                None,
                AgentStepStatus::Running,
                None,
            )?;

            let context = self.manager.build_planner_context(self.task_id)?;
            let action = self
                .planner
                .plan(
                    &config,
                    &context,
                    &self.tools.schemas(),
                    self.skill.as_ref(),
                )
                .await;

            let action = match action {
                Ok(action) => {
                    retries = 0;
                    self.manager.finish_step(
                        self.task_id,
                        planning_step.id,
                        AgentStepStatus::Completed,
                        Some(match &action {
                            PlannerAction::ToolCall {
                                tool_name,
                                rationale,
                                ..
                            } => format!("调用工具 {}: {}", tool_name, rationale),
                            PlannerAction::Complete { summary } => summary.clone(),
                            PlannerAction::Fail { reason } => reason.clone(),
                        }),
                    )?;
                    self.manager.append_event(
                        &self.app,
                        self.task_id,
                        "planner_action",
                        serde_json::to_value(&action).map_err(|e| e.to_string())?,
                    )?;
                    action
                }
                Err(error) => {
                    self.manager.finish_step(
                        self.task_id,
                        planning_step.id,
                        AgentStepStatus::Failed,
                        Some(error.message()),
                    )?;

                    if retries < MAX_RETRIES {
                        retries += 1;
                        self.manager.set_status(
                            &self.app,
                            self.task_id,
                            AgentTaskStatus::Retrying,
                            "task_retrying",
                            serde_json::json!({
                                "attempt": retries,
                                "reason": error.message(),
                            }),
                        )?;
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        continue;
                    }

                    self.manager.fail_task(
                        &self.app,
                        self.task_id,
                        planner_error_code(&error),
                        &error.message(),
                    )?;
                    return Ok(());
                }
            };

            planner_steps += 1;

            match action {
                PlannerAction::ToolCall {
                    tool_name,
                    args,
                    rationale,
                } => {
                    let Some(tool) = self.tools.get(&tool_name) else {
                        self.manager.fail_task(
                            &self.app,
                            self.task_id,
                            "unknown_tool",
                            &format!("未知工具: {}", tool_name),
                        )?;
                        return Ok(());
                    };

                    match tool.authorize(&args).await? {
                        ToolAuthorization::Allow => {
                            self.execute_tool(&tool_name, rationale, args, tool).await?;
                        }
                        ToolAuthorization::Deny { reason } => {
                            let step = self.manager.add_step(
                                self.task_id,
                                AgentStepKind::ToolExecution,
                                format!("已拒绝工具 {} 的执行", tool_name),
                                Some(tool_name.clone()),
                                args.get("command")
                                    .and_then(|v| v.as_str())
                                    .map(|v| v.to_string()),
                                AgentStepStatus::Rejected,
                                None,
                            )?;
                            self.manager.finish_step(
                                self.task_id,
                                step.id,
                                AgentStepStatus::Rejected,
                                Some(reason.clone()),
                            )?;
                            self.manager.append_event(
                                &self.app,
                                self.task_id,
                                "tool_rejected_by_sandbox",
                                serde_json::json!({
                                    "tool_name": tool_name,
                                    "reason": reason,
                                }),
                            )?;
                        }
                        ToolAuthorization::NeedConfirm {
                            reason,
                            risk_level,
                            command,
                        } => {
                            let step = self.manager.add_step(
                                self.task_id,
                                AgentStepKind::Confirmation,
                                format!("等待确认：{}", rationale),
                                Some(tool_name.clone()),
                                Some(command.clone()),
                                AgentStepStatus::Running,
                                Some(risk_level.clone()),
                            )?;

                            let confirm_rx = self.manager.prepare_confirmation(
                                &self.app,
                                self.task_id,
                                step.id,
                                command.clone(),
                                reason.clone(),
                                risk_level.clone(),
                            )?;

                            let decision = tokio::select! {
                                _ = &mut self.cancel_rx => {
                                    self.manager.cancel_task_final(&self.app, self.task_id)?;
                                    return Ok(());
                                }
                                result = tokio::time::timeout(tokio::time::Duration::from_secs(300), confirm_rx) => {
                                    match result {
                                        Ok(Ok(value)) => Some(value),
                                        _ => None,
                                    }
                                }
                            };

                            self.manager.clear_pending_confirm(self.task_id)?;

                            match decision {
                                Some(true) => {
                                    self.manager.finish_step(
                                        self.task_id,
                                        step.id,
                                        AgentStepStatus::Completed,
                                        Some("用户已确认执行".to_string()),
                                    )?;
                                    self.execute_tool(&tool_name, rationale, args, tool).await?;
                                }
                                Some(false) => {
                                    self.manager.finish_step(
                                        self.task_id,
                                        step.id,
                                        AgentStepStatus::Rejected,
                                        Some("用户拒绝执行该命令".to_string()),
                                    )?;
                                    self.manager.append_event(
                                        &self.app,
                                        self.task_id,
                                        "confirmation_rejected",
                                        serde_json::json!({
                                            "tool_name": tool_name,
                                            "command": command,
                                        }),
                                    )?;
                                }
                                None => {
                                    self.manager.finish_step(
                                        self.task_id,
                                        step.id,
                                        AgentStepStatus::Rejected,
                                        Some("等待确认超时，已拒绝执行".to_string()),
                                    )?;
                                    self.manager.append_event(
                                        &self.app,
                                        self.task_id,
                                        "confirmation_timed_out",
                                        serde_json::json!({
                                            "tool_name": tool_name,
                                            "command": command,
                                        }),
                                    )?;
                                }
                            }
                        }
                    }
                }
                PlannerAction::Complete { summary } => {
                    if summary.trim().is_empty() {
                        self.manager.fail_task(
                            &self.app,
                            self.task_id,
                            "empty_summary",
                            "模型尝试完成任务，但没有提供有效总结。",
                        )?;
                    } else {
                        self.manager
                            .complete_task(&self.app, self.task_id, summary.trim())?;
                    }
                    return Ok(());
                }
                PlannerAction::Fail { reason } => {
                    self.manager.fail_task(
                        &self.app,
                        self.task_id,
                        "planner_failed",
                        reason.trim(),
                    )?;
                    return Ok(());
                }
            }
        }
    }

    async fn execute_tool(
        &mut self,
        tool_name: &str,
        rationale: String,
        args: serde_json::Value,
        tool: Arc<dyn AgentTool>,
    ) -> Result<(), String> {
        self.manager.check_cancelled(self.task_id, &self.app)?;
        self.manager.set_status(
            &self.app,
            self.task_id,
            AgentTaskStatus::Executing,
            "task_status_changed",
            serde_json::json!({ "status": "executing", "tool_name": tool_name }),
        )?;

        let step = self.manager.add_step(
            self.task_id,
            AgentStepKind::ToolExecution,
            rationale,
            Some(tool_name.to_string()),
            args.get("command")
                .and_then(|v| v.as_str())
                .map(|v| v.to_string()),
            AgentStepStatus::Running,
            None,
        )?;

        let result = tool.execute(self.session_id, &args).await;
        if self.manager.is_cancelling(self.task_id)? {
            self.manager.cancel_task_final(&self.app, self.task_id)?;
            return Ok(());
        }

        match result {
            Ok(output) => {
                self.manager.finish_step(
                    self.task_id,
                    step.id,
                    AgentStepStatus::Completed,
                    Some(output.output.clone()),
                )?;
                self.manager
                    .mark_last_successful_step(self.task_id, step.id)?;
                self.manager.append_event(
                    &self.app,
                    self.task_id,
                    "tool_completed",
                    serde_json::json!({
                        "tool_name": tool_name,
                        "title": output.title,
                        "command": output.command,
                    }),
                )?;
            }
            Err(error) => {
                self.manager.finish_step(
                    self.task_id,
                    step.id,
                    AgentStepStatus::Failed,
                    Some(error.clone()),
                )?;
                self.manager.append_event(
                    &self.app,
                    self.task_id,
                    "tool_failed",
                    serde_json::json!({
                        "tool_name": tool_name,
                        "error": error,
                    }),
                )?;
            }
        }

        Ok(())
    }
}

fn planner_error_code(error: &PlannerError) -> &'static str {
    match error {
        PlannerError::EmptyResponse => "planner_empty_response",
        PlannerError::InvalidSchema(_) => "planner_invalid_schema",
        PlannerError::Transport(_) => "planner_transport_error",
    }
}

#[cfg(test)]
mod tests {
    use super::planner_error_code;
    use crate::modules::ai::planner::PlannerError;

    #[test]
    fn maps_empty_response_error_code() {
        assert_eq!(
            planner_error_code(&PlannerError::EmptyResponse),
            "planner_empty_response"
        );
    }

    #[test]
    fn maps_invalid_schema_error_code() {
        assert_eq!(
            planner_error_code(&PlannerError::InvalidSchema("bad".to_string())),
            "planner_invalid_schema"
        );
    }

    #[test]
    fn maps_transport_error_code() {
        assert_eq!(
            planner_error_code(&PlannerError::Transport("timeout".to_string())),
            "planner_transport_error"
        );
    }
}
