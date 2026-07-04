use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredSkillRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub applies_to: String,
    pub source_type: String,
    pub source_path: String,
    pub manifest_path: String,
    pub trigger_mode: String,
    pub trigger_regex: Option<String>,
    pub match_keywords_json: String,
    pub allowed_tools_json: String,
    pub starter_examples_json: String,
    pub recommended_sandbox: Option<String>,
    pub enabled: bool,
    pub trusted: bool,
    pub content_hash: Option<String>,
    pub installed_at: String,
    pub updated_at: String,
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let pragma = format!("PRAGMA table_info({})", table);
    let mut stmt = conn.prepare(&pragma)?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let exists = columns
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .any(|name| name == column);

    if !exists {
        conn.execute(
            &format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition),
            [],
        )?;
    }

    Ok(())
}

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
            skill_id         TEXT,
            created_at       TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS sandbox_rules (
            id         TEXT PRIMARY KEY,
            rule_type  TEXT NOT NULL CHECK(rule_type IN ('whitelist','blacklist')),
            pattern    TEXT NOT NULL,
            reason     TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS ai_command_audit (
            id          TEXT PRIMARY KEY,
            task_id     TEXT NOT NULL,
            session_id  TEXT NOT NULL,
            command     TEXT NOT NULL,
            output      TEXT,
            executed_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS ai_agent_tasks (
            id                   TEXT PRIMARY KEY,
            session_id           TEXT NOT NULL,
            instruction          TEXT NOT NULL,
            sandbox_mode         TEXT NOT NULL,
            status               TEXT NOT NULL,
            summary              TEXT,
            error_code           TEXT,
            error_message        TEXT,
            skill_id             TEXT,
            pending_confirm_json TEXT,
            final_result_json    TEXT,
            started_at           TEXT NOT NULL,
            finished_at          TEXT
        );

        CREATE TABLE IF NOT EXISTS ai_agent_steps (
            id          TEXT PRIMARY KEY,
            task_id     TEXT NOT NULL REFERENCES ai_agent_tasks(id) ON DELETE CASCADE,
            seq         INTEGER NOT NULL,
            kind        TEXT NOT NULL,
            title       TEXT NOT NULL,
            tool_name   TEXT,
            command     TEXT,
            output      TEXT,
            status      TEXT NOT NULL,
            risk_level  TEXT,
            started_at  TEXT NOT NULL,
            finished_at TEXT
        );

        CREATE TABLE IF NOT EXISTS ai_agent_events (
            id          TEXT PRIMARY KEY,
            task_id     TEXT NOT NULL REFERENCES ai_agent_tasks(id) ON DELETE CASCADE,
            seq         INTEGER NOT NULL,
            event_type  TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            created_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS ai_skills (
            id                   TEXT PRIMARY KEY,
            name                 TEXT NOT NULL,
            description          TEXT NOT NULL,
            applies_to           TEXT NOT NULL,
            source_type          TEXT NOT NULL,
            source_path          TEXT NOT NULL,
            manifest_path        TEXT NOT NULL,
            trigger_mode         TEXT NOT NULL DEFAULT 'auto',
            trigger_regex        TEXT,
            match_keywords_json  TEXT NOT NULL DEFAULT '[]',
            allowed_tools_json   TEXT NOT NULL DEFAULT '[]',
            starter_examples_json TEXT NOT NULL DEFAULT '[]',
            recommended_sandbox  TEXT,
            enabled              INTEGER NOT NULL DEFAULT 0,
            trusted              INTEGER NOT NULL DEFAULT 0,
            content_hash         TEXT,
            installed_at         TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at           TEXT NOT NULL DEFAULT (datetime('now'))
        );
        ",
    )?;

    ensure_column(conn, "ai_messages", "skill_id", "TEXT")?;
    ensure_column(conn, "ai_agent_tasks", "skill_id", "TEXT")?;
    Ok(())
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
        params![id.to_string(), title, session_id.map(|s| s.to_string())],
    )?;
    Ok(())
}

pub fn get_all_conversations(
    conn: &Connection,
) -> Result<Vec<(String, String, Option<String>, String, String)>> {
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

/// 只保留最近 `keep` 条对话，删除更旧的（按 updated_at 排序）。
/// 关联消息会通过 ON DELETE CASCADE 一并删除。
pub fn prune_conversations(conn: &Connection, keep: usize) -> Result<()> {
    conn.execute(
        "DELETE FROM ai_conversations
         WHERE id NOT IN (
             SELECT id FROM ai_conversations
             ORDER BY updated_at DESC
             LIMIT ?
         )",
        params![keep as i64],
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
    skill_id: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO ai_messages (id, conversation_id, role, content, context_snapshot, skill_id) VALUES (?, ?, ?, ?, ?, ?)",
        params![
            id.to_string(),
            conversation_id.to_string(),
            role,
            content,
            context_snapshot,
            skill_id,
        ],
    )?;
    touch_conversation(conn, conversation_id)?;
    Ok(())
}

pub fn get_messages(
    conn: &Connection,
    conversation_id: &Uuid,
) -> Result<
    Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
    )>,
> {
    let mut stmt = conn.prepare(
        "SELECT id, role, content, context_snapshot, skill_id, created_at
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
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
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

// ── Installed Skills ───────────────────────────────────────────────────────────

pub fn save_skill_record(conn: &Connection, record: &StoredSkillRecord) -> Result<()> {
    conn.execute(
        "INSERT INTO ai_skills (
            id, name, description, applies_to, source_type, source_path, manifest_path,
            trigger_mode, trigger_regex, match_keywords_json, allowed_tools_json,
            starter_examples_json, recommended_sandbox, enabled, trusted, content_hash
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            applies_to = excluded.applies_to,
            source_type = excluded.source_type,
            source_path = excluded.source_path,
            manifest_path = excluded.manifest_path,
            trigger_mode = excluded.trigger_mode,
            trigger_regex = excluded.trigger_regex,
            match_keywords_json = excluded.match_keywords_json,
            allowed_tools_json = excluded.allowed_tools_json,
            starter_examples_json = excluded.starter_examples_json,
            recommended_sandbox = excluded.recommended_sandbox,
            enabled = excluded.enabled,
            trusted = excluded.trusted,
            content_hash = excluded.content_hash,
            updated_at = datetime('now')",
        params![
            record.id,
            record.name,
            record.description,
            record.applies_to,
            record.source_type,
            record.source_path,
            record.manifest_path,
            record.trigger_mode,
            record.trigger_regex,
            record.match_keywords_json,
            record.allowed_tools_json,
            record.starter_examples_json,
            record.recommended_sandbox,
            record.enabled as i64,
            record.trusted as i64,
            record.content_hash,
        ],
    )?;
    Ok(())
}

pub fn list_skill_records(conn: &Connection) -> Result<Vec<StoredSkillRecord>> {
    let mut stmt = conn.prepare(
        "SELECT
            id, name, description, applies_to, source_type, source_path, manifest_path,
            trigger_mode, trigger_regex, match_keywords_json, allowed_tools_json,
            starter_examples_json, recommended_sandbox, enabled, trusted, content_hash,
            installed_at, updated_at
         FROM ai_skills
         ORDER BY installed_at DESC, id ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(StoredSkillRecord {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            applies_to: row.get(3)?,
            source_type: row.get(4)?,
            source_path: row.get(5)?,
            manifest_path: row.get(6)?,
            trigger_mode: row.get(7)?,
            trigger_regex: row.get(8)?,
            match_keywords_json: row.get(9)?,
            allowed_tools_json: row.get(10)?,
            starter_examples_json: row.get(11)?,
            recommended_sandbox: row.get(12)?,
            enabled: row.get::<_, i64>(13)? != 0,
            trusted: row.get::<_, i64>(14)? != 0,
            content_hash: row.get(15)?,
            installed_at: row.get(16)?,
            updated_at: row.get(17)?,
        })
    })?;
    rows.collect()
}

pub fn get_skill_record(conn: &Connection, skill_id: &str) -> Result<Option<StoredSkillRecord>> {
    let mut stmt = conn.prepare(
        "SELECT
            id, name, description, applies_to, source_type, source_path, manifest_path,
            trigger_mode, trigger_regex, match_keywords_json, allowed_tools_json,
            starter_examples_json, recommended_sandbox, enabled, trusted, content_hash,
            installed_at, updated_at
         FROM ai_skills
         WHERE id = ?",
    )?;
    let mut rows = stmt.query(params![skill_id])?;
    if let Some(row) = rows.next()? {
        return Ok(Some(StoredSkillRecord {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            applies_to: row.get(3)?,
            source_type: row.get(4)?,
            source_path: row.get(5)?,
            manifest_path: row.get(6)?,
            trigger_mode: row.get(7)?,
            trigger_regex: row.get(8)?,
            match_keywords_json: row.get(9)?,
            allowed_tools_json: row.get(10)?,
            starter_examples_json: row.get(11)?,
            recommended_sandbox: row.get(12)?,
            enabled: row.get::<_, i64>(13)? != 0,
            trusted: row.get::<_, i64>(14)? != 0,
            content_hash: row.get(15)?,
            installed_at: row.get(16)?,
            updated_at: row.get(17)?,
        }));
    }
    Ok(None)
}

pub fn set_skill_enabled(conn: &Connection, skill_id: &str, enabled: bool) -> Result<()> {
    conn.execute(
        "UPDATE ai_skills SET enabled = ?, updated_at = datetime('now') WHERE id = ?",
        params![enabled as i64, skill_id],
    )?;
    Ok(())
}

pub fn set_skill_trusted(conn: &Connection, skill_id: &str, trusted: bool) -> Result<()> {
    conn.execute(
        "UPDATE ai_skills SET trusted = ?, updated_at = datetime('now') WHERE id = ?",
        params![trusted as i64, skill_id],
    )?;
    Ok(())
}

pub fn delete_skill_record(conn: &Connection, skill_id: &str) -> Result<()> {
    conn.execute("DELETE FROM ai_skills WHERE id = ?", params![skill_id])?;
    Ok(())
}

// ── Command Audit ─────────────────────────────────────────────────────────────

pub fn save_command_audit(
    conn: &Connection,
    id: &Uuid,
    task_id: &Uuid,
    session_id: &Uuid,
    command: &str,
    output: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO ai_command_audit (id, task_id, session_id, command, output) VALUES (?, ?, ?, ?, ?)",
        params![
            id.to_string(),
            task_id.to_string(),
            session_id.to_string(),
            command,
            output,
        ],
    )?;
    Ok(())
}

pub fn get_command_audit(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<(String, String, String, String, Option<String>, String)>> {
    let mut stmt = conn.prepare(
        "SELECT id, task_id, session_id, command, output, executed_at
         FROM ai_command_audit ORDER BY executed_at DESC LIMIT ?",
    )?;
    let rows = stmt.query_map(params![limit], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    rows.collect()
}

// ── Sandbox Rules ─────────────────────────────────────────────────────────────

pub fn save_sandbox_rule(
    conn: &Connection,
    id: &Uuid,
    rule_type: &str,
    pattern: &str,
    reason: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO sandbox_rules (id, rule_type, pattern, reason) VALUES (?, ?, ?, ?)",
        params![id.to_string(), rule_type, pattern, reason],
    )?;
    Ok(())
}

pub fn get_sandbox_rules(
    conn: &Connection,
) -> Result<Vec<(String, String, String, Option<String>)>> {
    let mut stmt = conn
        .prepare("SELECT id, rule_type, pattern, reason FROM sandbox_rules ORDER BY created_at")?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;
    rows.collect()
}

pub fn delete_sandbox_rule(conn: &Connection, id: &Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM sandbox_rules WHERE id = ?",
        params![id.to_string()],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::db::DatabaseManager;

    /// 创建一条对话并显式设定 updated_at，方便控制排序
    fn insert_conv(conn: &Connection, id: &Uuid, updated_at: &str) {
        create_conversation(conn, id, "chat", None).unwrap();
        conn.execute(
            "UPDATE ai_conversations SET updated_at = ? WHERE id = ?",
            params![updated_at, id.to_string()],
        )
        .unwrap();
    }

    fn conv_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM ai_conversations", [], |r| r.get(0))
            .unwrap()
    }

    fn msg_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM ai_messages", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn prune_keeps_only_most_recent() {
        let db = DatabaseManager::new(":memory:").unwrap();
        let conn = db.conn();

        // 5 条对话，updated_at 递增（conv5 最新）
        let mut ids = Vec::new();
        for i in 1..=5 {
            let id = Uuid::new_v4();
            insert_conv(conn, &id, &format!("2026-01-0{i} 00:00:00"));
            ids.push(id);
        }

        prune_conversations(conn, 3).unwrap();

        assert_eq!(conv_count(conn), 3);
        // 最旧的两条被删，最新的三条保留
        let remaining = get_all_conversations(conn).unwrap();
        let remaining_ids: Vec<String> = remaining.iter().map(|r| r.0.clone()).collect();
        assert!(remaining_ids.contains(&ids[4].to_string()));
        assert!(remaining_ids.contains(&ids[3].to_string()));
        assert!(remaining_ids.contains(&ids[2].to_string()));
        assert!(!remaining_ids.contains(&ids[0].to_string()));
        assert!(!remaining_ids.contains(&ids[1].to_string()));
    }

    #[test]
    fn prune_cascades_to_messages() {
        let db = DatabaseManager::new(":memory:").unwrap();
        let conn = db.conn();

        let old = Uuid::new_v4();
        let keep = Uuid::new_v4();
        insert_conv(conn, &old, "2026-01-01 00:00:00");
        insert_conv(conn, &keep, "2026-01-02 00:00:00");

        // 给两条对话各存一条消息
        save_message(conn, &Uuid::new_v4(), &old, "user", "old msg", None, None).unwrap();
        save_message(conn, &Uuid::new_v4(), &keep, "user", "keep msg", None, None).unwrap();
        assert_eq!(msg_count(conn), 2);

        prune_conversations(conn, 1).unwrap();

        // 旧对话及其消息应随 CASCADE 一并删除
        assert_eq!(conv_count(conn), 1);
        assert_eq!(msg_count(conn), 1);
    }

    #[test]
    fn prune_noop_when_under_limit() {
        let db = DatabaseManager::new(":memory:").unwrap();
        let conn = db.conn();

        insert_conv(conn, &Uuid::new_v4(), "2026-01-01 00:00:00");
        insert_conv(conn, &Uuid::new_v4(), "2026-01-02 00:00:00");

        prune_conversations(conn, 15).unwrap();

        assert_eq!(conv_count(conn), 2);
    }
}
