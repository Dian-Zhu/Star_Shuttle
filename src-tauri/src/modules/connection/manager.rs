use std::collections::HashMap; use uuid::Uuid; use chrono::Utc;

pub struct ConnectionManagerImpl {
    connections: HashMap<Uuid, super::ConnectionConfig>,
    sessions: HashMap<Uuid, super::SessionInfo>,
}

impl Default for ConnectionManagerImpl {
    fn default() -> Self {
        Self {
            connections: HashMap::new(),
            sessions: HashMap::new(),
        }
    }
}

impl super::ConnectionManager for ConnectionManagerImpl {
    fn connect(
        &mut self,
        config: &super::ConnectionConfig,
    ) -> std::result::Result<Uuid, super::ConnectionError> {
        // 新增：验证配置合法性，确保不使用默认空配置
        config.validate()?;
        
        let session_id = Uuid::new_v4();

        // Create session info
        let mut session = super::SessionInfo {
            id: session_id,
            connection_id: config.id,
            status: super::ConnectionStatus::Connecting,
            terminal_id: None,
            created_at: Utc::now(),
            last_active: Utc::now(),
        };

        self.sessions.insert(session_id, session.clone());

        // TODO: Implement actual SSH connection logic with russh
        // For now, we'll just simulate a successful connection
        session.status = super::ConnectionStatus::Connected;
        self.sessions.insert(session_id, session);

        Ok(session_id)
    }

    fn disconnect(&mut self, session_id: &Uuid) -> std::result::Result<(), super::ConnectionError> {
        let session = self.sessions.get_mut(session_id).ok_or(super::ConnectionError::SessionNotFound(*session_id))?;
        
        session.status = super::ConnectionStatus::Disconnecting;
        
        // TODO: Implement actual SSH disconnection logic with russh
        // For now, just simulate disconnection
        session.status = super::ConnectionStatus::Disconnected;
        
        Ok(())
    }

    fn get_session(&self, session_id: &Uuid) -> Option<&super::SessionInfo> {
        self.sessions.get(session_id)
    }

    fn get_all_sessions(&self) -> Vec<super::SessionInfo> {
        self.sessions.values().cloned().collect()
    }

    fn get_connection_config(&self, connection_id: &Uuid) -> Option<&super::ConnectionConfig> {
        self.connections.get(connection_id)
    }

    fn save_connection_config(
        &mut self,
        config: super::ConnectionConfig,
    ) -> std::result::Result<(), super::ConnectionError> {
        self.connections.insert(config.id, config);
        Ok(())
    }

    fn delete_connection_config(
        &mut self,
        connection_id: &Uuid,
    ) -> std::result::Result<(), super::ConnectionError> {
        if self.connections.remove(connection_id).is_some() {
            // Also remove related sessions
            self.sessions.retain(|_, session| session.connection_id != *connection_id);
            Ok(())
        } else {
            Err(super::ConnectionError::ConnectionNotFound(*connection_id))
        }
    }

    fn get_all_connection_configs(&self) -> Vec<super::ConnectionConfig> {
        self.connections.values().cloned().collect()
    }

    fn test_connection(
        &self,
        config: &super::ConnectionConfig,
    ) -> std::result::Result<(), super::ConnectionError> {
        // 新增：验证配置合法性
        config.validate()?;
        
        // TODO: Implement actual connection testing logic
        // For now, just simulate a successful test
        Ok(())
    }
}
