use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub id: Uuid,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub auth_method: String,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandSnippet {
    pub id: Uuid,
    pub name: String,
    pub command: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub tags: Option<String>,
    pub created_at: u64,
    pub updated_at: u64,
    pub usage_count: i32,
}

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

pub struct DatabaseManager {
    conn: Connection,
}

impl DatabaseManager {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        Self::create_tables(&conn)?;
        Ok(Self { conn })
    }

    fn create_tables(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS connection_profiles (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                username TEXT NOT NULL,
                auth_method TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS command_snippets (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                description TEXT,
                category TEXT,
                tags TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                usage_count INTEGER DEFAULT 0
            )",
            [],
        )?;

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

    pub fn save_connection_profile(&self, profile: &ConnectionProfile) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO connection_profiles (id, name, host, port, username, auth_method, created_at, updated_at) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                profile.id.to_string(),
                profile.name,
                profile.host,
                profile.port,
                profile.username,
                profile.auth_method,
                profile.created_at,
                profile.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn get_connection_profiles(&self) -> Result<Vec<ConnectionProfile>> {
        let mut stmt = self.conn.prepare("SELECT id, name, host, port, username, auth_method, created_at, updated_at FROM connection_profiles")?;
        let profile_iter = stmt.query_map([], |row| {
            Ok(ConnectionProfile {
                id: Uuid::parse_str(row.get::<_, String>(0)?.as_str()).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                auth_method: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;

        let mut profiles = Vec::new();
        for profile in profile_iter {
            profiles.push(profile?);
        }

        Ok(profiles)
    }

    // Command snippets CRUD
    pub fn save_command_snippet(&self, snippet: &CommandSnippet) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO command_snippets (id, name, command, description, category, tags, created_at, updated_at, usage_count) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                snippet.id.to_string(),
                snippet.name,
                snippet.command,
                snippet.description,
                snippet.category,
                snippet.tags,
                snippet.created_at,
                snippet.updated_at,
                snippet.usage_count,
            ],
        )?;
        Ok(())
    }

    pub fn get_command_snippets(&self) -> Result<Vec<CommandSnippet>> {
        let mut stmt = self.conn.prepare("SELECT id, name, command, description, category, tags, created_at, updated_at, usage_count FROM command_snippets")?;
        let snippet_iter = stmt.query_map([], |row| {
            Ok(CommandSnippet {
                id: Uuid::parse_str(row.get::<_, String>(0)?.as_str()).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                name: row.get(1)?,
                command: row.get(2)?,
                description: row.get(3)?,
                category: row.get(4)?,
                tags: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                usage_count: row.get(8)?,
            })
        })?;

        let mut snippets = Vec::new();
        for snippet in snippet_iter {
            snippets.push(snippet?);
        }
        Ok(snippets)
    }

    pub fn get_command_snippet_by_id(&self, id: &Uuid) -> Result<Option<CommandSnippet>> {
        let mut stmt = self.conn.prepare("SELECT id, name, command, description, category, tags, created_at, updated_at, usage_count FROM command_snippets WHERE id = ?")?;
        let mut rows = stmt.query(params![id.to_string()])?;

        if let Some(row) = rows.next()? {
            Ok(Some(CommandSnippet {
                id: Uuid::parse_str(row.get::<_, String>(0)?.as_str()).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?,
                name: row.get(1)?,
                command: row.get(2)?,
                description: row.get(3)?,
                category: row.get(4)?,
                tags: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
                usage_count: row.get(8)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn delete_command_snippet(&self, id: &Uuid) -> Result<()> {
        self.conn.execute(
            "DELETE FROM command_snippets WHERE id = ?",
            params![id.to_string()],
        )?;
        Ok(())
    }

    pub fn increment_usage_count(&self, id: &Uuid) -> Result<()> {
        self.conn.execute(
            "UPDATE command_snippets SET usage_count = usage_count + 1 WHERE id = ?",
            params![id.to_string()],
        )?;
        Ok(())
    }

    pub fn save_audit_event(&self, event: &AuditEvent) -> Result<()> {
        self.conn.execute(
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

    pub fn get_audit_events(&self, limit: Option<u32>) -> Result<Vec<AuditEvent>> {
        let limit = limit.unwrap_or(100);
        let mut stmt = self.conn.prepare(
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

    pub fn clear_audit_events(&self) -> Result<()> {
        self.conn.execute("DELETE FROM audit_events", [])?;
        Ok(())
    }
}
