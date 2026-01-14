use serde::{Deserialize, Serialize};
use russh_keys::key::KeyPair;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AuthMethod {
    Password(String),
    PrivateKey(String, Option<String>), // path, optional passphrase
    Certificate(String, String, Option<String>), // cert path, key path, optional passphrase
    Agent,
    Kerberos,
}

impl AuthMethod {
    pub fn is_password(&self) -> bool {
        matches!(self, AuthMethod::Password(_))
    }
    
    pub fn is_private_key(&self) -> bool {
        matches!(self, AuthMethod::PrivateKey(_, _))
    }
    
    pub fn is_certificate(&self) -> bool {
        matches!(self, AuthMethod::Certificate(_, _, _))
    }
    
    pub fn is_agent(&self) -> bool {
        matches!(self, AuthMethod::Agent)
    }
    
    pub fn is_kerberos(&self) -> bool {
        matches!(self, AuthMethod::Kerberos)
    }
    
    pub fn get_private_key_path(&self) -> Option<&str> {
        match self {
            AuthMethod::PrivateKey(path, _) => Some(path),
            AuthMethod::Certificate(_, path, _) => Some(path),
            _ => None,
        }
    }
    
    pub fn get_passphrase(&self) -> Option<&str> {
        match self {
            AuthMethod::PrivateKey(_, passphrase) => passphrase.as_deref(),
            AuthMethod::Certificate(_, _, passphrase) => passphrase.as_deref(),
            _ => None,
        }
    }
    
    // Load private key from file
    pub fn load_private_key(&self) -> Result<KeyPair, anyhow::Error> {
        match self {
            AuthMethod::PrivateKey(path, _passphrase) => {
                // TODO: Implement actual private key loading with russh-keys 0.40
                Err(anyhow::anyhow!("Private key loading not implemented yet: {}", path))
            },
            AuthMethod::Certificate(_, path, _passphrase) => {
                // TODO: Implement actual certificate loading with russh-keys 0.40
                Err(anyhow::anyhow!("Certificate loading not implemented yet: {}", path))
            },
            _ => Err(anyhow::anyhow!("Not a private key authentication method")),
        }
    }
    
    // Get password for password authentication
    pub fn get_password(&self) -> Option<&str> {
        match self {
            AuthMethod::Password(password) => Some(password),
            _ => None,
        }
    }
}