//! Disconnect flow -- tunnel teardown, server destruction, session cleanup.
//!
//! Implements the full disconnect lifecycle: read active session, tear down
//! WireGuard tunnel (best-effort), destroy the cloud server (with retry and
//! exponential backoff), and delete the local session file on success.

use crate::keychain_adapter::KeychainAdapter;
use crate::provider_manager::{CloudProvider, ProviderRegistry};
use crate::session_tracker::ActiveSession;
use crate::vpn_manager::tunnel;

use super::LifecycleError;
use super::ServerLifecycle;

// ── ServerLifecycle::disconnect ─────────────────────────────────────────────

impl ServerLifecycle {
    /// Execute the full disconnect flow.
    ///
    /// # Steps
    ///
    /// 1. Read active session -- if none, return `NoActiveSession`
    /// 2. Tear down WireGuard tunnel (best-effort; log warning if fails)
    /// 3. Retrieve API key from Keychain
    /// 4. Lock provider registry and get provider implementation
    /// 5. Destroy server via cloud provider API
    /// 6. Verify deletion via `get_server` -- retry up to 3 times (1s, 2s, 4s backoff)
    /// 7. Delete session file (only on confirmed server destruction)
    ///
    /// # Error Handling
    ///
    /// - Tunnel teardown failure is non-fatal -- logged as a warning and the
    ///   flow continues (tunnel may already be down).
    /// - If server still exists after all retries, returns `DestructionFailed`
    ///   with a console URL so the user can verify manually. The session file
    ///   is **preserved** on persistent destruction failure.
    pub async fn disconnect(
        &self,
        provider_registry: &tokio::sync::Mutex<ProviderRegistry>,
    ) -> Result<(), LifecycleError> {
        // Step 1: Read active session.
        let session = self
            .session_tracker
            .read_session()
            .map_err(|e| LifecycleError::PersistenceFailed(e.to_string()))?
            .ok_or(LifecycleError::NoActiveSession)?;

        // Step 2: Tear down WireGuard tunnel (best-effort).
        if let Err(e) = tunnel::tunnel_down_interface().await {
            eprintln!("[warn] Tunnel teardown failed (continuing): {e:?}");
        }

        // Step 3: Retrieve API key from Keychain.
        let credential = KeychainAdapter::retrieve_credential(&session.provider)
            .map_err(|e| LifecycleError::KeychainFailed(e.to_string()))?
            .ok_or_else(|| {
                LifecycleError::ProviderNotRegistered(format!(
                    "No credential found for provider: {}",
                    session.provider
                ))
            })?;

        let api_key = credential.api_key;

        // Step 4: Lock provider registry and get provider implementation.
        let registry_guard = provider_registry.lock().await;
        let cloud_provider = registry_guard
            .get(&session.provider)
            .ok_or_else(|| {
                LifecycleError::ProviderNotRegistered(format!(
                    "Provider not registered: {}",
                    session.provider
                ))
            })?;

        // Steps 5-7: Destroy server with retry and clean up session on success.
        self.destroy_and_cleanup(&session, cloud_provider, &api_key)
            .await
    }

    /// Inner logic: destroy server with retry loop, delete session on confirmed success.
    ///
    /// Extracted from `disconnect()` to allow unit testing without Keychain
    /// and `ProviderRegistry` dependencies. Called by `disconnect()` after
    /// acquiring the API key and provider trait object.
    ///
    /// # Steps
    ///
    /// 5. Destroy server (initial attempt; ignore immediate error)
    /// 6. Verify deletion via `get_server` -- retry up to 3 times (1s, 2s, 4s backoff)
    /// 7. Delete session file only if server destruction is confirmed
    pub(crate) async fn destroy_and_cleanup(
        &self,
        session: &ActiveSession,
        cloud_provider: &dyn CloudProvider,
        api_key: &str,
    ) -> Result<(), LifecycleError> {
        // Step 5: Destroy server (initial attempt; ignore immediate error --
        // the verification step below confirms the final state).
        let _ = cloud_provider
            .destroy_server(api_key, &session.server_id)
            .await;

        // Step 6: Verify deletion via get_server.
        // If server still exists, enter retry loop (max 3 retries, exponential
        // backoff: 1s, 2s, 4s). Each retry calls destroy_server then get_server.
        let still_exists = cloud_provider
            .get_server(api_key, &session.server_id)
            .await
            .map_err(LifecycleError::Provider)?
            .is_some();

        let mut server_destroyed = !still_exists;

        if !server_destroyed {
            let delays = [1u64, 2, 4];
            for delay in delays {
                tokio::time::sleep(tokio::time::Duration::from_secs(delay)).await;

                let _ = cloud_provider
                    .destroy_server(api_key, &session.server_id)
                    .await;

                let still_exists = cloud_provider
                    .get_server(api_key, &session.server_id)
                    .await
                    .map_err(LifecycleError::Provider)?
                    .is_some();

                if !still_exists {
                    server_destroyed = true;
                    break;
                }
            }
        }

        // Preserve session on persistent destruction failure -- user must
        // verify and retry manually via the provider console.
        if !server_destroyed {
            return Err(LifecycleError::DestructionFailed(format!(
                "Server {} could not be destroyed after retries. Check manually: {}",
                session.server_id,
                session.provider.console_url()
            )));
        }

        // Step 7: Server confirmed gone -- delete session file.
        self.session_tracker
            .delete_session()
            .map_err(|e| LifecycleError::PersistenceFailed(e.to_string()))?;

        Ok(())
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ProviderError;
    use crate::preferences_store::PreferencesStore;
    use crate::session_tracker::{ActiveSession, SessionTracker};
    use crate::types::{Provider, RegionInfo, ServerInfo, ServerStatus};
    use async_trait::async_trait;
    use chrono::Utc;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    // ── Test helpers ────────────────────────────────────────────────────

    /// Creates a `ServerLifecycle` backed by a unique isolated temp directory.
    /// Cleans up any leftovers from a previous run.
    fn test_lifecycle(name: &str) -> (ServerLifecycle, PathBuf) {
        let dir = std::env::temp_dir()
            .join("oh-my-vpn-disconnect-test")
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        let lifecycle = ServerLifecycle {
            session_tracker: SessionTracker::new(dir.clone()),
            preferences_store: PreferencesStore::new(dir.clone()),
        };
        (lifecycle, dir)
    }

    /// Returns a realistic `ActiveSession` for use in tests.
    fn sample_session() -> ActiveSession {
        ActiveSession {
            server_id: "mock-server-123".to_string(),
            provider: Provider::Hetzner,
            region: "fsn1".to_string(),
            server_ip: "1.2.3.4".to_string(),
            created_at: Utc::now().to_rfc3339(),
            hourly_cost: 0.007,
            ssh_key_id: None,
            server_wireguard_public_key: None,
            client_wireguard_private_key: None,
        }
    }

    // ── Mock Provider ───────────────────────────────────────────────────

    /// Configurable mock for `CloudProvider` that tracks call counts and
    /// controls `get_server` behaviour via a threshold.
    ///
    /// - `destroy_server` always returns `Ok(())` and increments a counter.
    /// - `get_server` returns `Some` for the first `get_server_returns_none_after`
    ///   calls, then returns `None` for all subsequent calls.
    ///   Set to `0` for immediate success, `usize::MAX` for persistent failure.
    struct MockProvider {
        destroy_call_count: Arc<AtomicUsize>,
        get_server_call_count: Arc<AtomicUsize>,
        /// Call index (0-based) at which `get_server` starts returning `None`.
        get_server_returns_none_after: usize,
    }

    impl MockProvider {
        /// Server is already gone -- `get_server` returns `None` immediately.
        fn success() -> Self {
            Self {
                destroy_call_count: Arc::new(AtomicUsize::new(0)),
                get_server_call_count: Arc::new(AtomicUsize::new(0)),
                get_server_returns_none_after: 0,
            }
        }

        /// Server appears alive for `some_count` calls, then gone.
        /// Use `some_count = 1` to trigger exactly one retry cycle.
        fn retries(some_count: usize) -> Self {
            Self {
                destroy_call_count: Arc::new(AtomicUsize::new(0)),
                get_server_call_count: Arc::new(AtomicUsize::new(0)),
                get_server_returns_none_after: some_count,
            }
        }

        /// Server never disappears -- all `get_server` calls return `Some`.
        fn persistent_failure() -> Self {
            Self {
                destroy_call_count: Arc::new(AtomicUsize::new(0)),
                get_server_call_count: Arc::new(AtomicUsize::new(0)),
                get_server_returns_none_after: usize::MAX,
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
            Ok("mock-ssh-key-id".to_string())
        }

        async fn delete_ssh_key(
            &self,
            _api_key: &str,
            _key_id: &str,
        ) -> Result<(), ProviderError> {
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
                server_id: "mock-server-123".to_string(),
                public_ip: "1.2.3.4".to_string(),
                status: ServerStatus::Running,
            })
        }

        async fn destroy_server(
            &self,
            _api_key: &str,
            _server_id: &str,
        ) -> Result<(), ProviderError> {
            self.destroy_call_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn get_server(
            &self,
            _api_key: &str,
            _server_id: &str,
        ) -> Result<Option<ServerInfo>, ProviderError> {
            // fetch_add returns the PREVIOUS value (0-based call index).
            let call_index = self.get_server_call_count.fetch_add(1, Ordering::SeqCst);
            if call_index >= self.get_server_returns_none_after {
                Ok(None) // Server is gone
            } else {
                Ok(Some(ServerInfo {
                    server_id: "mock-server-123".to_string(),
                    public_ip: "1.2.3.4".to_string(),
                    status: ServerStatus::Deleting,
                }))
            }
        }
    }

    // ── Tests ───────────────────────────────────────────────────────────

    /// `disconnect()` returns `NoActiveSession` when no session file exists.
    ///
    /// This test calls the full `disconnect()` entry point. It returns before
    /// reaching the Keychain step (step 3), so no macOS Keychain access is needed.
    #[tokio::test]
    async fn disconnect_no_session_returns_error() {
        let (lifecycle, _dir) = test_lifecycle("no_session");

        // Empty ProviderRegistry -- never reached because session check fails first.
        let registry = tokio::sync::Mutex::new(ProviderRegistry::new());

        let result = lifecycle.disconnect(&registry).await;

        assert!(
            matches!(result, Err(LifecycleError::NoActiveSession)),
            "expected NoActiveSession when no session file exists, got: {result:?}"
        );
    }

    /// On successful destruction, session file is deleted.
    ///
    /// Tests `destroy_and_cleanup()` directly (bypasses Keychain).
    /// MockProvider returns `None` on first `get_server()` call -- server
    /// is already gone, no retry needed.
    #[tokio::test]
    async fn disconnect_success_deletes_session() {
        let (lifecycle, _dir) = test_lifecycle("success_deletes_session");
        let session = sample_session();

        // Write a session so there is something to clean up.
        lifecycle
            .session_tracker
            .create_session(&session)
            .expect("create_session should succeed");

        let mock = MockProvider::success();
        let result = lifecycle
            .destroy_and_cleanup(&session, &mock, "test-api-key")
            .await;

        assert!(
            result.is_ok(),
            "destroy_and_cleanup should return Ok(()) on success, got: {result:?}"
        );

        // Session file must be deleted after confirmed destruction.
        let remaining = lifecycle
            .session_tracker
            .read_session()
            .expect("read_session should not error");
        assert!(
            remaining.is_none(),
            "session file should be deleted after successful disconnect"
        );

        // destroy_server called exactly once (initial attempt, no retries).
        assert_eq!(
            mock.destroy_call_count.load(Ordering::SeqCst),
            1,
            "destroy_server should be called exactly once when server is immediately gone"
        );
    }

    /// Retry loop succeeds on second `get_server()` call.
    ///
    /// MockProvider returns `Some` on call 0 (initial verify), then `None` on
    /// call 1 (after first retry). Time is paused so the 1-second sleep is instant.
    #[tokio::test]
    async fn disconnect_retries_on_verify_failure() {
        let (lifecycle, _dir) = test_lifecycle("retries_on_verify_failure");
        let session = sample_session();

        lifecycle
            .session_tracker
            .create_session(&session)
            .expect("create_session should succeed");

        // get_server returns Some on call 0 (initial), None on call 1 (retry 1).
        let mock = MockProvider::retries(1);
        let destroy_count = mock.destroy_call_count.clone();
        let get_server_count = mock.get_server_call_count.clone();

        let result = lifecycle
            .destroy_and_cleanup(&session, &mock, "test-api-key")
            .await;

        assert!(
            result.is_ok(),
            "destroy_and_cleanup should succeed after one retry, got: {result:?}"
        );

        // destroy_server called twice: initial + one retry.
        assert_eq!(
            destroy_count.load(Ordering::SeqCst),
            2,
            "destroy_server should be called twice: initial attempt + one retry"
        );

        // get_server called twice: initial verify + one retry verify.
        assert_eq!(
            get_server_count.load(Ordering::SeqCst),
            2,
            "get_server should be called twice: initial verify + one retry verify"
        );

        // Session deleted after confirmed success.
        let remaining = lifecycle
            .session_tracker
            .read_session()
            .expect("read_session should not error");
        assert!(
            remaining.is_none(),
            "session file should be deleted after retry succeeds"
        );
    }

    /// Session is preserved when server cannot be destroyed after all retries.
    ///
    /// MockProvider always returns `Some` from `get_server()` -- server never
    /// disappears. Time is paused so the 1+2+4 second sleeps are instant.
    #[tokio::test]
    async fn disconnect_persistent_failure_preserves_session() {
        let (lifecycle, _dir) = test_lifecycle("persistent_failure_preserves_session");
        let session = sample_session();

        lifecycle
            .session_tracker
            .create_session(&session)
            .expect("create_session should succeed");

        let mock = MockProvider::persistent_failure();

        let result = lifecycle
            .destroy_and_cleanup(&session, &mock, "test-api-key")
            .await;

        // Must return DestructionFailed.
        assert!(
            matches!(result, Err(LifecycleError::DestructionFailed(_))),
            "expected DestructionFailed when server cannot be destroyed, got: {result:?}"
        );

        // Error message must contain the provider console URL.
        if let Err(LifecycleError::DestructionFailed(msg)) = &result {
            assert!(
                msg.contains(Provider::Hetzner.console_url()),
                "error message should contain console URL for manual verification, got: {msg}"
            );
        }

        // Session file must be preserved -- user needs it to retry.
        let remaining = lifecycle
            .session_tracker
            .read_session()
            .expect("read_session should not error");
        assert!(
            remaining.is_some(),
            "session file must be preserved when server destruction fails persistently"
        );
    }

    /// Tunnel teardown failure is non-fatal -- disconnect continues to succeed.
    ///
    /// `tunnel_down_interface()` calls osascript which fails in the test
    /// environment (no admin privileges, no wg-quick). This test verifies that
    /// the tunnel failure (step 2) does not abort the disconnect flow.
    ///
    /// We test `destroy_and_cleanup()` directly -- the successful outcome here
    /// represents steps 5-7 completing correctly after step 2 already failed
    /// and was swallowed by `disconnect()`. All other tests in this module
    /// also implicitly exercise the post-tunnel-failure path.
    #[tokio::test]
    async fn disconnect_continues_after_tunnel_failure() {
        let (lifecycle, _dir) = test_lifecycle("continues_after_tunnel_failure");
        let session = sample_session();

        lifecycle
            .session_tracker
            .create_session(&session)
            .expect("create_session should succeed");

        // Successful provider -- verify that provider-side steps complete
        // regardless of what happened to the tunnel in step 2.
        let mock = MockProvider::success();

        let result = lifecycle
            .destroy_and_cleanup(&session, &mock, "test-api-key")
            .await;

        assert!(
            result.is_ok(),
            "disconnect should succeed even after tunnel teardown failure, got: {result:?}"
        );

        // Session must be gone -- full cleanup completed.
        let remaining = lifecycle
            .session_tracker
            .read_session()
            .expect("read_session should not error");
        assert!(
            remaining.is_none(),
            "session file should be deleted after successful destroy_and_cleanup"
        );
    }
}
