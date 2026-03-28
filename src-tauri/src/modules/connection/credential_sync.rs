use crate::modules::connection::{
    AuthMethod, ConnectionConfig, ConnectionError, ConnectionProtocol, ProxyType,
};
use crate::modules::credential::CredentialManager;
use uuid::Uuid;

pub(crate) const PROXY_SOCKS5_PASSWORD_KIND: &str = "proxy_socks5_password";
pub(crate) const PROXY_HTTP_PASSWORD_KIND: &str = "proxy_http_password";

fn redact_proxy_password(_password: &Option<String>) -> Option<String> {
    None
}

fn maybe_fill_proxy_password(
    credential_manager: &CredentialManager,
    connection_id: &Uuid,
    kind: &str,
    password: &mut Option<String>,
) -> Result<(), ConnectionError> {
    if password.as_deref().unwrap_or_default().is_empty() {
        match credential_manager.get_password_kind(connection_id, kind) {
            Ok(Some(v)) => *password = Some(v),
            Ok(None) => {}
            Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
        }
    }
    Ok(())
}

pub fn sanitize_auth_method(auth_method: &AuthMethod) -> AuthMethod {
    match auth_method {
        AuthMethod::Password { save_password, .. } => AuthMethod::Password {
            password: String::new(),
            save_password: *save_password,
        },
        AuthMethod::KeyboardInteractive {} => AuthMethod::KeyboardInteractive {},
        AuthMethod::PrivateKey {
            key_path,
            save_passphrase,
            ..
        } => AuthMethod::PrivateKey {
            key_path: key_path.clone(),
            passphrase: None,
            save_passphrase: *save_passphrase,
        },
        AuthMethod::Agent { agent_path } => AuthMethod::Agent {
            agent_path: agent_path.clone(),
        },
        AuthMethod::Certificate {
            certificate_path,
            private_key_path,
            save_passphrase,
            ..
        } => AuthMethod::Certificate {
            certificate_path: certificate_path.clone(),
            private_key_path: private_key_path.clone(),
            passphrase: None,
            save_passphrase: *save_passphrase,
        },
    }
}

pub fn sanitize_proxy_type(proxy_type: &ProxyType) -> ProxyType {
    match proxy_type {
        ProxyType::None => ProxyType::None,
        ProxyType::Socks5 {
            host,
            port,
            username,
            password,
            has_password,
        } => ProxyType::Socks5 {
            host: host.clone(),
            port: *port,
            username: username.clone(),
            password: redact_proxy_password(password),
            has_password: *has_password || password.is_some(),
        },
        ProxyType::Http {
            host,
            port,
            username,
            password,
            has_password,
        } => ProxyType::Http {
            host: host.clone(),
            port: *port,
            username: username.clone(),
            password: redact_proxy_password(password),
            has_password: *has_password || password.is_some(),
        },
        ProxyType::JumpHost {
            host,
            port,
            username,
            auth_method,
        } => ProxyType::JumpHost {
            host: host.clone(),
            port: *port,
            username: username.clone(),
            auth_method: sanitize_auth_method(auth_method),
        },
    }
}

pub fn sanitize_config_for_storage(config: &ConnectionConfig) -> ConnectionConfig {
    let mut out = config.clone();
    out.auth_method = sanitize_auth_method(&out.auth_method);
    out.proxy_type = sanitize_proxy_type(&out.proxy_type);
    out
}

pub fn fill_saved_credentials(
    credential_manager: &CredentialManager,
    config: &mut ConnectionConfig,
) -> Result<(), ConnectionError> {
    if config.protocol != ConnectionProtocol::Ssh {
        return Ok(());
    }

    match &mut config.auth_method {
        AuthMethod::Password {
            password,
            save_password,
        } => {
            if *save_password && password.is_empty() {
                match credential_manager.get_password(&config.id) {
                    Ok(Some(v)) => *password = v,
                    Ok(None) => {}
                    Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                }
            }
        }
        AuthMethod::KeyboardInteractive {} => {}
        AuthMethod::PrivateKey {
            passphrase,
            save_passphrase,
            ..
        } => {
            if *save_passphrase && passphrase.as_deref().unwrap_or_default().is_empty() {
                match credential_manager.get_passphrase(&config.id) {
                    Ok(Some(v)) => *passphrase = Some(v),
                    Ok(None) => {}
                    Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                }
            }
        }
        AuthMethod::Agent { .. } => {}
        AuthMethod::Certificate {
            passphrase,
            save_passphrase,
            ..
        } => {
            if *save_passphrase && passphrase.as_deref().unwrap_or_default().is_empty() {
                match credential_manager.get_passphrase(&config.id) {
                    Ok(Some(v)) => *passphrase = Some(v),
                    Ok(None) => {}
                    Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                }
            }
        }
    }

    if let ProxyType::JumpHost { auth_method, .. } = &mut config.proxy_type {
        match auth_method {
            AuthMethod::Password {
                password,
                save_password,
            } => {
                if *save_password && password.is_empty() {
                    match credential_manager.get_password_kind(&config.id, "jump_password") {
                        Ok(Some(v)) => *password = v,
                        Ok(None) => {}
                        Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                    }
                }
            }
            AuthMethod::KeyboardInteractive {} => {}
            AuthMethod::PrivateKey {
                passphrase,
                save_passphrase,
                ..
            } => {
                if *save_passphrase && passphrase.as_deref().unwrap_or_default().is_empty() {
                    match credential_manager.get_password_kind(&config.id, "jump_passphrase") {
                        Ok(Some(v)) => *passphrase = Some(v),
                        Ok(None) => {}
                        Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                    }
                }
            }
            AuthMethod::Agent { .. } => {}
            AuthMethod::Certificate {
                passphrase,
                save_passphrase,
                ..
            } => {
                if *save_passphrase && passphrase.as_deref().unwrap_or_default().is_empty() {
                    match credential_manager.get_password_kind(&config.id, "jump_passphrase") {
                        Ok(Some(v)) => *passphrase = Some(v),
                        Ok(None) => {}
                        Err(e) => return Err(ConnectionError::CredentialError(e.to_string())),
                    }
                }
            }
        }
    }

    match &mut config.proxy_type {
        ProxyType::Socks5 {
            password,
            has_password,
            ..
        } => {
            maybe_fill_proxy_password(
                credential_manager,
                &config.id,
                PROXY_SOCKS5_PASSWORD_KIND,
                password,
            )?;
            if password.as_ref().is_some_and(|value| !value.is_empty()) {
                *has_password = true;
            }
        }
        ProxyType::Http {
            password,
            has_password,
            ..
        } => {
            maybe_fill_proxy_password(
                credential_manager,
                &config.id,
                PROXY_HTTP_PASSWORD_KIND,
                password,
            )?;
            if password.as_ref().is_some_and(|value| !value.is_empty()) {
                *has_password = true;
            }
        }
        _ => {}
    }

    Ok(())
}

pub fn sync_credentials_for_save(
    credential_manager: &CredentialManager,
    config: &ConnectionConfig,
) -> Result<(), ConnectionError> {
    if config.protocol != ConnectionProtocol::Ssh {
        return Ok(());
    }

    match &config.auth_method {
        AuthMethod::Password {
            password,
            save_password,
        } => {
            if *save_password {
                if !password.is_empty() {
                    credential_manager
                        .save_password(&config.id, password)
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }
            } else {
                credential_manager
                    .delete_password(&config.id)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }

            credential_manager
                .delete_passphrase(&config.id)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
        }
        AuthMethod::KeyboardInteractive {} => {
            credential_manager
                .delete_password(&config.id)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            credential_manager
                .delete_passphrase(&config.id)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
        }
        AuthMethod::PrivateKey {
            passphrase,
            save_passphrase,
            ..
        } => {
            if *save_passphrase {
                if let Some(p) = passphrase.as_ref().filter(|p| !p.is_empty()) {
                    credential_manager
                        .save_passphrase(&config.id, p)
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }
            } else {
                credential_manager
                    .delete_passphrase(&config.id)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }

            credential_manager
                .delete_password(&config.id)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
        }
        AuthMethod::Agent { .. } => {
            credential_manager
                .delete_password(&config.id)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            credential_manager
                .delete_passphrase(&config.id)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
        }
        AuthMethod::Certificate {
            passphrase,
            save_passphrase,
            ..
        } => {
            if *save_passphrase {
                if let Some(p) = passphrase.as_ref().filter(|p| !p.is_empty()) {
                    credential_manager
                        .save_passphrase(&config.id, p)
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }
            } else {
                credential_manager
                    .delete_passphrase(&config.id)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }

            credential_manager
                .delete_password(&config.id)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
        }
    }

    if let ProxyType::JumpHost { auth_method, .. } = &config.proxy_type {
        match auth_method {
            AuthMethod::Password {
                password,
                save_password,
            } => {
                if *save_password {
                    if !password.is_empty() {
                        credential_manager
                            .save_password_kind(&config.id, "jump_password", password)
                            .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    }
                } else {
                    credential_manager
                        .delete_password_kind(&config.id, "jump_password")
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }

                credential_manager
                    .delete_password_kind(&config.id, "jump_passphrase")
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
            AuthMethod::KeyboardInteractive {} => {
                credential_manager
                    .delete_password_kind(&config.id, "jump_password")
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                credential_manager
                    .delete_password_kind(&config.id, "jump_passphrase")
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
            AuthMethod::PrivateKey {
                passphrase,
                save_passphrase,
                ..
            } => {
                if *save_passphrase {
                    if let Some(p) = passphrase.as_ref().filter(|p| !p.is_empty()) {
                        credential_manager
                            .save_password_kind(&config.id, "jump_passphrase", p)
                            .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    }
                } else {
                    credential_manager
                        .delete_password_kind(&config.id, "jump_passphrase")
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }

                credential_manager
                    .delete_password_kind(&config.id, "jump_password")
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
            AuthMethod::Agent { .. } => {
                credential_manager
                    .delete_password_kind(&config.id, "jump_password")
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                credential_manager
                    .delete_password_kind(&config.id, "jump_passphrase")
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
            AuthMethod::Certificate {
                passphrase,
                save_passphrase,
                ..
            } => {
                if *save_passphrase {
                    if let Some(p) = passphrase.as_ref().filter(|p| !p.is_empty()) {
                        credential_manager
                            .save_password_kind(&config.id, "jump_passphrase", p)
                            .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                    }
                } else {
                    credential_manager
                        .delete_password_kind(&config.id, "jump_passphrase")
                        .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
                }

                credential_manager
                    .delete_password_kind(&config.id, "jump_password")
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
        }
    }

    match &config.proxy_type {
        ProxyType::Socks5 {
            password,
            has_password,
            ..
        } => {
            if let Some(secret) = password.as_deref().filter(|value| !value.is_empty()) {
                credential_manager
                    .save_password_kind(&config.id, PROXY_SOCKS5_PASSWORD_KIND, secret)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            } else if !*has_password {
                credential_manager
                    .delete_password_kind(&config.id, PROXY_SOCKS5_PASSWORD_KIND)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
            credential_manager
                .delete_password_kind(&config.id, PROXY_HTTP_PASSWORD_KIND)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
        }
        ProxyType::Http {
            password,
            has_password,
            ..
        } => {
            if let Some(secret) = password.as_deref().filter(|value| !value.is_empty()) {
                credential_manager
                    .save_password_kind(&config.id, PROXY_HTTP_PASSWORD_KIND, secret)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            } else if !*has_password {
                credential_manager
                    .delete_password_kind(&config.id, PROXY_HTTP_PASSWORD_KIND)
                    .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            }
            credential_manager
                .delete_password_kind(&config.id, PROXY_SOCKS5_PASSWORD_KIND)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
        }
        _ => {
            credential_manager
                .delete_password_kind(&config.id, PROXY_SOCKS5_PASSWORD_KIND)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
            credential_manager
                .delete_password_kind(&config.id, PROXY_HTTP_PASSWORD_KIND)
                .map_err(|e| ConnectionError::CredentialError(e.to_string()))?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::connection::{AuthMethod, ConnectionConfig, ConnectionProtocol, ProxyType};
    use crate::modules::credential::CredentialManager;
    use crate::modules::db::DatabaseManager;
    use std::fs;
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    fn temp_db_path(label: &str) -> String {
        let path =
            std::env::temp_dir().join(format!("star-shuttle-{}-{}.db", label, Uuid::new_v4()));
        path.to_string_lossy().to_string()
    }

    fn base_config(proxy_type: ProxyType) -> ConnectionConfig {
        ConnectionConfig {
            id: Uuid::new_v4(),
            name: "proxy-test".to_string(),
            protocol: ConnectionProtocol::Ssh,
            host: "127.0.0.1".to_string(),
            port: 22,
            username: "root".to_string(),
            auth_method: AuthMethod::Password {
                password: "p@ssw0rd".to_string(),
                save_password: false,
            },
            description: None,
            tags: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            group_id: None,
            local_forwards: vec![],
            remote_forwards: vec![],
            proxy_type,
            socks_proxy_port: None,
            auto_reconnect: None,
        }
    }

    #[test]
    fn sanitize_proxy_type_redacts_proxy_password() {
        let socks = ProxyType::Socks5 {
            host: "127.0.0.1".to_string(),
            port: 1080,
            username: Some("u".to_string()),
            password: Some("secret".to_string()),
            has_password: true,
        };
        let http = ProxyType::Http {
            host: "127.0.0.1".to_string(),
            port: 8080,
            username: Some("u".to_string()),
            password: Some("secret".to_string()),
            has_password: true,
        };

        match sanitize_proxy_type(&socks) {
            ProxyType::Socks5 {
                password,
                has_password,
                ..
            } => {
                assert_eq!(password, None);
                assert!(has_password);
            }
            other => panic!("unexpected proxy type: {:?}", other),
        }
        match sanitize_proxy_type(&http) {
            ProxyType::Http {
                password,
                has_password,
                ..
            } => {
                assert_eq!(password, None);
                assert!(has_password);
            }
            other => panic!("unexpected proxy type: {:?}", other),
        }
    }

    #[test]
    fn fill_saved_credentials_restores_proxy_password_from_db_fallback() {
        let db_path = temp_db_path("credential-sync");
        let db = DatabaseManager::new(&db_path).expect("db should be created");
        let db = Arc::new(Mutex::new(db));

        let mut credential_manager = CredentialManager::new();
        credential_manager.set_db(db.clone());

        let mut config = base_config(ProxyType::Socks5 {
            host: "127.0.0.1".to_string(),
            port: 1080,
            username: Some("proxy-user".to_string()),
            password: None,
            has_password: true,
        });

        {
            let guard = db.lock().expect("db lock should work");
            guard
                .save_setting(
                    &format!("credential:{}:{}", config.id, PROXY_SOCKS5_PASSWORD_KIND),
                    "proxy-secret",
                )
                .expect("seed fallback secret");
        }

        fill_saved_credentials(&credential_manager, &mut config).expect("fill should succeed");

        match config.proxy_type {
            ProxyType::Socks5 {
                password,
                has_password,
                ..
            } => {
                assert_eq!(password.as_deref(), Some("proxy-secret"));
                assert!(has_password);
            }
            other => panic!("unexpected proxy type: {:?}", other),
        }

        let _ = fs::remove_file(db_path);
    }

    #[test]
    fn sync_credentials_for_save_preserves_existing_proxy_password_when_requested() {
        let db_path = temp_db_path("credential-sync-preserve");
        let db = DatabaseManager::new(&db_path).expect("db should be created");
        let db = Arc::new(Mutex::new(db));

        let mut credential_manager = CredentialManager::new();
        credential_manager.set_db(db.clone());

        let config_id = Uuid::new_v4();
        {
            let guard = db.lock().expect("db lock should work");
            guard
                .save_setting(
                    &format!("credential:{}:{}", config_id, PROXY_SOCKS5_PASSWORD_KIND),
                    "proxy-secret",
                )
                .expect("seed fallback secret");
        }

        let config = ConnectionConfig {
            id: config_id,
            name: "proxy-test".to_string(),
            protocol: ConnectionProtocol::Ssh,
            host: "127.0.0.1".to_string(),
            port: 22,
            username: "root".to_string(),
            auth_method: AuthMethod::Password {
                password: String::new(),
                save_password: false,
            },
            description: None,
            tags: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            group_id: None,
            local_forwards: vec![],
            remote_forwards: vec![],
            proxy_type: ProxyType::Socks5 {
                host: "127.0.0.1".to_string(),
                port: 1080,
                username: Some("proxy-user".to_string()),
                password: None,
                has_password: true,
            },
            socks_proxy_port: None,
            auto_reconnect: None,
        };

        sync_credentials_for_save(&credential_manager, &config).expect("sync should succeed");

        let saved = credential_manager
            .get_password_kind(&config_id, PROXY_SOCKS5_PASSWORD_KIND)
            .expect("load should succeed");
        assert_eq!(saved.as_deref(), Some("proxy-secret"));

        let _ = fs::remove_file(db_path);
    }
}
