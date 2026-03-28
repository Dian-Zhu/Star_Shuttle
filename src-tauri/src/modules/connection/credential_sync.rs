use crate::modules::connection::{
    AuthMethod, ConnectionConfig, ConnectionError, ConnectionProtocol, ProxyType,
};
use crate::modules::credential::CredentialManager;

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
        } => ProxyType::Socks5 {
            host: host.clone(),
            port: *port,
            username: username.clone(),
            password: password.as_ref().map(|_| String::new()),
        },
        ProxyType::Http {
            host,
            port,
            username,
            password,
        } => ProxyType::Http {
            host: host.clone(),
            port: *port,
            username: username.clone(),
            password: password.as_ref().map(|_| String::new()),
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

    Ok(())
}
