use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandHistoryEntry {
    pub id: Uuid,
    pub command: String,
    pub connection_id: Option<String>,
    pub connection_name: Option<String>,
    pub cwd: Option<String>,
    pub executed_at: u64,
}

/// 单会话保留的最大历史条数上限，避免历史表无限增长。
const MAX_HISTORY_ENTRIES: usize = 5000;

pub fn create_table(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS command_history (
            id TEXT PRIMARY KEY,
            command TEXT NOT NULL,
            connection_id TEXT,
            connection_name TEXT,
            cwd TEXT,
            executed_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_command_history_executed_at
            ON command_history(executed_at)",
        [],
    )?;
    Ok(())
}

pub fn add(conn: &Connection, entry: &CommandHistoryEntry) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO command_history
            (id, command, connection_id, connection_name, cwd, executed_at)
         VALUES (?, ?, ?, ?, ?, ?)",
        params![
            entry.id.to_string(),
            entry.command,
            entry.connection_id,
            entry.connection_name,
            entry.cwd,
            entry.executed_at,
        ],
    )?;
    prune(conn)?;
    Ok(())
}

/// 超过上限时删除最旧的记录，只保留最近的 MAX_HISTORY_ENTRIES 条。
fn prune(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM command_history
         WHERE id NOT IN (
            SELECT id FROM command_history
            ORDER BY executed_at DESC
            LIMIT ?
         )",
        params![MAX_HISTORY_ENTRIES as i64],
    )?;
    Ok(())
}

pub fn get_recent(conn: &Connection, limit: i64) -> Result<Vec<CommandHistoryEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, command, connection_id, connection_name, cwd, executed_at
         FROM command_history
         ORDER BY executed_at DESC
         LIMIT ?",
    )?;
    let iter = stmt.query_map(params![limit], row_to_entry)?;

    let mut entries = Vec::new();
    for entry in iter {
        entries.push(entry?);
    }
    Ok(entries)
}

pub fn clear(conn: &Connection) -> Result<()> {
    conn.execute("DELETE FROM command_history", [])?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM command_history WHERE id = ?",
        params![id.to_string()],
    )?;
    Ok(())
}

fn row_to_entry(row: &rusqlite::Row) -> Result<CommandHistoryEntry> {
    Ok(CommandHistoryEntry {
        id: Uuid::parse_str(row.get::<_, String>(0)?.as_str()).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?,
        command: row.get(1)?,
        connection_id: row.get(2)?,
        connection_name: row.get(3)?,
        cwd: row.get(4)?,
        executed_at: row.get(5)?,
    })
}
