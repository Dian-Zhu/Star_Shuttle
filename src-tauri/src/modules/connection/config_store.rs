use serde_json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::modules::connection::{ConnectionConfig, ConnectionError};
use crate::modules::db::DatabaseManager;

/// Load connection configs previously saved in the database.
pub fn load_connection_configs_from_db(
    db: &Option<Arc<Mutex<DatabaseManager>>>,
    connections: &mut HashMap<Uuid, ConnectionConfig>,
) -> Result<(), ConnectionError> {
    let Some(db) = db else { return Ok(()) };

    let guard = db
        .lock()
        .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;
    let raw = guard
        .get_setting("connection_configs")
        .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;

    connections.clear();

    let Some(raw) = raw else { return Ok(()) };

    let configs: Vec<ConnectionConfig> =
        serde_json::from_str(&raw).map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;

    for config in configs {
        connections.insert(config.id, config);
    }

    Ok(())
}

/// Persist all current configs back to the database.
pub fn persist_connection_configs_to_db(
    db: &Option<Arc<Mutex<DatabaseManager>>>,
    connections: &HashMap<Uuid, ConnectionConfig>,
) -> Result<(), ConnectionError> {
    let Some(db) = db else { return Ok(()) };

    let mut configs: Vec<ConnectionConfig> = connections.values().cloned().collect();
    configs.sort_by_key(|c| c.updated_at);

    let raw = serde_json::to_string(&configs)
        .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;

    let guard = db
        .lock()
        .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;
    guard
        .save_setting("connection_configs", &raw)
        .map_err(|e| ConnectionError::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Helper to return a specific stored config.
pub fn get_connection_config<'a>(
    connections: &'a HashMap<Uuid, ConnectionConfig>,
    connection_id: &Uuid,
) -> Option<&'a ConnectionConfig> {
    connections.get(connection_id)
}

/// Helper to serialize all configs stored in memory.
pub fn get_all_connection_configs(
    connections: &HashMap<Uuid, ConnectionConfig>,
) -> Vec<ConnectionConfig> {
    connections.values().cloned().collect()
}
