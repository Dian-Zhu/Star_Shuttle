use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

pub fn create_table(conn: &Connection) -> Result<()> {
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
    Ok(())
}

pub fn save(conn: &Connection, snippet: &CommandSnippet) -> Result<()> {
    conn.execute(
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

pub fn get_all(conn: &Connection) -> Result<Vec<CommandSnippet>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, command, description, category, tags, created_at, updated_at, usage_count FROM command_snippets",
    )?;
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

pub fn get_by_id(conn: &Connection, id: &Uuid) -> Result<Option<CommandSnippet>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, command, description, category, tags, created_at, updated_at, usage_count FROM command_snippets WHERE id = ?",
    )?;
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

pub fn delete(conn: &Connection, id: &Uuid) -> Result<()> {
    conn.execute(
        "DELETE FROM command_snippets WHERE id = ?",
        params![id.to_string()],
    )?;
    Ok(())
}

pub fn increment_usage_count(conn: &Connection, id: &Uuid) -> Result<()> {
    conn.execute(
        "UPDATE command_snippets SET usage_count = usage_count + 1 WHERE id = ?",
        params![id.to_string()],
    )?;
    Ok(())
}
