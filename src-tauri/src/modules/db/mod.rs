use rusqlite::{params, Connection, Result}; use serde::{Deserialize, Serialize}; use uuid::Uuid;

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
                id: Uuid::parse_str(row.get::<_, String>(0)?.as_str())
                    .map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)))?,
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
}