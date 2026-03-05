//! Active session tracking and monitoring.
//!
//! Tracks session metadata: server IP, connection time, estimated cost,
//! and connection health. Detects orphaned servers on app restart.

use std::fmt;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::types::Provider;

// -- Error

#[derive(Debug)]
pub enum SessionError {
    Read(String),
    Write(String),
    Parse(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Read(msg) => write!(f, "Session read failed: {msg}"),
            SessionError::Write(msg) => write!(f, "Session write failed: {msg}"),
            SessionError::Parse(msg) => write!(f, "Session parse failed: {msg}"),
        }
    }
}

impl std::error::Error for SessionError {}

// -- ActiveSession

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSession {
    pub server_id: String,
    pub provider: Provider,
    pub region: String,
    /// Human-readable region name (e.g., "Falkenstein, DE · Hetzner").
    #[serde(default)]
    pub region_display_name: String,
    pub server_ip: String,
    pub created_at: String,
    pub hourly_cost: f64,
    pub ssh_key_id: Option<String>,
    /// Server-side WireGuard public key (stored for reconnect on orphan recovery).
    #[serde(default)]
    pub server_wireguard_public_key: Option<String>,
    /// Client-side WireGuard private key (stored for reconnect on orphan recovery).
    #[serde(default)]
    pub client_wireguard_private_key: Option<String>,
}

// -- SessionStatus

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatus {
    pub provider: Provider,
    pub region: String,
    /// Human-readable region name (e.g., "Falkenstein, DE · Hetzner").
    pub region_display_name: String,
    pub server_ip: String,
    pub elapsed_seconds: u64,
    pub hourly_cost: f64,
    pub accumulated_cost: f64,
}

// -- SessionTracker

pub struct SessionTracker {
    data_dir: PathBuf,
}

impl SessionTracker {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    pub fn file_path(&self) -> PathBuf {
        self.data_dir.join("active-session.json")
    }

    pub fn create_session(&self, session: &ActiveSession) -> Result<(), SessionError> {
        fs::create_dir_all(&self.data_dir).map_err(|e| {
            SessionError::Write(format!("Failed to create data directory: {e}"))
        })?;

        let json = serde_json::to_string_pretty(session).map_err(|e| {
            SessionError::Write(format!("Failed to serialize session: {e}"))
        })?;

        let tmp_path = self.data_dir.join(".active-session.tmp.json");

        fs::write(&tmp_path, &json).map_err(|e| {
            SessionError::Write(format!("Failed to write temp file: {e}"))
        })?;

        fs::rename(&tmp_path, self.file_path()).map_err(|e| {
            SessionError::Write(format!("Failed to atomically rename session file: {e}"))
        })?;

        Ok(())
    }

    pub fn read_session(&self) -> Result<Option<ActiveSession>, SessionError> {
        let file_path = self.file_path();

        if !file_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&file_path).map_err(|e| {
            SessionError::Read(format!("Failed to read session file: {e}"))
        })?;

        let session = serde_json::from_str::<ActiveSession>(&content).map_err(|e| {
            SessionError::Parse(format!("Failed to parse session file: {e}"))
        })?;

        Ok(Some(session))
    }

    pub fn delete_session(&self) -> Result<(), SessionError> {
        match fs::remove_file(self.file_path()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(SessionError::Write(format!(
                "Failed to delete session file: {e}"
            ))),
        }
    }

    pub fn get_status(&self) -> Result<Option<SessionStatus>, SessionError> {
        let Some(session) = self.read_session()? else {
            return Ok(None);
        };

        let created: DateTime<Utc> = session.created_at.parse().map_err(|e| {
            SessionError::Parse(format!("Failed to parse created_at: {e}"))
        })?;

        let elapsed = Utc::now().signed_duration_since(created);
        let elapsed_seconds = elapsed.num_seconds().max(0) as u64;
        let accumulated_cost = session.hourly_cost * elapsed_seconds as f64 / 3600.0;

        Ok(Some(SessionStatus {
            provider: session.provider,
            region: session.region.clone(),
            region_display_name: if session.region_display_name.is_empty() {
                session.region
            } else {
                session.region_display_name
            },
            server_ip: session.server_ip,
            elapsed_seconds,
            hourly_cost: session.hourly_cost,
            accumulated_cost,
        }))
    }
}

// -- Tests

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Creates a SessionTracker backed by a unique temp directory.
    /// Cleans up any leftovers from previous runs, but does NOT create the directory --
    /// some tests verify that create_session() creates it on demand.
    fn test_tracker(name: &str) -> (SessionTracker, PathBuf) {
        let dir = std::env::temp_dir()
            .join("oh-my-vpn-session-test")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        let tracker = SessionTracker::new(dir.clone());
        (tracker, dir)
    }

    fn sample_session() -> ActiveSession {
        ActiveSession {
            server_id: "srv-123".to_string(),
            provider: Provider::Hetzner,
            region: "fsn1".to_string(),
            region_display_name: "Falkenstein, DE".to_string(),
            server_ip: "1.2.3.4".to_string(),
            created_at: Utc::now().to_rfc3339(),
            hourly_cost: 0.007,
            ssh_key_id: Some("key-456".to_string()),
            server_wireguard_public_key: None,
            client_wireguard_private_key: None,
        }
    }

    #[test]
    fn test_create_read_delete_round_trip() {
        let (tracker, _dir) = test_tracker("round_trip");
        let session = sample_session();

        tracker.create_session(&session).expect("create should succeed");

        let read = tracker.read_session().expect("read should succeed");
        assert!(read.is_some(), "session should be Some after create");
        assert_eq!(read.unwrap(), session);

        tracker.delete_session().expect("delete should succeed");

        let read_after = tracker.read_session().expect("read after delete should succeed");
        assert!(read_after.is_none(), "session should be None after delete");
    }

    #[test]
    fn test_read_missing_file_returns_none() {
        let (tracker, _dir) = test_tracker("missing_file");

        let result = tracker.read_session().expect("read on empty dir should return Ok");
        assert!(result.is_none(), "read on empty dir should return Ok(None)");
    }

    #[test]
    fn test_atomic_write_no_tmp_leftover() {
        let (tracker, dir) = test_tracker("atomic_write");

        tracker
            .create_session(&sample_session())
            .expect("create should succeed");

        let tmp_path = dir.join(".active-session.tmp.json");
        assert!(
            !tmp_path.exists(),
            ".active-session.tmp.json should not exist after create"
        );

        assert!(
            tracker.file_path().exists(),
            "active-session.json should exist"
        );
    }

    #[test]
    fn test_get_status_live_calculation() {
        let (tracker, _dir) = test_tracker("get_status");

        let two_seconds_ago = Utc::now() - chrono::Duration::seconds(2);
        let session = ActiveSession {
            server_id: "srv-abc".to_string(),
            provider: Provider::Hetzner,
            region: "fsn1".to_string(),
            region_display_name: "Falkenstein, DE".to_string(),
            server_ip: "5.6.7.8".to_string(),
            created_at: two_seconds_ago.to_rfc3339(),
            hourly_cost: 0.007,
            ssh_key_id: None,
            server_wireguard_public_key: None,
            client_wireguard_private_key: None,
        };

        tracker.create_session(&session).expect("create should succeed");

        let status = tracker
            .get_status()
            .expect("get_status should succeed")
            .expect("status should be Some");

        assert!(
            status.elapsed_seconds >= 2,
            "elapsed_seconds should be >= 2, got {}",
            status.elapsed_seconds
        );
        assert!(
            status.accumulated_cost > 0.0,
            "accumulated_cost should be > 0.0, got {}",
            status.accumulated_cost
        );
    }

    #[test]
    fn test_delete_nonexistent_file_is_ok() {
        let (tracker, _dir) = test_tracker("delete_nonexistent");

        let result = tracker.delete_session();
        assert!(
            result.is_ok(),
            "delete on empty tracker should return Ok(()), got {:?}",
            result
        );
    }
}
