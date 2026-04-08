use crate::modules::ai::sandbox::SandboxMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTaskStatus {
    Queued,
    Planning,
    Executing,
    WaitingConfirm,
    Retrying,
    Cancelling,
    Cancelled,
    Failed,
    Completed,
}

impl AgentTaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Planning => "planning",
            Self::Executing => "executing",
            Self::WaitingConfirm => "waiting_confirm",
            Self::Retrying => "retrying",
            Self::Cancelling => "cancelling",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Completed => "completed",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        Some(match value {
            "queued" => Self::Queued,
            "planning" => Self::Planning,
            "executing" => Self::Executing,
            "waiting_confirm" => Self::WaitingConfirm,
            "retrying" => Self::Retrying,
            "cancelling" => Self::Cancelling,
            "cancelled" => Self::Cancelled,
            "failed" => Self::Failed,
            "completed" => Self::Completed,
            _ => return None,
        })
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Cancelled | Self::Failed | Self::Completed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Rejected,
    Skipped,
}

impl AgentStepStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Rejected => "rejected",
            Self::Skipped => "skipped",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        Some(match value {
            "pending" => Self::Pending,
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "rejected" => Self::Rejected,
            "skipped" => Self::Skipped,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStepKind {
    Planning,
    ToolExecution,
    Confirmation,
    Result,
}

impl AgentStepKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Planning => "planning",
            Self::ToolExecution => "tool_execution",
            Self::Confirmation => "confirmation",
            Self::Result => "result",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        Some(match value {
            "planning" => Self::Planning,
            "tool_execution" => Self::ToolExecution,
            "confirmation" => Self::Confirmation,
            "result" => Self::Result,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub id: Uuid,
    pub seq: u32,
    pub kind: AgentStepKind,
    pub title: String,
    pub tool_name: Option<String>,
    pub command: Option<String>,
    pub output: Option<String>,
    pub status: AgentStepStatus,
    pub risk_level: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingConfirm {
    pub task_id: Uuid,
    pub step_id: Uuid,
    pub command: String,
    pub reason: String,
    pub risk_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFinalResult {
    pub status: AgentTaskStatus,
    pub summary: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub last_successful_step_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskSnapshot {
    pub id: Uuid,
    pub session_id: Uuid,
    pub instruction: String,
    pub skill_id: Option<String>,
    pub sandbox_mode: SandboxMode,
    pub status: AgentTaskStatus,
    pub steps: Vec<AgentStep>,
    pub pending_confirm: Option<PendingConfirm>,
    pub final_result: Option<AgentFinalResult>,
    pub summary: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTaskSummary {
    pub id: Uuid,
    pub session_id: Uuid,
    pub instruction: String,
    pub skill_id: Option<String>,
    pub sandbox_mode: SandboxMode,
    pub status: AgentTaskStatus,
    pub summary: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEvent {
    pub id: Uuid,
    pub task_id: Uuid,
    pub seq: u32,
    pub event_type: String,
    pub payload_json: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlannerAction {
    ToolCall {
        tool_name: String,
        args: Value,
        rationale: String,
    },
    Complete {
        summary: String,
    },
    Fail {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerContextStep {
    pub seq: u32,
    pub kind: String,
    pub title: String,
    pub tool_name: Option<String>,
    pub command: Option<String>,
    pub output: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerSkillContext {
    pub id: String,
    pub name: String,
    pub description: String,
    pub recommended_sandbox: Option<SandboxMode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannerContext {
    pub task_id: Uuid,
    pub session_id: Uuid,
    pub instruction: String,
    pub skill: Option<PlannerSkillContext>,
    pub sandbox_mode: SandboxMode,
    pub status: AgentTaskStatus,
    pub steps: Vec<PlannerContextStep>,
    pub pending_confirm: Option<PendingConfirm>,
}

#[cfg(test)]
mod tests {
    use super::{AgentStepStatus, AgentTaskStatus};

    #[test]
    fn task_status_terminal_flags_are_consistent() {
        assert!(!AgentTaskStatus::Queued.is_terminal());
        assert!(!AgentTaskStatus::Planning.is_terminal());
        assert!(!AgentTaskStatus::Executing.is_terminal());
        assert!(!AgentTaskStatus::WaitingConfirm.is_terminal());
        assert!(!AgentTaskStatus::Retrying.is_terminal());
        assert!(!AgentTaskStatus::Cancelling.is_terminal());
        assert!(AgentTaskStatus::Cancelled.is_terminal());
        assert!(AgentTaskStatus::Failed.is_terminal());
        assert!(AgentTaskStatus::Completed.is_terminal());
    }

    #[test]
    fn task_status_round_trip_strings() {
        for value in [
            AgentTaskStatus::Queued,
            AgentTaskStatus::Planning,
            AgentTaskStatus::Executing,
            AgentTaskStatus::WaitingConfirm,
            AgentTaskStatus::Retrying,
            AgentTaskStatus::Cancelling,
            AgentTaskStatus::Cancelled,
            AgentTaskStatus::Failed,
            AgentTaskStatus::Completed,
        ] {
            assert_eq!(AgentTaskStatus::from_str(value.as_str()), Some(value));
        }
        assert_eq!(AgentTaskStatus::from_str("unknown"), None);
    }

    #[test]
    fn step_status_round_trip_strings() {
        for value in [
            AgentStepStatus::Pending,
            AgentStepStatus::Running,
            AgentStepStatus::Completed,
            AgentStepStatus::Failed,
            AgentStepStatus::Rejected,
            AgentStepStatus::Skipped,
        ] {
            assert_eq!(AgentStepStatus::from_str(value.as_str()), Some(value));
        }
        assert_eq!(AgentStepStatus::from_str("unknown"), None);
    }
}
