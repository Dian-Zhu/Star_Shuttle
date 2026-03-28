use chrono::{DateTime, Utc};
use log::{debug, error, info};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use crate::modules::connection::config_store;
use crate::modules::connection::credential_sync::{
    fill_saved_credentials, sanitize_config_for_storage, sync_credentials_for_save,
    PROXY_HTTP_PASSWORD_KIND, PROXY_SOCKS5_PASSWORD_KIND,
};
use crate::modules::connection::{ConnectionConfig, ConnectionError, SessionInfo};
use crate::modules::credential::CredentialManager;
use crate::modules::db::DatabaseManager;

pub(crate) fn get_session<'a>(
    sessions: &'a HashMap<Uuid, SessionInfo>,
    session_id: &Uuid,
) -> Option<&'a SessionInfo> {
    sessions.get(session_id)
}

pub(crate) fn get_all_sessions(sessions: &HashMap<Uuid, SessionInfo>) -> Vec<SessionInfo> {
    sessions.values().cloned().collect()
}

pub(crate) fn get_connection_config<'a>(
    connections: &'a HashMap<Uuid, ConnectionConfig>,
    connection_id: &Uuid,
) -> Option<&'a ConnectionConfig> {
    config_store::get_connection_config(connections, connection_id)
}

pub(crate) fn get_all_connection_configs(
    connections: &HashMap<Uuid, ConnectionConfig>,
) -> Vec<ConnectionConfig> {
    config_store::get_all_connection_configs(connections)
}

pub(crate) fn save_connection_config(
    connections: &mut HashMap<Uuid, ConnectionConfig>,
    credential_manager: &CredentialManager,
    db: &Option<Arc<Mutex<DatabaseManager>>>,
    mut config: ConnectionConfig,
) -> Result<(), ConnectionError> {
    info!("Saving connection configuration for id: {:?}", config.id);
    debug!(
        "Validating connection configuration for id: {:?}",
        config.id
    );

    fill_saved_credentials(credential_manager, &mut config)?;
    config.validate_for_save()?;
    debug!(
        "Connection configuration validation passed for id: {:?}",
        config.id
    );

    let id = if config.id == Uuid::nil() {
        let new_id = Uuid::new_v4();
        config.id = new_id;
        debug!("Generated new ID {:?} for connection", new_id);
        new_id
    } else {
        config.id
    };

    let now = Utc::now();
    if config.created_at == DateTime::UNIX_EPOCH {
        config.created_at = now;
        debug!("Set created_at timestamp for new connection: {:?}", now);
    }
    config.updated_at = now;
    debug!("Updated updated_at timestamp for connection: {:?}", now);

    sync_credentials_for_save(credential_manager, &config)?;

    let stored = sanitize_config_for_storage(&config);
    connections.insert(id, stored);
    config_store::persist_connection_configs_to_db(db, connections)?;
    info!(
        "Successfully saved connection configuration with id: {:?}",
        id
    );
    Ok(())
}

pub(crate) fn delete_connection_config(
    connections: &mut HashMap<Uuid, ConnectionConfig>,
    credential_manager: &CredentialManager,
    db: &Option<Arc<Mutex<DatabaseManager>>>,
    connection_id: &Uuid,
) -> Result<(), ConnectionError> {
    info!(
        "Deleting connection configuration with id: {:?}",
        connection_id
    );

    if connections.remove(connection_id).is_some() {
        let _ = credential_manager.delete_password(connection_id);
        let _ = credential_manager.delete_passphrase(connection_id);
        let _ = credential_manager.delete_password_kind(connection_id, "jump_password");
        let _ = credential_manager.delete_password_kind(connection_id, "jump_passphrase");
        let _ = credential_manager.delete_password_kind(connection_id, PROXY_SOCKS5_PASSWORD_KIND);
        let _ = credential_manager.delete_password_kind(connection_id, PROXY_HTTP_PASSWORD_KIND);
        config_store::persist_connection_configs_to_db(db, connections)?;
        info!(
            "Successfully deleted connection configuration with id: {:?}",
            connection_id
        );
        Ok(())
    } else {
        error!(
            "Failed to delete connection configuration: connection not found for id: {:?}",
            connection_id
        );
        Err(ConnectionError::ConnectionNotFound(*connection_id))
    }
}
