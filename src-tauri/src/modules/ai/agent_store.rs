use crate::modules::ai::agent_types::{
    AgentEvent, AgentFinalResult, AgentStep, AgentStepKind, AgentStepStatus, AgentTaskSnapshot,
    AgentTaskStatus, AgentTaskSummary, PendingConfirm,
};
use crate::modules::ai::sandbox::SandboxMode;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use uuid::Uuid;

fn sandbox_mode_to_str(mode: &SandboxMode) -> &'static str {
    match mode {
        SandboxMode::Standard => "standard",
        SandboxMode::Full => "full",
    }
}

fn sandbox_mode_from_str(value: &str) -> SandboxMode {
    match value {
        "full" => SandboxMode::Full,
        _ => SandboxMode::Standard,
    }
}

fn parse_uuid(value: String) -> rusqlite::Result<Uuid> {
    Uuid::parse_str(&value).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            value.len(),
            rusqlite::types::Type::Text,
            Box::new(e),
        )
    })
}

pub fn now_string() -> String {
    Utc::now().to_rfc3339()
}

pub fn save_task(conn: &Connection, task: &AgentTaskSnapshot) -> Result<(), String> {
    let final_result = task
        .final_result
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| e.to_string())?;
    let pending_confirm = task
        .pending_confirm
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| e.to_string())?;

    conn.execute(
        "INSERT INTO ai_agent_tasks (
            id, session_id, instruction, sandbox_mode, status, summary, error_code, error_message,
            pending_confirm_json, final_result_json, started_at, finished_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            session_id = excluded.session_id,
            instruction = excluded.instruction,
            sandbox_mode = excluded.sandbox_mode,
            status = excluded.status,
            summary = excluded.summary,
            error_code = excluded.error_code,
            error_message = excluded.error_message,
            pending_confirm_json = excluded.pending_confirm_json,
            final_result_json = excluded.final_result_json,
            started_at = excluded.started_at,
            finished_at = excluded.finished_at",
        params![
            task.id.to_string(),
            task.session_id.to_string(),
            task.instruction,
            sandbox_mode_to_str(&task.sandbox_mode),
            task.status.as_str(),
            task.summary,
            task.error_code,
            task.error_message,
            pending_confirm,
            final_result,
            task.started_at,
            task.finished_at,
        ],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn save_step(conn: &Connection, task_id: Uuid, step: &AgentStep) -> Result<(), String> {
    conn.execute(
        "INSERT INTO ai_agent_steps (
            id, task_id, seq, kind, title, tool_name, command, output, status, risk_level, started_at, finished_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            seq = excluded.seq,
            kind = excluded.kind,
            title = excluded.title,
            tool_name = excluded.tool_name,
            command = excluded.command,
            output = excluded.output,
            status = excluded.status,
            risk_level = excluded.risk_level,
            started_at = excluded.started_at,
            finished_at = excluded.finished_at",
        params![
            step.id.to_string(),
            task_id.to_string(),
            step.seq,
            step.kind.as_str(),
            step.title,
            step.tool_name,
            step.command,
            step.output,
            step.status.as_str(),
            step.risk_level,
            step.started_at,
            step.finished_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn append_event(conn: &Connection, event: &AgentEvent) -> Result<(), String> {
    conn.execute(
        "INSERT INTO ai_agent_events (id, task_id, seq, event_type, payload_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        params![
            event.id.to_string(),
            event.task_id.to_string(),
            event.seq,
            event.event_type,
            serde_json::to_string(&event.payload_json).map_err(|e| e.to_string())?,
            event.created_at,
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn load_steps(conn: &Connection, task_id: Uuid) -> Result<Vec<AgentStep>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, seq, kind, title, tool_name, command, output, status, risk_level, started_at, finished_at
             FROM ai_agent_steps WHERE task_id = ? ORDER BY seq ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![task_id.to_string()], |row| {
            let id = parse_uuid(row.get(0)?)?;
            let kind = AgentStepKind::from_str(&row.get::<_, String>(2)?).ok_or_else(|| {
                rusqlite::Error::InvalidColumnType(2, "kind".to_string(), rusqlite::types::Type::Text)
            })?;
            let status = AgentStepStatus::from_str(&row.get::<_, String>(7)?).ok_or_else(|| {
                rusqlite::Error::InvalidColumnType(7, "status".to_string(), rusqlite::types::Type::Text)
            })?;
            Ok(AgentStep {
                id,
                seq: row.get(1)?,
                kind,
                title: row.get(3)?,
                tool_name: row.get(4)?,
                command: row.get(5)?,
                output: row.get(6)?,
                status,
                risk_level: row.get(8)?,
                started_at: row.get(9)?,
                finished_at: row.get(10)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn load_task(conn: &Connection, task_id: Uuid) -> Result<Option<AgentTaskSnapshot>, String> {
    let row = conn
        .query_row(
            "SELECT id, session_id, instruction, sandbox_mode, status, summary, error_code, error_message,
                    pending_confirm_json, final_result_json, started_at, finished_at
             FROM ai_agent_tasks WHERE id = ?",
            params![task_id.to_string()],
            |row| {
                let id = parse_uuid(row.get(0)?)?;
                let session_id = parse_uuid(row.get(1)?)?;
                let status = AgentTaskStatus::from_str(&row.get::<_, String>(4)?).ok_or_else(|| {
                    rusqlite::Error::InvalidColumnType(4, "status".to_string(), rusqlite::types::Type::Text)
                })?;
                let pending_confirm = row
                    .get::<_, Option<String>>(8)?
                    .map(|json| serde_json::from_str::<PendingConfirm>(&json))
                    .transpose()
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            8,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;
                let final_result = row
                    .get::<_, Option<String>>(9)?
                    .map(|json| serde_json::from_str::<AgentFinalResult>(&json))
                    .transpose()
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            9,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?;

                Ok(AgentTaskSnapshot {
                    id,
                    session_id,
                    instruction: row.get(2)?,
                    sandbox_mode: sandbox_mode_from_str(&row.get::<_, String>(3)?),
                    status,
                    steps: Vec::new(),
                    pending_confirm,
                    final_result,
                    summary: row.get(5)?,
                    error_code: row.get(6)?,
                    error_message: row.get(7)?,
                    started_at: row.get(10)?,
                    finished_at: row.get(11)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    let Some(mut snapshot) = row else {
        return Ok(None);
    };
    snapshot.steps = load_steps(conn, snapshot.id)?;
    Ok(Some(snapshot))
}

pub fn list_tasks(
    conn: &Connection,
    session_id: Option<Uuid>,
    limit: u32,
) -> Result<Vec<AgentTaskSummary>, String> {
    let sql = if session_id.is_some() {
        "SELECT id, session_id, instruction, sandbox_mode, status, summary, error_code, error_message, started_at, finished_at
         FROM ai_agent_tasks WHERE session_id = ? ORDER BY started_at DESC LIMIT ?"
    } else {
        "SELECT id, session_id, instruction, sandbox_mode, status, summary, error_code, error_message, started_at, finished_at
         FROM ai_agent_tasks ORDER BY started_at DESC LIMIT ?"
    };
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

    let mapper = |row: &rusqlite::Row<'_>| -> rusqlite::Result<AgentTaskSummary> {
        Ok(AgentTaskSummary {
            id: parse_uuid(row.get(0)?)?,
            session_id: parse_uuid(row.get(1)?)?,
            instruction: row.get(2)?,
            sandbox_mode: sandbox_mode_from_str(&row.get::<_, String>(3)?),
            status: AgentTaskStatus::from_str(&row.get::<_, String>(4)?).ok_or_else(|| {
                rusqlite::Error::InvalidColumnType(4, "status".to_string(), rusqlite::types::Type::Text)
            })?,
            summary: row.get(5)?,
            error_code: row.get(6)?,
            error_message: row.get(7)?,
            started_at: row.get(8)?,
            finished_at: row.get(9)?,
        })
    };

    let rows = if let Some(session_id) = session_id {
        stmt.query_map(params![session_id.to_string(), limit], mapper)
            .map_err(|e| e.to_string())?
    } else {
        stmt.query_map(params![limit], mapper).map_err(|e| e.to_string())?
    };

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

pub fn list_events(conn: &Connection, task_id: Uuid) -> Result<Vec<AgentEvent>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, task_id, seq, event_type, payload_json, created_at
             FROM ai_agent_events WHERE task_id = ? ORDER BY seq ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![task_id.to_string()], |row| {
            let payload_json = row.get::<_, String>(4)?;
            Ok(AgentEvent {
                id: parse_uuid(row.get(0)?)?,
                task_id: parse_uuid(row.get(1)?)?,
                seq: row.get(2)?,
                event_type: row.get(3)?,
                payload_json: serde_json::from_str::<Value>(&payload_json).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        4,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::db::ai_store;
    use rusqlite::Connection;

    #[test]
    fn saves_and_loads_agent_history() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        ai_store::create_tables(&conn).expect("create tables");

        let task_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let step_id = Uuid::new_v4();
        let snapshot = AgentTaskSnapshot {
            id: task_id,
            session_id,
            instruction: "list containers".to_string(),
            sandbox_mode: SandboxMode::Standard,
            status: AgentTaskStatus::Completed,
            steps: vec![AgentStep {
                id: step_id,
                seq: 1,
                kind: AgentStepKind::ToolExecution,
                title: "执行 docker ps".to_string(),
                tool_name: Some("execute_command".to_string()),
                command: Some("docker ps".to_string()),
                output: Some("ok".to_string()),
                status: AgentStepStatus::Completed,
                risk_level: None,
                started_at: now_string(),
                finished_at: Some(now_string()),
            }],
            pending_confirm: None,
            final_result: Some(AgentFinalResult {
                status: AgentTaskStatus::Completed,
                summary: Some("done".to_string()),
                error_code: None,
                error_message: None,
                last_successful_step_id: Some(step_id),
            }),
            summary: Some("done".to_string()),
            error_code: None,
            error_message: None,
            started_at: now_string(),
            finished_at: Some(now_string()),
        };
        save_task(&conn, &snapshot).expect("save task");
        save_step(&conn, task_id, &snapshot.steps[0]).expect("save step");
        append_event(
            &conn,
            &AgentEvent {
                id: Uuid::new_v4(),
                task_id,
                seq: 1,
                event_type: "task_completed".to_string(),
                payload_json: serde_json::json!({ "summary": "done" }),
                created_at: now_string(),
            },
        )
        .expect("save event");

        let loaded = load_task(&conn, task_id).expect("load task").expect("task exists");
        assert_eq!(loaded.summary.as_deref(), Some("done"));
        assert_eq!(loaded.steps.len(), 1);

        let listed = list_tasks(&conn, None, 10).expect("list tasks");
        assert_eq!(listed.len(), 1);

        let events = list_events(&conn, task_id).expect("list events");
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn lists_tasks_by_session_and_persists_failure_fields() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        ai_store::create_tables(&conn).expect("create tables");

        let session_a = Uuid::new_v4();
        let session_b = Uuid::new_v4();

        for (session_id, instruction, status, error_code) in [
            (session_a, "inspect nginx", AgentTaskStatus::Failed, Some("planner_failed")),
            (session_b, "check disk", AgentTaskStatus::Completed, None),
        ] {
            let snapshot = AgentTaskSnapshot {
                id: Uuid::new_v4(),
                session_id,
                instruction: instruction.to_string(),
                sandbox_mode: SandboxMode::Standard,
                status: status.clone(),
                steps: Vec::new(),
                pending_confirm: None,
                final_result: Some(AgentFinalResult {
                    status,
                    summary: None,
                    error_code: error_code.map(|value| value.to_string()),
                    error_message: Some("boom".to_string()).filter(|_| error_code.is_some()),
                    last_successful_step_id: None,
                }),
                summary: None,
                error_code: error_code.map(|value| value.to_string()),
                error_message: Some("boom".to_string()).filter(|_| error_code.is_some()),
                started_at: now_string(),
                finished_at: Some(now_string()),
            };
            save_task(&conn, &snapshot).expect("save task");
        }

        let session_a_tasks = list_tasks(&conn, Some(session_a), 10).expect("list filtered tasks");
        assert_eq!(session_a_tasks.len(), 1);
        assert_eq!(session_a_tasks[0].instruction, "inspect nginx");
        assert_eq!(session_a_tasks[0].error_code.as_deref(), Some("planner_failed"));
    }

    #[test]
    fn loads_pending_confirmation_payload() {
        let conn = Connection::open_in_memory().expect("in-memory db");
        ai_store::create_tables(&conn).expect("create tables");

        let task_id = Uuid::new_v4();
        let pending_confirm = PendingConfirm {
            task_id,
            step_id: Uuid::new_v4(),
            command: "systemctl restart nginx".to_string(),
            reason: "敏感操作".to_string(),
            risk_level: "high".to_string(),
        };
        let snapshot = AgentTaskSnapshot {
            id: task_id,
            session_id: Uuid::new_v4(),
            instruction: "restart nginx".to_string(),
            sandbox_mode: SandboxMode::Standard,
            status: AgentTaskStatus::WaitingConfirm,
            steps: Vec::new(),
            pending_confirm: Some(pending_confirm.clone()),
            final_result: None,
            summary: None,
            error_code: None,
            error_message: None,
            started_at: now_string(),
            finished_at: None,
        };

        save_task(&conn, &snapshot).expect("save task");
        let loaded = load_task(&conn, task_id).expect("load task").expect("task exists");
        assert_eq!(
            loaded.pending_confirm.as_ref().map(|item| item.command.as_str()),
            Some("systemctl restart nginx")
        );
    }
}
