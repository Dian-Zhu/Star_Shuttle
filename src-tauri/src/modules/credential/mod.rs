use crate::modules::error::{AppError, Result};
use keyring::Entry;
use uuid::Uuid;

pub struct CredentialManager {
    service: String,
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
        }
    }

    fn entry(&self, connection_id: &Uuid, kind: &str) -> Result<Entry> {
        Entry::new(&self.service, &format!("{}:{}", connection_id, kind)).map_err(AppError::from)
    }

    pub fn save_password_kind(
        &self,
        connection_id: &Uuid,
        kind: &str,
        password: &str,
    ) -> Result<()> {
        let entry = self.entry(connection_id, kind)?;
        entry.set_password(password)?;
        Ok(())
    }

    pub fn get_password_kind(&self, connection_id: &Uuid, kind: &str) -> Result<Option<String>> {
        let entry = self.entry(connection_id, kind)?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(AppError::from(e)),
        }
    }

    pub fn delete_password_kind(&self, connection_id: &Uuid, kind: &str) -> Result<()> {
        let entry = self.entry(connection_id, kind)?;
        match entry.delete_password() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(AppError::from(e)),
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
