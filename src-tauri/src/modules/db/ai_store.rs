use rusqlite::{params, Connection, Result};
use uuid::Uuid;

/// 创建 AI 相关数据表
pub fn create_tables(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS ai_conversations (
            id          TEXT PRIMARY KEY,
            title       TEXT NOT NULL DEFAULT 'New Chat',
            session_id  TEXT,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS ai_messages (
            id               TEXT PRIMARY KEY,
            conversation_id  TEXT NOT NULL REFERENCES ai_conversations(id) ON DELETE CASCADE,
            role             TEXT NOT NULL,
            content          TEXT NOT NULL,
            context_snapshot TEXT,
            created_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );
        ",
    )
}

// ── Conversation CRUD ─────────────────────────────────────────────────────────

pub fn create_conversation(
    conn: &Connection,
    id: &Uuid,
    title: &str,
    session_id: Option<&Uuid>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO ai_conversations (id, title, session_id) VALUES (?, ?, ?)",
        params![
            id.to_string(),
            title,
            session_id.map(|s| s.to_string())
        ],
    )?;
    Ok(())
}

pub fn get_all_conversations(conn: &Connection) -> Result<Vec<(String, String, Option<String>, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, title, session_id, created_at, updated_at FROM ai_conversations ORDER BY updated_at DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    rows.collect()
}

pub fn update_conversation_title(conn: &Connection, id: &Uuid, title: &str) -> Result<()> {
    conn.execute(
        "UPDATE ai_conversations SET title = ?, updated_at = datetime('now') WHERE id = ?",
        params![title, id.to_string()],
    )?;
    Ok(())
}

pub fn touch_conversation(conn: &Connection, id: &Uuid) -> Result<()> {
    conn.execute(
        "UPDATE ai_conversations SET updated_at = datetime('now') WHERE id = ?",
        params![id.to_string()],
    )?;
    Ok(())
}

pub fn delete_conversation(conn: &Connection, id: &Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM ai_conversations WHERE id = ?",
        params![id.to_string()],
    )?;
    Ok(())
}

// ── Message CRUD ──────────────────────────────────────────────────────────────

pub fn save_message(
    conn: &Connection,
    id: &Uuid,
    conversation_id: &Uuid,
    role: &str,
    content: &str,
    context_snapshot: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO ai_messages (id, conversation_id, role, content, context_snapshot) VALUES (?, ?, ?, ?, ?)",
        params![
            id.to_string(),
            conversation_id.to_string(),
            role,
            content,
            context_snapshot,
        ],
    )?;
    touch_conversation(conn, conversation_id)?;
    Ok(())
}

pub fn get_messages(
    conn: &Connection,
    conversation_id: &Uuid,
) -> Result<Vec<(String, String, String, Option<String>, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, role, content, context_snapshot, created_at
         FROM ai_messages
         WHERE conversation_id = ?
         ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![conversation_id.to_string()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    rows.collect()
}

pub fn delete_messages(conn: &Connection, conversation_id: &Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM ai_messages WHERE conversation_id = ?",
        params![conversation_id.to_string()],
    )?;
    Ok(())
}
