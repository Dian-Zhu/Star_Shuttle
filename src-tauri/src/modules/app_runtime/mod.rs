use crate::modules::db::DatabaseManager;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Debug, Default)]
pub struct AppLockRuntimeState {
    pub unlocked: bool,
}

pub(crate) const APP_LOCKED_ERROR: &str = "App is locked. Please unlock first.";
const HOST_KEY_CHALLENGE_TTL: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostKeyChallengeType {
    Unknown,
    Mismatch,
    Unavailable,
}

impl HostKeyChallengeType {
    fn marker_prefix(self) -> &'static str {
        match self {
            Self::Unknown => "HOST_KEY_UNKNOWN|",
            Self::Mismatch => "HOST_KEY_MISMATCH|",
            Self::Unavailable => "HOST_KEY_UNAVAILABLE|",
        }
    }

    fn expected_replace(self) -> bool {
        matches!(self, Self::Mismatch)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct HostKeyChallengePayload {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub key_type: String,
    pub key_base64: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub challenge_token: Option<String>,
}

#[derive(Debug, Clone)]
struct HostKeyChallengeEntry {
    host: String,
    port: u16,
    key_type: String,
    key_base64: String,
    expected_replace: bool,
    expires_at: Instant,
}

#[derive(Debug, Default)]
pub struct HostKeyChallengeRuntimeState {
    pending: HashMap<String, HostKeyChallengeEntry>,
}

impl HostKeyChallengeRuntimeState {
    fn prune_expired(&mut self) {
        let now = Instant::now();
        self.pending.retain(|_, entry| entry.expires_at > now);
    }

    fn issue(
        &mut self,
        challenge_type: HostKeyChallengeType,
        payload: &HostKeyChallengePayload,
    ) -> String {
        self.prune_expired();
        let token = Uuid::new_v4().to_string();
        self.pending.insert(
            token.clone(),
            HostKeyChallengeEntry {
                host: payload.host.clone(),
                port: payload.port,
                key_type: payload.key_type.clone(),
                key_base64: payload.key_base64.clone(),
                expected_replace: challenge_type.expected_replace(),
                expires_at: Instant::now() + HOST_KEY_CHALLENGE_TTL,
            },
        );
        token
    }

    pub fn consume(
        &mut self,
        challenge_token: &str,
        host: &str,
        port: u16,
        key_type: &str,
        key_base64: &str,
        replace: bool,
    ) -> Result<(), String> {
        self.prune_expired();
        let Some(entry) = self.pending.remove(challenge_token) else {
            return Err("Invalid or expired host key challenge token".to_string());
        };

        if entry.host != host
            || entry.port != port
            || entry.key_type != key_type
            || entry.key_base64 != key_base64
            || entry.expected_replace != replace
        {
            return Err("Host key challenge token does not match payload".to_string());
        }

        Ok(())
    }
}

pub(crate) fn parse_host_key_error_payload(
    error: &str,
) -> Option<(String, HostKeyChallengePayload)> {
    const MARKERS: [(HostKeyChallengeType, &str); 3] = [
        (HostKeyChallengeType::Unknown, "HOST_KEY_UNKNOWN|"),
        (HostKeyChallengeType::Mismatch, "HOST_KEY_MISMATCH|"),
        (HostKeyChallengeType::Unavailable, "HOST_KEY_UNAVAILABLE|"),
    ];

    for (_challenge_type, marker) in MARKERS {
        let Some(idx) = error.rfind(marker) else {
            continue;
        };
        let json_part = error[idx + marker.len()..].trim();
        if let Ok(payload) = serde_json::from_str::<HostKeyChallengePayload>(json_part) {
            return Some((marker.to_string(), payload));
        }
    }

    None
}

fn parse_host_key_payload(error: &str) -> Option<(HostKeyChallengeType, HostKeyChallengePayload)> {
    let (marker, payload) = parse_host_key_error_payload(error)?;
    let challenge_type = match marker.as_str() {
        "HOST_KEY_UNKNOWN|" => HostKeyChallengeType::Unknown,
        "HOST_KEY_MISMATCH|" => HostKeyChallengeType::Mismatch,
        "HOST_KEY_UNAVAILABLE|" => HostKeyChallengeType::Unavailable,
        _ => return None,
    };
    Some((challenge_type, payload))
}

pub(crate) fn enrich_host_key_error_with_challenge(
    error: String,
    challenge_state: &Arc<Mutex<HostKeyChallengeRuntimeState>>,
) -> String {
    let Some((challenge_type, mut payload)) = parse_host_key_payload(&error) else {
        return error;
    };

    let token = {
        let mut state = match challenge_state.lock() {
            Ok(state) => state,
            Err(_) => return error,
        };
        state.issue(challenge_type, &payload)
    };
    payload.challenge_token = Some(token);

    match serde_json::to_string(&payload) {
        Ok(json) => format!("{}{}", challenge_type.marker_prefix(), json),
        Err(_) => error,
    }
}

pub(crate) fn ensure_app_unlocked_runtime(
    db: &Arc<Mutex<DatabaseManager>>,
    app_lock_state: &Arc<Mutex<AppLockRuntimeState>>,
) -> Result<(), String> {
    let lock_enabled = {
        let db = db.lock().map_err(|e| e.to_string())?;
        db.get_setting("app_lock_hash")
            .map_err(|e| e.to_string())?
            .is_some()
    };

    if !lock_enabled {
        let mut state = app_lock_state.lock().map_err(|e| e.to_string())?;
        state.unlocked = true;
        return Ok(());
    }

    let state = app_lock_state.lock().map_err(|e| e.to_string())?;
    if state.unlocked {
        Ok(())
    } else {
        Err(APP_LOCKED_ERROR.to_string())
    }
}
