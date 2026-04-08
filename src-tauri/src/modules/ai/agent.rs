use crate::modules::ai::agent_store;
use crate::modules::ai::agent_types::{
    AgentEvent, AgentFinalResult, AgentStep, AgentStepKind, AgentStepStatus, AgentTaskSnapshot,
    AgentTaskStatus, AgentTaskSummary, PendingConfirm, PlannerContext, PlannerContextStep,
    PlannerSkillContext,
};
use crate::modules::ai::orchestrator::Orchestrator;
use crate::modules::ai::sandbox::SandboxMode;
use crate::modules::ai::skills::{resolve_skill, SkillMode};
use crate::modules::connection::DefaultConnectionManager;
use crate::modules::db::DatabaseManager;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tauri::AppHandle;
use uuid::Uuid;

#[derive(Clone)]
struct RuntimeTask {
    snapshot: AgentTaskSnapshot,
    next_step_seq: u32,
    next_event_seq: u32,
    last_successful_step_id: Option<Uuid>,
}

pub struct AgentManager {
    pub(crate) db: Arc<Mutex<DatabaseManager>>,
    pub(crate) connection_manager: Arc<RwLock<DefaultConnectionManager>>,
    tasks: Mutex<HashMap<Uuid, RuntimeTask>>,
    confirm_senders: Mutex<HashMap<Uuid, tokio::sync::oneshot::Sender<bool>>>,
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

    pub fn get_task(&self, task_id: Uuid) -> Result<Option<AgentTaskSnapshot>, String> {
        if let Ok(tasks) = self.tasks.lock() {
            if let Some(task) = tasks.get(&task_id) {
                return Ok(Some(task.snapshot.clone()));
            }
        }

        let db = self.db.lock().map_err(|e| e.to_string())?;
        agent_store::load_task(db.conn(), task_id)
    }

    pub fn list_tasks(
        &self,
        session_id: Option<Uuid>,
        limit: u32,
    ) -> Result<Vec<AgentTaskSummary>, String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        agent_store::list_tasks(db.conn(), session_id, limit)
    }

    pub fn get_task_events(&self, task_id: Uuid) -> Result<Vec<AgentEvent>, String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        agent_store::list_events(db.conn(), task_id)
    }

    pub async fn start_task(
        self: Arc<Self>,
        app: AppHandle,
        session_id: Uuid,
        instruction: String,
        sandbox_mode: SandboxMode,
        skill_id: Option<String>,
    ) -> Result<Uuid, String> {
        let skill = resolve_skill(&self.db, skill_id.as_deref(), SkillMode::Agent)?;
        let task_id = Uuid::new_v4();
        let started_at = agent_store::now_string();
        let snapshot = AgentTaskSnapshot {
            id: task_id,
            session_id,
            instruction,
            skill_id: skill.as_ref().map(|item| item.summary.id.clone()),
            sandbox_mode,
            status: AgentTaskStatus::Queued,
            steps: Vec::new(),
            pending_confirm: None,
            final_result: None,
            summary: None,
            error_code: None,
            error_message: None,
            started_at,
            finished_at: None,
        };

        self.persist_snapshot(&snapshot)?;
        {
            let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
            tasks.insert(
                task_id,
                RuntimeTask {
                    snapshot: snapshot.clone(),
                    next_step_seq: 1,
                    next_event_seq: 1,
                    last_successful_step_id: None,
                },
            );
        }

        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        self.cancel_senders
            .lock()
            .map_err(|e| e.to_string())?
            .insert(task_id, cancel_tx);

        self.emit_task(&app, &snapshot);
        self.append_event(&app, task_id, "task_started", serde_json::json!({}))?;

        let orchestrator = Orchestrator::new(
            self.clone(),
            app.clone(),
            task_id,
            session_id,
            sandbox_mode,
            skill,
            cancel_rx,
        )?;
        tokio::spawn(async move {
            if let Err(error) = orchestrator.run().await {
                let is_cancelled = self
                    .get_task(task_id)
                    .ok()
                    .flatten()
                    .map(|task| task.status == AgentTaskStatus::Cancelled)
                    .unwrap_or(false);
                if !is_cancelled {
                    let _ = self.fail_task(&app, task_id, "runtime_error", &error);
                }
            }
        });

        Ok(task_id)
    }

    pub fn confirm_step(&self, task_id: Uuid, confirmed: bool) -> Result<(), String> {
        let mut senders = self.confirm_senders.lock().map_err(|e| e.to_string())?;
        if let Some(sender) = senders.remove(&task_id) {
            let _ = sender.send(confirmed);
        }
        Ok(())
    }

    pub fn cancel_task(&self, task_id: Uuid) -> Result<(), String> {
        if self.get_task(task_id)?.is_none() {
            return Err("Task not found".to_string());
        }

        self.update_task(task_id, |task| {
            if !task.status.is_terminal() {
                task.status = AgentTaskStatus::Cancelling;
                task.pending_confirm = None;
                task.error_message = Some("用户已请求停止任务".to_string());
            }
        })?;

        if let Ok(mut senders) = self.confirm_senders.lock() {
            if let Some(sender) = senders.remove(&task_id) {
                let _ = sender.send(false);
            }
        }
        if let Ok(mut senders) = self.cancel_senders.lock() {
            if let Some(sender) = senders.remove(&task_id) {
                let _ = sender.send(());
            }
        }
        Ok(())
    }

    pub(crate) fn build_planner_context(&self, task_id: Uuid) -> Result<PlannerContext, String> {
        let snapshot = self
            .get_task(task_id)?
            .ok_or("Task not found".to_string())?;
        Ok(PlannerContext {
            task_id: snapshot.id,
            session_id: snapshot.session_id,
            instruction: snapshot.instruction,
            skill: snapshot
                .skill_id
                .as_deref()
                .map(|skill_id| resolve_skill(&self.db, Some(skill_id), SkillMode::Agent))
                .transpose()?
                .flatten()
                .map(|skill| PlannerSkillContext {
                    id: skill.summary.id,
                    name: skill.summary.name,
                    description: skill.summary.description,
                    recommended_sandbox: skill.summary.recommended_sandbox,
                }),
            sandbox_mode: snapshot.sandbox_mode,
            status: snapshot.status,
            steps: snapshot
                .steps
                .into_iter()
                .map(|step| PlannerContextStep {
                    seq: step.seq,
                    kind: step.kind.as_str().to_string(),
                    title: step.title,
                    tool_name: step.tool_name,
                    command: step.command,
                    output: step.output,
                    status: step.status.as_str().to_string(),
                })
                .collect(),
            pending_confirm: snapshot.pending_confirm,
        })
    }

    pub(crate) fn set_status(
        &self,
        app: &AppHandle,
        task_id: Uuid,
        status: AgentTaskStatus,
        event_type: &str,
        payload: Value,
    ) -> Result<(), String> {
        let snapshot = self.update_task(task_id, |task| task.status = status.clone())?;
        self.emit_task(app, &snapshot);
        self.append_event(app, task_id, event_type, payload)?;
        Ok(())
    }

    pub(crate) fn add_step(
        &self,
        task_id: Uuid,
        kind: AgentStepKind,
        title: String,
        tool_name: Option<String>,
        command: Option<String>,
        status: AgentStepStatus,
        risk_level: Option<String>,
    ) -> Result<AgentStep, String> {
        let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
        let runtime = tasks
            .get_mut(&task_id)
            .ok_or("Task not found".to_string())?;
        let step = AgentStep {
            id: Uuid::new_v4(),
            seq: runtime.next_step_seq,
            kind,
            title,
            tool_name,
            command,
            output: None,
            status,
            risk_level,
            started_at: agent_store::now_string(),
            finished_at: None,
        };
        runtime.next_step_seq += 1;
        runtime.snapshot.steps.push(step.clone());
        let snapshot = runtime.snapshot.clone();
        drop(tasks);

        self.persist_snapshot(&snapshot)?;
        self.persist_step(task_id, &step)?;
        Ok(step)
    }

    pub(crate) fn finish_step(
        &self,
        task_id: Uuid,
        step_id: Uuid,
        status: AgentStepStatus,
        output: Option<String>,
    ) -> Result<(), String> {
        let snapshot = self.update_task(task_id, |task| {
            if let Some(step) = task.steps.iter_mut().find(|step| step.id == step_id) {
                step.status = status.clone();
                step.output = output.clone();
                step.finished_at = Some(agent_store::now_string());
            }
        })?;
        if let Some(step) = snapshot.steps.iter().find(|step| step.id == step_id) {
            self.persist_step(task_id, step)?;
        }
        Ok(())
    }

    pub(crate) fn append_event(
        &self,
        app: &AppHandle,
        task_id: Uuid,
        event_type: &str,
        payload_json: Value,
    ) -> Result<(), String> {
        let event = {
            let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
            let runtime = tasks
                .get_mut(&task_id)
                .ok_or("Task not found".to_string())?;
            let event = AgentEvent {
                id: Uuid::new_v4(),
                task_id,
                seq: runtime.next_event_seq,
                event_type: event_type.to_string(),
                payload_json,
                created_at: agent_store::now_string(),
            };
            runtime.next_event_seq += 1;
            event
        };

        {
            let db = self.db.lock().map_err(|e| e.to_string())?;
            agent_store::append_event(db.conn(), &event)?;
        }
        use tauri::Emitter;
        let _ = app.emit(&format!("ai-agent-event-{}", task_id), &event);
        Ok(())
    }

    pub(crate) fn prepare_confirmation(
        &self,
        app: &AppHandle,
        task_id: Uuid,
        step_id: Uuid,
        command: String,
        reason: String,
        risk_level: String,
    ) -> Result<tokio::sync::oneshot::Receiver<bool>, String> {
        let pending_confirm = PendingConfirm {
            task_id,
            step_id,
            command: command.clone(),
            reason: reason.clone(),
            risk_level: risk_level.clone(),
        };
        let snapshot = self.update_task(task_id, |task| {
            task.status = AgentTaskStatus::WaitingConfirm;
            task.pending_confirm = Some(pending_confirm.clone());
        })?;
        self.emit_task(app, &snapshot);

        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        self.confirm_senders
            .lock()
            .map_err(|e| e.to_string())?
            .insert(task_id, tx);

        self.append_event(
            app,
            task_id,
            "confirmation_requested",
            serde_json::json!({
                "step_id": step_id,
                "command": command,
                "reason": reason,
                "risk_level": risk_level,
            }),
        )?;

        Ok(rx)
    }

    pub(crate) fn clear_pending_confirm(&self, task_id: Uuid) -> Result<(), String> {
        if let Ok(mut senders) = self.confirm_senders.lock() {
            senders.remove(&task_id);
        }
        let _ = self.update_task(task_id, |task| {
            task.pending_confirm = None;
            if task.status == AgentTaskStatus::WaitingConfirm {
                task.status = AgentTaskStatus::Planning;
            }
        })?;
        Ok(())
    }

    pub(crate) fn check_cancelled(&self, task_id: Uuid, app: &AppHandle) -> Result<(), String> {
        if self.is_cancelling(task_id)? {
            self.cancel_task_final(app, task_id)?;
            return Err("Task cancelled".to_string());
        }
        Ok(())
    }

    pub(crate) fn is_cancelling(&self, task_id: Uuid) -> Result<bool, String> {
        let snapshot = self
            .get_task(task_id)?
            .ok_or("Task not found".to_string())?;
        Ok(matches!(
            snapshot.status,
            AgentTaskStatus::Cancelling | AgentTaskStatus::Cancelled
        ))
    }

    pub(crate) fn cancel_task_final(&self, app: &AppHandle, task_id: Uuid) -> Result<(), String> {
        let snapshot = self.update_task(task_id, |task| {
            task.status = AgentTaskStatus::Cancelled;
            task.pending_confirm = None;
            task.finished_at = Some(agent_store::now_string());
            task.final_result = Some(AgentFinalResult {
                status: AgentTaskStatus::Cancelled,
                summary: None,
                error_code: Some("task_cancelled".to_string()),
                error_message: task.error_message.clone(),
                last_successful_step_id: None,
            });
            task.error_code = Some("task_cancelled".to_string());
        })?;
        self.emit_task(app, &snapshot);
        self.append_event(
            app,
            task_id,
            "task_cancelled",
            serde_json::json!({ "status": "cancelled" }),
        )?;
        Ok(())
    }

    pub(crate) fn fail_task(
        &self,
        app: &AppHandle,
        task_id: Uuid,
        error_code: &str,
        error_message: &str,
    ) -> Result<(), String> {
        let last_successful_step_id = {
            let tasks = self.tasks.lock().map_err(|e| e.to_string())?;
            tasks
                .get(&task_id)
                .and_then(|runtime| runtime.last_successful_step_id)
        };
        let result_step = self.add_step(
            task_id,
            AgentStepKind::Result,
            "任务失败".to_string(),
            None,
            None,
            AgentStepStatus::Failed,
            None,
        )?;
        self.finish_step(
            task_id,
            result_step.id,
            AgentStepStatus::Failed,
            Some(error_message.to_string()),
        )?;
        let snapshot = self.update_task(task_id, |task| {
            task.status = AgentTaskStatus::Failed;
            task.pending_confirm = None;
            task.error_code = Some(error_code.to_string());
            task.error_message = Some(error_message.to_string());
            task.summary = None;
            task.finished_at = Some(agent_store::now_string());
            task.final_result = Some(AgentFinalResult {
                status: AgentTaskStatus::Failed,
                summary: None,
                error_code: Some(error_code.to_string()),
                error_message: Some(error_message.to_string()),
                last_successful_step_id,
            });
        })?;
        self.emit_task(app, &snapshot);
        self.append_event(
            app,
            task_id,
            "task_failed",
            serde_json::json!({
                "error_code": error_code,
                "error_message": error_message,
            }),
        )?;
        Ok(())
    }

    pub(crate) fn complete_task(
        &self,
        app: &AppHandle,
        task_id: Uuid,
        summary: &str,
    ) -> Result<(), String> {
        let last_successful_step_id = {
            let tasks = self.tasks.lock().map_err(|e| e.to_string())?;
            tasks
                .get(&task_id)
                .and_then(|runtime| runtime.last_successful_step_id)
        };
        let result_step = self.add_step(
            task_id,
            AgentStepKind::Result,
            "任务完成".to_string(),
            None,
            None,
            AgentStepStatus::Completed,
            None,
        )?;
        self.finish_step(
            task_id,
            result_step.id,
            AgentStepStatus::Completed,
            Some(summary.to_string()),
        )?;
        let snapshot = self.update_task(task_id, |task| {
            task.status = AgentTaskStatus::Completed;
            task.pending_confirm = None;
            task.summary = Some(summary.to_string());
            task.error_code = None;
            task.error_message = None;
            task.finished_at = Some(agent_store::now_string());
            task.final_result = Some(AgentFinalResult {
                status: AgentTaskStatus::Completed,
                summary: Some(summary.to_string()),
                error_code: None,
                error_message: None,
                last_successful_step_id,
            });
        })?;
        self.emit_task(app, &snapshot);
        self.append_event(
            app,
            task_id,
            "task_completed",
            serde_json::json!({ "summary": summary }),
        )?;
        Ok(())
    }

    pub(crate) fn mark_last_successful_step(
        &self,
        task_id: Uuid,
        step_id: Uuid,
    ) -> Result<(), String> {
        let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
        let runtime = tasks
            .get_mut(&task_id)
            .ok_or("Task not found".to_string())?;
        runtime.last_successful_step_id = Some(step_id);
        Ok(())
    }

    fn emit_task(&self, app: &AppHandle, snapshot: &AgentTaskSnapshot) {
        use tauri::Emitter;
        let _ = app.emit(&format!("ai-agent-task-{}", snapshot.id), snapshot);
    }

    fn update_task<F>(&self, task_id: Uuid, mutator: F) -> Result<AgentTaskSnapshot, String>
    where
        F: FnOnce(&mut AgentTaskSnapshot),
    {
        let snapshot = {
            let mut tasks = self.tasks.lock().map_err(|e| e.to_string())?;
            let runtime = tasks
                .get_mut(&task_id)
                .ok_or("Task not found".to_string())?;
            mutator(&mut runtime.snapshot);
            runtime.snapshot.clone()
        };
        self.persist_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    fn persist_snapshot(&self, snapshot: &AgentTaskSnapshot) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        agent_store::save_task(db.conn(), snapshot)
    }

    fn persist_step(&self, task_id: Uuid, step: &AgentStep) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| e.to_string())?;
        agent_store::save_step(db.conn(), task_id, step)
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentManager, RuntimeTask};
    use crate::modules::ai::agent_store;
    use crate::modules::ai::agent_types::{
        AgentEvent, AgentFinalResult, AgentStep, AgentStepKind, AgentStepStatus, AgentTaskSnapshot,
        AgentTaskStatus, PendingConfirm,
    };
    use crate::modules::ai::sandbox::SandboxMode;
    use crate::modules::connection::DefaultConnectionManager;
    use crate::modules::db::DatabaseManager;
    use std::sync::{Arc, Mutex, RwLock};
    use uuid::Uuid;

    fn build_manager() -> Arc<AgentManager> {
        Arc::new(AgentManager::new(
            Arc::new(Mutex::new(
                DatabaseManager::new(":memory:").expect("in-memory db"),
            )),
            Arc::new(RwLock::new(DefaultConnectionManager::new())),
        ))
    }

    fn insert_runtime_task(
        manager: &AgentManager,
        status: AgentTaskStatus,
        pending_confirm: Option<PendingConfirm>,
    ) -> Uuid {
        let task_id = Uuid::new_v4();
        let snapshot = AgentTaskSnapshot {
            id: task_id,
            session_id: Uuid::new_v4(),
            instruction: "test instruction".to_string(),
            skill_id: None,
            sandbox_mode: SandboxMode::Standard,
            status,
            steps: Vec::new(),
            pending_confirm,
            final_result: None,
            summary: None,
            error_code: None,
            error_message: None,
            started_at: agent_store::now_string(),
            finished_at: None,
        };
        manager.tasks.lock().expect("task mutex").insert(
            task_id,
            RuntimeTask {
                snapshot,
                next_step_seq: 1,
                next_event_seq: 1,
                last_successful_step_id: None,
            },
        );
        manager
            .persist_snapshot(
                &manager
                    .get_task(task_id)
                    .expect("load task")
                    .expect("task exists"),
            )
            .expect("persist snapshot");
        task_id
    }

    fn persist_history_task(
        manager: &AgentManager,
        snapshot: AgentTaskSnapshot,
        events: &[AgentEvent],
    ) {
        manager
            .persist_snapshot(&snapshot)
            .expect("persist history snapshot");
        for step in &snapshot.steps {
            manager
                .persist_step(snapshot.id, step)
                .expect("persist history step");
        }

        let db = manager.db.lock().expect("db mutex");
        for event in events {
            agent_store::append_event(db.conn(), event).expect("persist history event");
        }
    }

    #[test]
    fn cancel_task_marks_cancelling_and_clears_pending_confirmation() {
        let manager = build_manager();
        let task_id = insert_runtime_task(
            &manager,
            AgentTaskStatus::WaitingConfirm,
            Some(PendingConfirm {
                task_id: Uuid::new_v4(),
                step_id: Uuid::new_v4(),
                command: "rm -rf /tmp/demo".to_string(),
                reason: "need confirm".to_string(),
                risk_level: "high".to_string(),
            }),
        );

        manager.cancel_task(task_id).expect("cancel task");

        let task = manager
            .get_task(task_id)
            .expect("load task")
            .expect("task exists");
        assert_eq!(task.status, AgentTaskStatus::Cancelling);
        assert!(task.pending_confirm.is_none());
        assert_eq!(task.error_message.as_deref(), Some("用户已请求停止任务"));
        assert!(manager.is_cancelling(task_id).expect("cancelling state"));
    }

    #[test]
    fn clear_pending_confirm_returns_waiting_task_to_planning() {
        let manager = build_manager();
        let task_id = insert_runtime_task(
            &manager,
            AgentTaskStatus::WaitingConfirm,
            Some(PendingConfirm {
                task_id: Uuid::new_v4(),
                step_id: Uuid::new_v4(),
                command: "docker rm demo".to_string(),
                reason: "need confirm".to_string(),
                risk_level: "high".to_string(),
            }),
        );

        manager
            .clear_pending_confirm(task_id)
            .expect("clear confirm");

        let task = manager
            .get_task(task_id)
            .expect("load task")
            .expect("task exists");
        assert_eq!(task.status, AgentTaskStatus::Planning);
        assert!(task.pending_confirm.is_none());
    }

    #[test]
    fn clear_pending_confirm_does_not_override_non_waiting_status() {
        let manager = build_manager();
        let task_id = insert_runtime_task(
            &manager,
            AgentTaskStatus::Cancelling,
            Some(PendingConfirm {
                task_id: Uuid::new_v4(),
                step_id: Uuid::new_v4(),
                command: "docker rm demo".to_string(),
                reason: "need confirm".to_string(),
                risk_level: "high".to_string(),
            }),
        );

        manager
            .clear_pending_confirm(task_id)
            .expect("clear confirm");

        let task = manager
            .get_task(task_id)
            .expect("load task")
            .expect("task exists");
        assert_eq!(task.status, AgentTaskStatus::Cancelling);
        assert!(task.pending_confirm.is_none());
    }

    #[test]
    fn late_step_does_not_override_cancelled_final_result() {
        let manager = build_manager();
        let task_id = Uuid::new_v4();
        let snapshot = AgentTaskSnapshot {
            id: task_id,
            session_id: Uuid::new_v4(),
            instruction: "late tool".to_string(),
            skill_id: None,
            sandbox_mode: SandboxMode::Standard,
            status: AgentTaskStatus::Cancelled,
            steps: Vec::new(),
            pending_confirm: None,
            final_result: Some(AgentFinalResult {
                status: AgentTaskStatus::Cancelled,
                summary: None,
                error_code: Some("task_cancelled".to_string()),
                error_message: Some("user cancelled".to_string()),
                last_successful_step_id: None,
            }),
            summary: None,
            error_code: Some("task_cancelled".to_string()),
            error_message: Some("user cancelled".to_string()),
            started_at: agent_store::now_string(),
            finished_at: None,
        };

        manager.tasks.lock().expect("task mutex").insert(
            task_id,
            RuntimeTask {
                snapshot: snapshot.clone(),
                next_step_seq: 1,
                next_event_seq: 1,
                last_successful_step_id: None,
            },
        );

        let step = manager
            .add_step(
                task_id,
                AgentStepKind::ToolExecution,
                "late".to_string(),
                Some("execute_command".to_string()),
                Some("echo too late".to_string()),
                AgentStepStatus::Running,
                None,
            )
            .expect("add step");
        manager
            .finish_step(
                task_id,
                step.id,
                AgentStepStatus::Completed,
                Some("ignored".to_string()),
            )
            .expect("finish step");

        let task = manager
            .get_task(task_id)
            .expect("load task")
            .expect("task exists");
        assert_eq!(task.status, AgentTaskStatus::Cancelled);
        let final_result = task.final_result.expect("final result");
        assert_eq!(final_result.status, AgentTaskStatus::Cancelled);
        assert_eq!(final_result.error_code.as_deref(), Some("task_cancelled"));
    }

    #[test]
    fn get_task_falls_back_to_persisted_history_when_runtime_is_missing() {
        let manager = build_manager();
        let task_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        let snapshot = AgentTaskSnapshot {
            id: task_id,
            session_id: Uuid::new_v4(),
            instruction: "inspect disk".to_string(),
            skill_id: Some("system_health_check".to_string()),
            sandbox_mode: SandboxMode::Standard,
            status: AgentTaskStatus::Completed,
            steps: vec![AgentStep {
                id: step_id,
                seq: 1,
                kind: AgentStepKind::ToolExecution,
                title: "执行 df -h".to_string(),
                tool_name: Some("execute_command".to_string()),
                command: Some("df -h".to_string()),
                output: Some("Filesystem ...".to_string()),
                status: AgentStepStatus::Completed,
                risk_level: None,
                started_at: agent_store::now_string(),
                finished_at: Some(agent_store::now_string()),
            }],
            pending_confirm: None,
            final_result: Some(AgentFinalResult {
                status: AgentTaskStatus::Completed,
                summary: Some("磁盘空间正常".to_string()),
                error_code: None,
                error_message: None,
                last_successful_step_id: Some(step_id),
            }),
            summary: Some("磁盘空间正常".to_string()),
            error_code: None,
            error_message: None,
            started_at: agent_store::now_string(),
            finished_at: Some(agent_store::now_string()),
        };

        persist_history_task(
            &manager,
            snapshot,
            &[AgentEvent {
                id: Uuid::new_v4(),
                task_id,
                seq: 1,
                event_type: "task_completed".to_string(),
                payload_json: serde_json::json!({ "summary": "磁盘空间正常" }),
                created_at: agent_store::now_string(),
            }],
        );

        let loaded = manager
            .get_task(task_id)
            .expect("load task")
            .expect("task exists");
        assert_eq!(loaded.status, AgentTaskStatus::Completed);
        assert_eq!(loaded.steps.len(), 1);
        assert_eq!(loaded.summary.as_deref(), Some("磁盘空间正常"));
    }

    #[test]
    fn list_tasks_returns_session_history_in_descending_order() {
        let manager = build_manager();
        let session_a = Uuid::new_v4();
        let session_b = Uuid::new_v4();
        let snapshots = [
            AgentTaskSnapshot {
                id: Uuid::new_v4(),
                session_id: session_a,
                instruction: "older".to_string(),
                skill_id: None,
                sandbox_mode: SandboxMode::Standard,
                status: AgentTaskStatus::Completed,
                steps: Vec::new(),
                pending_confirm: None,
                final_result: None,
                summary: Some("older".to_string()),
                error_code: None,
                error_message: None,
                started_at: "2026-04-08T10:00:00Z".to_string(),
                finished_at: Some("2026-04-08T10:00:01Z".to_string()),
            },
            AgentTaskSnapshot {
                id: Uuid::new_v4(),
                session_id: session_a,
                instruction: "newer".to_string(),
                skill_id: Some("log_diagnostics".to_string()),
                sandbox_mode: SandboxMode::Standard,
                status: AgentTaskStatus::Failed,
                steps: Vec::new(),
                pending_confirm: None,
                final_result: None,
                summary: None,
                error_code: Some("planner_failed".to_string()),
                error_message: Some("boom".to_string()),
                started_at: "2026-04-08T11:00:00Z".to_string(),
                finished_at: Some("2026-04-08T11:00:01Z".to_string()),
            },
            AgentTaskSnapshot {
                id: Uuid::new_v4(),
                session_id: session_b,
                instruction: "other session".to_string(),
                skill_id: None,
                sandbox_mode: SandboxMode::Standard,
                status: AgentTaskStatus::Cancelled,
                steps: Vec::new(),
                pending_confirm: None,
                final_result: None,
                summary: None,
                error_code: Some("task_cancelled".to_string()),
                error_message: Some("cancelled".to_string()),
                started_at: "2026-04-08T12:00:00Z".to_string(),
                finished_at: Some("2026-04-08T12:00:01Z".to_string()),
            },
        ];

        for snapshot in snapshots {
            persist_history_task(&manager, snapshot, &[]);
        }

        let tasks = manager
            .list_tasks(Some(session_a), 10)
            .expect("list session tasks");
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].instruction, "newer");
        assert_eq!(tasks[1].instruction, "older");
    }

    #[test]
    fn get_task_events_returns_ordered_history_for_replay() {
        let manager = build_manager();
        let task_id = Uuid::new_v4();
        let snapshot = AgentTaskSnapshot {
            id: task_id,
            session_id: Uuid::new_v4(),
            instruction: "restart nginx".to_string(),
            skill_id: Some("log_diagnostics".to_string()),
            sandbox_mode: SandboxMode::Standard,
            status: AgentTaskStatus::Failed,
            steps: Vec::new(),
            pending_confirm: None,
            final_result: None,
            summary: None,
            error_code: Some("planner_failed".to_string()),
            error_message: Some("等待确认超时".to_string()),
            started_at: agent_store::now_string(),
            finished_at: Some(agent_store::now_string()),
        };

        persist_history_task(
            &manager,
            snapshot,
            &[
                AgentEvent {
                    id: Uuid::new_v4(),
                    task_id,
                    seq: 2,
                    event_type: "confirmation_timed_out".to_string(),
                    payload_json: serde_json::json!({ "command": "systemctl restart nginx" }),
                    created_at: agent_store::now_string(),
                },
                AgentEvent {
                    id: Uuid::new_v4(),
                    task_id,
                    seq: 1,
                    event_type: "confirmation_requested".to_string(),
                    payload_json: serde_json::json!({ "command": "systemctl restart nginx" }),
                    created_at: agent_store::now_string(),
                },
                AgentEvent {
                    id: Uuid::new_v4(),
                    task_id,
                    seq: 3,
                    event_type: "task_failed".to_string(),
                    payload_json: serde_json::json!({ "error_code": "planner_failed" }),
                    created_at: agent_store::now_string(),
                },
            ],
        );

        let events = manager.get_task_events(task_id).expect("load task events");
        assert_eq!(
            events.iter().map(|event| event.seq).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(events[1].event_type, "confirmation_timed_out");
    }
}
