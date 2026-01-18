use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use uuid::Uuid;

/// Statistics for a specific channel/session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelStats {
    /// Unique identifier for the session.
    pub session_id: Uuid,
    /// Time when the session started.
    pub start_time: DateTime<Utc>,
    /// Time of the last activity (input or output).
    pub last_activity: DateTime<Utc>,
    /// Total bytes sent to the channel.
    pub bytes_sent: u64,
    /// Total bytes received from the channel.
    pub bytes_received: u64,
    /// Number of input operations.
    pub input_count: u64,
    /// Number of output operations.
    pub output_count: u64,
}

impl ChannelStats {
    /// Creates a new `ChannelStats` instance for the given session ID.
    pub fn new(session_id: Uuid) -> Self {
        Self {
            session_id,
            start_time: Utc::now(),
            last_activity: Utc::now(),
            bytes_sent: 0,
            bytes_received: 0,
            input_count: 0,
            output_count: 0,
        }
    }
}

/// Tracks channel activity and logs data.
pub struct ChannelTracker {
    stats: HashMap<Uuid, ChannelStats>,
    log_dir: PathBuf,
}

impl Default for ChannelTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelTracker {
    /// Creates a new `ChannelTracker`.
    ///
    /// Initializes a temporary directory for logging session data.
    pub fn new() -> Self {
        let mut log_dir = std::env::temp_dir();
        log_dir.push("star_shuttle_logs");

        if !log_dir.exists() {
            let _ = fs::create_dir_all(&log_dir);
        }

        Self {
            stats: HashMap::new(),
            log_dir,
        }
    }

    /// Registers a new session for tracking.
    pub fn register_session(&mut self, session_id: Uuid) {
        self.stats.insert(session_id, ChannelStats::new(session_id));
    }

    /// Logs data transfer for a session.
    ///
    /// Updates statistics and writes a log entry to disk.
    ///
    /// # Arguments
    /// * `session_id` - The UUID of the session.
    /// * `data` - The data being transferred.
    /// * `direction` - "sent" or "received".
    pub fn log_data(&mut self, session_id: Uuid, data: &[u8], direction: &str) {
        if let Some(stats) = self.stats.get_mut(&session_id) {
            stats.last_activity = Utc::now();
            match direction {
                "sent" => {
                    stats.bytes_sent += data.len() as u64;
                    stats.input_count += 1;
                }
                "received" => {
                    stats.bytes_received += data.len() as u64;
                    stats.output_count += 1;
                }
                _ => {}
            }
        }

        // Log to file
        let log_path = self.log_dir.join(format!("{}.log", session_id));
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
            let timestamp = Utc::now().to_rfc3339();
            // Simple text log: [TIMESTAMP] [DIRECTION] LENGTH bytes
            // We avoid logging full content to avoid huge files, but for audit we might need it.
            // For now, let's just log metadata to avoid filling disk.
            let _ = writeln!(file, "[{}] [{}] {} bytes", timestamp, direction, data.len());
        }
    }

    /// Retrieves statistics for a specific session.
    pub fn get_stats(&self, session_id: &Uuid) -> Option<&ChannelStats> {
        self.stats.get(session_id)
    }

    /// Retrieves statistics for all sessions.
    pub fn get_all_stats(&self) -> Vec<ChannelStats> {
        self.stats.values().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_initialization() {
        let tracker = ChannelTracker::new();
        assert!(tracker.stats.is_empty());
        assert!(tracker.log_dir.exists());
    }

    #[test]
    fn test_register_session() {
        let mut tracker = ChannelTracker::new();
        let session_id = Uuid::new_v4();
        tracker.register_session(session_id);

        let stats = tracker.get_stats(&session_id);
        assert!(stats.is_some());
        assert_eq!(stats.unwrap().session_id, session_id);
        assert_eq!(stats.unwrap().bytes_sent, 0);
    }

    #[test]
    fn test_log_data() {
        let mut tracker = ChannelTracker::new();
        let session_id = Uuid::new_v4();
        tracker.register_session(session_id);

        let data = b"hello world";
        tracker.log_data(session_id, data, "sent");

        let stats = tracker.get_stats(&session_id).unwrap();
        assert_eq!(stats.bytes_sent, data.len() as u64);
        assert_eq!(stats.input_count, 1);

        tracker.log_data(session_id, data, "received");
        let stats = tracker.get_stats(&session_id).unwrap();
        assert_eq!(stats.bytes_received, data.len() as u64);
        assert_eq!(stats.output_count, 1);
    }
}
