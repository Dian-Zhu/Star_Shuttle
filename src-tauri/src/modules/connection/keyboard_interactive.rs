use crate::modules::connection::ssh_impl;
use anyhow::anyhow;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;
use uuid::Uuid;

pub const SSH_KEYBOARD_INTERACTIVE_EVENT: &str = "ssh-keyboard-interactive-request";

type KeyboardInteractivePending =
    Arc<Mutex<HashMap<String, oneshot::Sender<Result<Vec<String>, String>>>>>;

#[derive(Clone)]
pub struct KeyboardInteractiveCoordinator {
    pending: KeyboardInteractivePending,
}

impl Default for KeyboardInteractiveCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardInteractiveCoordinator {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn respond(&self, request_id: String, responses: Vec<String>) -> Result<(), String> {
        let tx = self
            .pending
            .lock()
            .map_err(|e| e.to_string())?
            .remove(&request_id)
            .ok_or_else(|| "unknown request_id".to_string())?;
        let _ = tx.send(Ok(responses));
        Ok(())
    }

    pub fn cancel(&self, request_id: String) -> Result<(), String> {
        let tx = self
            .pending
            .lock()
            .map_err(|e| e.to_string())?
            .remove(&request_id)
            .ok_or_else(|| "unknown request_id".to_string())?;
        let _ = tx.send(Err("canceled".to_string()));
        Ok(())
    }

    fn remove_pending_request(
        &self,
        request_id: &str,
    ) -> Result<Option<oneshot::Sender<Result<Vec<String>, String>>>, anyhow::Error> {
        let removed = self
            .pending
            .lock()
            .map_err(|e| anyhow!(e.to_string()))?
            .remove(request_id);
        Ok(removed)
    }

    #[cfg(test)]
    pub fn pending_len_for_test(&self) -> usize {
        self.pending.lock().map(|m| m.len()).unwrap_or_default()
    }

    pub async fn request_with_emit<F>(
        &self,
        request: ssh_impl::KeyboardInteractivePromptRequest,
        emit: F,
    ) -> Result<Vec<String>, anyhow::Error>
    where
        F: FnOnce(serde_json::Value) -> Result<(), anyhow::Error>,
    {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<Result<Vec<String>, String>>();
        {
            let mut guard = self.pending.lock().map_err(|e| anyhow!(e.to_string()))?;
            guard.insert(request_id.clone(), tx);
        }

        let payload = serde_json::json!({
            "request_id": request_id,
            "host": request.host,
            "port": request.port,
            "username": request.username,
            "name": request.name,
            "instructions": request.instructions,
            "prompts": request.prompts.iter().map(|p| serde_json::json!({
                "prompt": p.prompt,
                "echo": p.echo
            })).collect::<Vec<_>>()
        });

        if let Err(err) = emit(payload) {
            let _ = self.remove_pending_request(&request_id);
            return Err(err);
        }

        let res = tokio::time::timeout(std::time::Duration::from_secs(300), rx).await;
        match res {
            Ok(Ok(Ok(v))) => Ok(v),
            Ok(Ok(Err(e))) => Err(anyhow!(e)),
            Ok(Err(_)) => {
                let _ = self.remove_pending_request(&request_id);
                Err(anyhow!("keyboard-interactive response channel closed"))
            }
            Err(_) => {
                let _ = self.remove_pending_request(&request_id);
                Err(anyhow!("keyboard-interactive prompt timeout"))
            }
        }
    }

    pub async fn request(
        &self,
        app: &AppHandle,
        request: ssh_impl::KeyboardInteractivePromptRequest,
    ) -> Result<Vec<String>, anyhow::Error> {
        self.request_with_emit(request, |payload| {
            app.emit(SSH_KEYBOARD_INTERACTIVE_EVENT, payload)
                .map_err(|e| anyhow!(e.to_string()))
        })
        .await
    }
}

#[derive(Clone)]
pub(crate) struct TauriKeyboardInteractivePrompter {
    pub app: AppHandle,
    pub coordinator: KeyboardInteractiveCoordinator,
}

#[async_trait]
impl ssh_impl::KeyboardInteractivePrompter for TauriKeyboardInteractivePrompter {
    async fn prompt(
        &self,
        request: ssh_impl::KeyboardInteractivePromptRequest,
    ) -> Result<Vec<String>, anyhow::Error> {
        self.coordinator.request(&self.app, request).await
    }
}
