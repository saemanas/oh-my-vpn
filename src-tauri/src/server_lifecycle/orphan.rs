//! Orphan detection and resolution for servers left running after a crash.
//!
//! On app launch, `check_orphaned_servers` reads the persisted session file,
//! queries the cloud provider to verify the server still exists, and returns
//! an `OrphanedServer` if found. `resolve_orphaned_server` dispatches to
//! destroy or reconnect paths based on the caller's chosen action.

use chrono::{DateTime, Utc};

use crate::keychain_adapter::KeychainAdapter;
use crate::provider_manager::ProviderRegistry;
use crate::session_tracker::SessionStatus;
use crate::types::{OrphanAction, OrphanedServer};
use crate::vpn_manager::keys::WireGuardKeyPair;
use crate::vpn_manager::tunnel;

use super::cleanup::cleanup_ssh_key;
use super::LifecycleError;
use super::ServerLifecycle;

impl ServerLifecycle {
    /// Check for orphaned servers by reading the persisted session file and
    /// verifying with the cloud provider.
    ///
    /// Returns a vec with at most one `OrphanedServer` (current design supports
    /// only one active session at a time). Returns an empty vec if:
    /// - No session file exists
    /// - Session file exists but the server is already gone (stale state cleared)
    ///
    /// # Errors
    ///
    /// Returns `LifecycleError::OrphanDetectionFailed` if the provider API
    /// cannot be reached or the credential is missing.
    pub async fn check_orphaned_servers(
        &self,
        registry: &tokio::sync::Mutex<ProviderRegistry>,
    ) -> Result<Vec<OrphanedServer>, LifecycleError> {
        // Read persisted session file.
        let session = match self
            .session_tracker
            .read_session()
            .map_err(|e| LifecycleError::OrphanDetectionFailed(e.to_string()))?
        {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        // Retrieve API key from Keychain.
        let credential = KeychainAdapter::retrieve_credential(&session.provider)
            .map_err(|e| LifecycleError::OrphanDetectionFailed(e.to_string()))?
            .ok_or_else(|| {
                LifecycleError::OrphanDetectionFailed(format!(
                    "No credential found for provider: {}",
                    session.provider
                ))
            })?;

        let api_key = credential.api_key;

        // Lock provider registry.
        let registry_guard = registry.lock().await;
        let cloud_provider = registry_guard.get(&session.provider).ok_or_else(|| {
            LifecycleError::OrphanDetectionFailed(format!(
                "Provider not registered: {}",
                session.provider
            ))
        })?;

        // Query provider API to check if server still exists.
        let server_exists = cloud_provider
            .get_server(&api_key, &session.server_id)
            .await
            .map_err(|e| LifecycleError::OrphanDetectionFailed(format!("{e:?}")))?
            .is_some();

        if !server_exists {
            // Server is already gone -- clear stale session file.
            self.session_tracker
                .delete_session()
                .map_err(|e| LifecycleError::OrphanDetectionFailed(e.to_string()))?;
            return Ok(vec![]);
        }

        // Server still alive -- compute estimated cost.
        let created: DateTime<Utc> = session
            .created_at
            .parse()
            .map_err(|e| LifecycleError::OrphanDetectionFailed(format!("Invalid created_at: {e}")))?;
        let elapsed_seconds = Utc::now()
            .signed_duration_since(created)
            .num_seconds()
            .max(0) as f64;
        let estimated_cost = session.hourly_cost * elapsed_seconds / 3600.0;

        Ok(vec![OrphanedServer {
            server_id: session.server_id,
            provider: session.provider,
            region: session.region,
            created_at: session.created_at,
            estimated_cost,
        }])
    }

    /// Resolve an orphaned server by destroying it or reconnecting.
    ///
    /// # Arguments
    ///
    /// - `server_id` -- provider-side server ID (must match persisted session)
    /// - `action` -- `Destroy` or `Reconnect`
    /// - `registry` -- provider registry for API access
    ///
    /// # Returns
    ///
    /// - `Destroy` → `Ok(None)` on success
    /// - `Reconnect` → `Ok(Some(SessionStatus))` on success
    pub async fn resolve_orphaned_server(
        &self,
        server_id: &str,
        action: OrphanAction,
        registry: &tokio::sync::Mutex<ProviderRegistry>,
    ) -> Result<Option<SessionStatus>, LifecycleError> {
        // Read persisted session and verify server_id matches.
        let session = self
            .session_tracker
            .read_session()
            .map_err(|e| LifecycleError::PersistenceFailed(e.to_string()))?
            .ok_or(LifecycleError::NoActiveSession)?;

        if session.server_id != server_id {
            return Err(LifecycleError::OrphanDetectionFailed(format!(
                "Session server_id mismatch: expected {}, got {server_id}",
                session.server_id
            )));
        }

        // Retrieve API key from Keychain.
        let credential = KeychainAdapter::retrieve_credential(&session.provider)
            .map_err(|e| LifecycleError::KeychainFailed(e.to_string()))?
            .ok_or_else(|| {
                LifecycleError::ProviderNotRegistered(format!(
                    "No credential found for provider: {}",
                    session.provider
                ))
            })?;

        let api_key = credential.api_key;

        // Lock provider registry.
        let registry_guard = registry.lock().await;
        let cloud_provider = registry_guard.get(&session.provider).ok_or_else(|| {
            LifecycleError::ProviderNotRegistered(format!(
                "Provider not registered: {}",
                session.provider
            ))
        })?;

        match action {
            OrphanAction::Destroy => {
                // Tunnel down (best-effort -- may already be down after crash).
                if let Err(e) = tunnel::tunnel_down_interface().await {
                    eprintln!("[warn] Tunnel teardown failed during orphan destroy (continuing): {e:?}");
                }

                // Destroy server with retry + verification + session deletion.
                self.destroy_and_cleanup(&session, cloud_provider, &api_key)
                    .await?;

                // Cleanup orphaned SSH key if present.
                if let Some(ssh_key_id) = &session.ssh_key_id {
                    cleanup_ssh_key(cloud_provider, &api_key, ssh_key_id).await;
                }

                Ok(None)
            }
            OrphanAction::Reconnect => {
                // Verify server still exists.
                let server_info = cloud_provider
                    .get_server(&api_key, &session.server_id)
                    .await
                    .map_err(|e| {
                        LifecycleError::OrphanReconnectFailed(format!(
                            "Failed to verify server: {e:?}"
                        ))
                    })?
                    .ok_or_else(|| {
                        LifecycleError::OrphanReconnectFailed(
                            "Server no longer exists -- cannot reconnect".to_string(),
                        )
                    })?;

                // Reconstruct original WireGuard key pair from stored keys.
                let client_private_key = session
                    .client_wireguard_private_key
                    .as_deref()
                    .ok_or_else(|| {
                        LifecycleError::OrphanReconnectFailed(
                            "No client WireGuard private key stored in session".to_string(),
                        )
                    })?;

                let server_public_key = session
                    .server_wireguard_public_key
                    .as_deref()
                    .ok_or_else(|| {
                        LifecycleError::OrphanReconnectFailed(
                            "No server WireGuard public key stored in session".to_string(),
                        )
                    })?;

                let client_wg_key_pair =
                    WireGuardKeyPair::from_private_key_base64(client_private_key).ok_or_else(
                        || {
                            LifecycleError::OrphanReconnectFailed(
                                "Failed to reconstruct WireGuard key pair from stored key"
                                    .to_string(),
                            )
                        },
                    )?;

                // Bring up WireGuard tunnel with original keys.
                tunnel::tunnel_up(
                    &client_wg_key_pair,
                    &server_info.public_ip,
                    server_public_key,
                    "10.0.0.2/32",
                    "1.1.1.1, 1.0.0.1",
                )
                .await
                .map_err(|e| {
                    LifecycleError::OrphanReconnectFailed(format!("Tunnel setup failed: {e:?}"))
                })?;

                // Update session timestamp (reconnect refreshes created_at for
                // accurate cost tracking going forward).
                let updated_session = crate::session_tracker::ActiveSession {
                    created_at: Utc::now().to_rfc3339(),
                    ..session
                };
                self.session_tracker
                    .create_session(&updated_session)
                    .map_err(|e| LifecycleError::PersistenceFailed(e.to_string()))?;

                // Return current session status.
                self.session_tracker
                    .get_status()
                    .map_err(|e| LifecycleError::PersistenceFailed(e.to_string()))?
                    .ok_or_else(|| {
                        LifecycleError::PersistenceFailed(
                            "Session was updated but could not be read back".to_string(),
                        )
                    })
                    .map(Some)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ProviderError;
    use crate::preferences_store::PreferencesStore;
    use crate::provider_manager::CloudProvider;
    use crate::session_tracker::{ActiveSession, SessionTracker};
    use crate::types::{Provider, RegionInfo, ServerInfo, ServerStatus};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;

    // ── Test helpers ────────────────────────────────────────────────────

    fn test_lifecycle(name: &str) -> (ServerLifecycle, PathBuf) {
        let dir = std::env::temp_dir()
            .join("oh-my-vpn-orphan-test")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        let lifecycle = ServerLifecycle {
            session_tracker: SessionTracker::new(dir.clone()),
            preferences_store: PreferencesStore::new(dir.clone()),
        };
        (lifecycle, dir)
    }

    fn sample_session() -> ActiveSession {
        ActiveSession {
            server_id: "orphan-srv-123".to_string(),
            provider: Provider::Hetzner,
            region: "fsn1".to_string(),
            server_ip: "1.2.3.4".to_string(),
            created_at: Utc::now().to_rfc3339(),
            hourly_cost: 0.007,
            ssh_key_id: Some("orphan-key-456".to_string()),
            server_wireguard_public_key: Some("c2VydmVyLXB1YmtleS1iYXNlNjQtMzItYnl0ZXMh".to_string()),
            client_wireguard_private_key: Some("Y2xpZW50LXByaXZrZXktYmFzZTY0LTMyLWJ5dGVz".to_string()),
        }
    }

    // ── Mock Provider ───────────────────────────────────────────────────

    struct MockProvider {
        server_exists: Arc<AtomicBool>,
        destroy_called: Arc<AtomicBool>,
        delete_ssh_key_called: Arc<AtomicBool>,
        get_server_call_count: Arc<AtomicUsize>,
    }

    impl MockProvider {
        fn server_gone() -> Self {
            Self {
                server_exists: Arc::new(AtomicBool::new(false)),
                destroy_called: Arc::new(AtomicBool::new(false)),
                delete_ssh_key_called: Arc::new(AtomicBool::new(false)),
                get_server_call_count: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn destroyable() -> Self {
            Self {
                server_exists: Arc::new(AtomicBool::new(true)),
                destroy_called: Arc::new(AtomicBool::new(false)),
                delete_ssh_key_called: Arc::new(AtomicBool::new(false)),
                get_server_call_count: Arc::new(AtomicUsize::new(0)),
            }
        }
    }

    #[async_trait]
    impl CloudProvider for MockProvider {
        async fn validate_credential(&self, _api_key: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn list_regions(&self, _api_key: &str) -> Result<Vec<RegionInfo>, ProviderError> {
            Ok(vec![RegionInfo {
                region: "fsn1".to_string(),
                display_name: "Falkenstein, DE".to_string(),
                instance_type: "cx22".to_string(),
                hourly_cost: 0.007,
            }])
        }

        async fn create_ssh_key(
            &self,
            _api_key: &str,
            _public_key: &str,
            _label: &str,
        ) -> Result<String, ProviderError> {
            Ok("mock-key".to_string())
        }

        async fn delete_ssh_key(
            &self,
            _api_key: &str,
            _key_id: &str,
        ) -> Result<(), ProviderError> {
            self.delete_ssh_key_called.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn create_server(
            &self,
            _api_key: &str,
            _region: &str,
            _ssh_key_id: &str,
            _cloud_init: &str,
        ) -> Result<ServerInfo, ProviderError> {
            Ok(ServerInfo {
                server_id: "mock".to_string(),
                public_ip: "0.0.0.0".to_string(),
                status: ServerStatus::Running,
            })
        }

        async fn destroy_server(
            &self,
            _api_key: &str,
            _server_id: &str,
        ) -> Result<(), ProviderError> {
            self.destroy_called.store(true, Ordering::SeqCst);
            self.server_exists.store(false, Ordering::SeqCst);
            Ok(())
        }

        async fn get_server(
            &self,
            _api_key: &str,
            _server_id: &str,
        ) -> Result<Option<ServerInfo>, ProviderError> {
            self.get_server_call_count.fetch_add(1, Ordering::SeqCst);
            if self.server_exists.load(Ordering::SeqCst) {
                Ok(Some(ServerInfo {
                    server_id: "orphan-srv-123".to_string(),
                    public_ip: "1.2.3.4".to_string(),
                    status: ServerStatus::Running,
                }))
            } else {
                Ok(None)
            }
        }
    }

    // ── Tests ───────────────────────────────────────────────────────────

    /// No session file → no orphans.
    #[test]
    fn no_session_file_returns_none() {
        let (lifecycle, _dir) = test_lifecycle("no_session");
        let result = lifecycle.session_tracker.read_session().unwrap();
        assert!(result.is_none(), "should return None when no session file exists");
    }

    /// Session exists but server is already gone → stale state cleared.
    #[tokio::test]
    async fn stale_session_cleared_when_server_gone() {
        let (lifecycle, _dir) = test_lifecycle("stale_session");
        let session = sample_session();

        lifecycle
            .session_tracker
            .create_session(&session)
            .expect("create_session should succeed");

        let mock = MockProvider::server_gone();

        let result = lifecycle
            .destroy_and_cleanup(&session, &mock, "test-api-key")
            .await;

        assert!(result.is_ok(), "should succeed when server is already gone: {result:?}");

        let remaining = lifecycle.session_tracker.read_session().unwrap();
        assert!(remaining.is_none(), "session file should be deleted");
    }

    /// Destroy path: calls destroy_server then deletes session.
    #[tokio::test]
    async fn destroy_orphan_cleans_up_session() {
        let (lifecycle, _dir) = test_lifecycle("destroy_orphan");
        let session = sample_session();

        lifecycle
            .session_tracker
            .create_session(&session)
            .expect("create_session should succeed");

        let mock = MockProvider::destroyable();
        let destroy_called = mock.destroy_called.clone();

        let result = lifecycle
            .destroy_and_cleanup(&session, &mock, "test-api-key")
            .await;

        assert!(result.is_ok(), "destroy should succeed: {result:?}");
        assert!(
            destroy_called.load(Ordering::SeqCst),
            "destroy_server should be called"
        );

        let remaining = lifecycle.session_tracker.read_session().unwrap();
        assert!(remaining.is_none(), "session file should be deleted after destroy");
    }

    /// Destroy path with SSH key cleanup.
    #[tokio::test]
    async fn destroy_orphan_cleans_up_ssh_key() {
        let (lifecycle, _dir) = test_lifecycle("destroy_ssh_key");
        let session = sample_session();

        lifecycle
            .session_tracker
            .create_session(&session)
            .expect("create_session should succeed");

        let mock = MockProvider::destroyable();
        let delete_ssh_called = mock.delete_ssh_key_called.clone();

        if let Some(ssh_key_id) = &session.ssh_key_id {
            cleanup_ssh_key(&mock, "test-api-key", ssh_key_id).await;
        }

        assert!(
            delete_ssh_called.load(Ordering::SeqCst),
            "delete_ssh_key should be called for orphaned SSH key"
        );
    }

    /// Reconnect key reconstruction round-trip.
    #[test]
    fn reconnect_key_reconstruction_matches_original() {
        let original = WireGuardKeyPair::generate();
        let private_b64 = original.private_key_base64();
        let public_b64 = original.public_key_base64();

        let reconstructed = WireGuardKeyPair::from_private_key_base64(&private_b64)
            .expect("should reconstruct from stored private key");

        assert_eq!(
            reconstructed.public_key_base64(),
            public_b64,
            "reconstructed public key should match original"
        );
    }

    /// Session with missing WG keys fails reconnect precondition.
    #[test]
    fn session_without_wg_keys_fails_reconnect_precondition() {
        let session = ActiveSession {
            server_id: "srv-no-keys".to_string(),
            provider: Provider::Hetzner,
            region: "fsn1".to_string(),
            server_ip: "1.2.3.4".to_string(),
            created_at: Utc::now().to_rfc3339(),
            hourly_cost: 0.007,
            ssh_key_id: None,
            server_wireguard_public_key: None,
            client_wireguard_private_key: None,
        };

        assert!(
            session.client_wireguard_private_key.is_none(),
            "no client WG key should block reconnect"
        );
        assert!(
            session.server_wireguard_public_key.is_none(),
            "no server WG key should block reconnect"
        );
    }

    /// Backward compatibility: old session files without WG key fields parse.
    #[test]
    fn old_session_format_deserializes_with_defaults() {
        let json = r#"{
            "serverId": "old-srv",
            "provider": "hetzner",
            "region": "fsn1",
            "serverIp": "1.2.3.4",
            "createdAt": "2026-03-05T00:00:00Z",
            "hourlyCost": 0.007,
            "sshKeyId": null
        }"#;

        let session: ActiveSession = serde_json::from_str(json)
            .expect("old format without WG keys should parse");

        assert_eq!(session.server_id, "old-srv");
        assert!(
            session.server_wireguard_public_key.is_none(),
            "missing WG pub key should default to None"
        );
        assert!(
            session.client_wireguard_private_key.is_none(),
            "missing WG priv key should default to None"
        );
    }
}
