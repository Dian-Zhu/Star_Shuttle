use crate::modules::db::DatabaseManager;
use crate::modules::error::{AppError, Result};
use keyring::Entry;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub struct CredentialManager {
    service: String,
    db: Option<Arc<Mutex<DatabaseManager>>>,
}

impl Default for CredentialManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialManager {
    pub fn new() -> Self {
        Self {
            service: "ssh_remote_manager".to_string(),
            db: None,
        }
    }

    pub fn set_db(&mut self, db: Arc<Mutex<DatabaseManager>>) {
        self.db = Some(db);
    }

    fn entry(&self, connection_id: &Uuid, kind: &str) -> Result<Entry> {
        Entry::new(&self.service, &format!("{}:{}", connection_id, kind)).map_err(AppError::from)
    }

    fn fallback_key(connection_id: &Uuid, kind: &str) -> String {
        format!("credential:{}:{}", connection_id, kind)
    }

    fn fallback_save(&self, connection_id: &Uuid, kind: &str, _password: &str) -> Result<()> {
        let _ = (connection_id, kind);
        Err(AppError::CredentialError(
            "secure storage unavailable; refusing to save plaintext credentials".to_string(),
        ))
    }

    fn fallback_get(&self, connection_id: &Uuid, kind: &str) -> Result<Option<String>> {
        let Some(db) = self.db.as_ref() else {
            return Ok(None);
        };
        let db = db
            .lock()
            .map_err(|e| AppError::CredentialError(e.to_string()))?;
        Ok(db.get_setting(&Self::fallback_key(connection_id, kind))?)
    }

    fn fallback_delete(&self, connection_id: &Uuid, kind: &str) -> Result<()> {
        let Some(db) = self.db.as_ref() else {
            return Ok(());
        };
        let db = db
            .lock()
            .map_err(|e| AppError::CredentialError(e.to_string()))?;
        db.delete_setting(&Self::fallback_key(connection_id, kind))?;
        Ok(())
    }

    pub fn save_password_kind(
        &self,
        connection_id: &Uuid,
        kind: &str,
        password: &str,
    ) -> Result<()> {
        match self.entry(connection_id, kind) {
            Ok(entry) => {
                if let Err(e) = entry.set_password(password) {
                    self.fallback_save(connection_id, kind, password)
                        .map_err(|fallback_err| {
                            AppError::CredentialError(format!(
                                "secure storage failure: {e}; fallback failure: {fallback_err}"
                            ))
                        })?;
                } else {
                    let _ = self.fallback_delete(connection_id, kind);
                }
                Ok(())
            }
            Err(e) => self
                .fallback_save(connection_id, kind, password)
                .map_err(|fallback_err| {
                    AppError::CredentialError(format!(
                        "secure storage failure: {e}; fallback failure: {fallback_err}"
                    ))
                }),
        }
    }

    pub fn get_password_kind(&self, connection_id: &Uuid, kind: &str) -> Result<Option<String>> {
        let Ok(entry) = self.entry(connection_id, kind) else {
            return self.fallback_get(connection_id, kind);
        };
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => self.fallback_get(connection_id, kind),
            Err(_) => self.fallback_get(connection_id, kind),
        }
    }

    pub fn delete_password_kind(&self, connection_id: &Uuid, kind: &str) -> Result<()> {
        let keyring_err = match self.entry(connection_id, kind) {
            Ok(entry) => match entry.delete_password() {
                Ok(()) => None,
                Err(keyring::Error::NoEntry) => None,
                Err(e) => Some(e.to_string()),
            },
            Err(e) => Some(e.to_string()),
        };

        let fallback_res = self.fallback_delete(connection_id, kind);
        match (keyring_err, fallback_res) {
            (_, Ok(())) => Ok(()),
            (Some(keyring_err), Err(fallback_err)) => Err(AppError::CredentialError(format!(
                "secure storage failure: {keyring_err}; fallback failure: {fallback_err}"
            ))),
            (None, Err(fallback_err)) => Err(fallback_err),
        }
    }

    pub fn save_password(&self, connection_id: &Uuid, password: &str) -> Result<()> {
        self.save_password_kind(connection_id, "password", password)
    }

    pub fn get_password(&self, connection_id: &Uuid) -> Result<Option<String>> {
        self.get_password_kind(connection_id, "password")
    }

    pub fn delete_password(&self, connection_id: &Uuid) -> Result<()> {
        self.delete_password_kind(connection_id, "password")
    }

    pub fn save_passphrase(&self, connection_id: &Uuid, passphrase: &str) -> Result<()> {
        self.save_password_kind(connection_id, "passphrase", passphrase)
    }

    pub fn get_passphrase(&self, connection_id: &Uuid) -> Result<Option<String>> {
        self.get_password_kind(connection_id, "passphrase")
    }

    pub fn delete_passphrase(&self, connection_id: &Uuid) -> Result<()> {
        self.delete_password_kind(connection_id, "passphrase")
    }
}
