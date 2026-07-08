pub mod ai_store;
mod command_history;
mod command_snippets;

use rusqlite::{params, Connection, Result};
use uuid::Uuid;

pub use command_history::CommandHistoryEntry;
pub use command_snippets::CommandSnippet;

pub struct DatabaseManager {
    conn: Connection,
}

impl DatabaseManager {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        // 开启外键约束，使 ON DELETE CASCADE 生效（删除对话时级联清理其消息）
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Self::create_tables(&conn)?;
        Ok(Self { conn })
    }

    /// 暴露底层连接供子模块直接使用（仅限内部）
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn create_tables(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;
        command_snippets::create_table(conn)?;
        command_history::create_table(conn)?;
        ai_store::create_tables(conn)?;
        Ok(())
    }

    pub fn save_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT value FROM settings WHERE key = ?")?;
        let mut rows = stmt.query(params![key])?;

        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn delete_setting(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM settings WHERE key = ?", params![key])?;
        Ok(())
    }

    // Command snippets CRUD
    pub fn save_command_snippet(&self, snippet: &CommandSnippet) -> Result<()> {
        command_snippets::save(&self.conn, snippet)
    }

    pub fn get_command_snippets(&self) -> Result<Vec<CommandSnippet>> {
        command_snippets::get_all(&self.conn)
    }

    pub fn get_command_snippet_by_id(&self, id: &Uuid) -> Result<Option<CommandSnippet>> {
        command_snippets::get_by_id(&self.conn, id)
    }

    pub fn delete_command_snippet(&self, id: &Uuid) -> Result<()> {
        command_snippets::delete(&self.conn, id)
    }

    pub fn increment_usage_count(&self, id: &Uuid) -> Result<()> {
        command_snippets::increment_usage_count(&self.conn, id)
    }

    // Command history CRUD
    pub fn add_command_history(&self, entry: &CommandHistoryEntry) -> Result<()> {
        command_history::add(&self.conn, entry)
    }

    pub fn get_command_history(&self, limit: i64) -> Result<Vec<CommandHistoryEntry>> {
        command_history::get_recent(&self.conn, limit)
    }

    pub fn clear_command_history(&self) -> Result<()> {
        command_history::clear(&self.conn)
    }

    pub fn delete_command_history(&self, id: &Uuid) -> Result<()> {
        command_history::delete(&self.conn, id)
    }
}
