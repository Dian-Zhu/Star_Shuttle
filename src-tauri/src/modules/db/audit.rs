use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: u64,
    pub session_id: Option<Uuid>,
    pub user_id: Option<String>,
    pub command: String,
    pub risk_level: String,
    pub description: String,
    pub detected_patterns: String,
    pub action: String,
    pub details: Option<String>,
}

pub fn create_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS audit_events (
            id TEXT PRIMARY KEY,
            timestamp INTEGER NOT NULL,
            session_id TEXT,
            user_id TEXT,
            command TEXT NOT NULL,
            risk_level TEXT NOT NULL,
            description TEXT NOT NULL,
            detected_patterns TEXT NOT NULL,
            action TEXT NOT NULL,
            details TEXT
        )",
        [],
    )?;
    Ok(())
}

pub fn save_event(conn: &Connection, event: &AuditEvent) -> Result<()> {
    conn.execute(
        "INSERT INTO audit_events (id, timestamp, session_id, user_id, command, risk_level, description, detected_patterns, action, details) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            event.id.to_string(),
            event.timestamp,
            event.session_id.map(|id| id.to_string()),
            event.user_id.clone(),
            event.command,
            event.risk_level,
            event.description,
            event.detected_patterns,
            event.action,
            event.details,
        ],
    )?;
    Ok(())
}

pub fn get_events(conn: &Connection, limit: Option<u32>) -> Result<Vec<AuditEvent>> {
    let limit = limit.unwrap_or(100);
    let mut stmt = conn.prepare(
        "SELECT id, timestamp, session_id, user_id, command, risk_level, description, detected_patterns, action, details FROM audit_events ORDER BY timestamp DESC LIMIT ?",
    )?;
    let event_iter = stmt.query_map(params![limit], |row| {
        let session_id_str: Option<String> = row.get(2)?;
        let session_id = session_id_str.and_then(|s| Uuid::parse_str(&s).ok());
        Ok(AuditEvent {
            id: Uuid::parse_str(row.get::<_, String>(0)?.as_str()).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
            timestamp: row.get(1)?,
            session_id,
            user_id: row.get(3)?,
            command: row.get(4)?,
            risk_level: row.get(5)?,
            description: row.get(6)?,
            detected_patterns: row.get(7)?,
            action: row.get(8)?,
            details: row.get(9)?,
        })
    })?;

    let mut events = Vec::new();
    for event in event_iter {
        events.push(event?);
    }
    Ok(events)
}

pub fn clear_events(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM audit_events", [])?;
    Ok(())
}
