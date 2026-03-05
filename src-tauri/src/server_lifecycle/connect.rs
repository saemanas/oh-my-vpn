//! Connect flow orchestration -- 11-step server provisioning and tunnel setup.
//!
//! Implements the full connect lifecycle: verify no active session, retrieve
//! API key, generate SSH + WG keys, provision server with cloud-init, establish
//! WireGuard tunnel, and persist session state. Auto-cleanup on any failure.

use chrono::Utc;
use serde::Serialize;
use tauri::Emitter;
use zeroize::Zeroize;

use crate::keychain_adapter::KeychainAdapter;
use crate::provider_manager::{CloudProvider, ProviderRegistry};
use crate::session_tracker::{ActiveSession, SessionStatus};
use crate::tray::{update_tray_icon, VpnState};
use crate::types::Provider;
use crate::vpn_manager::keys::WireGuardKeyPair;
use crate::vpn_manager::tunnel;

use super::cloud_init::build_cloud_init;
use super::ssh_keys::SshKeyPair;
use super::LifecycleError;
use super::ServerLifecycle;

// ── ConnectProgress ─────────────────────────────────────────────────────────

/// Progress event payload emitted during the connect flow.
///
/// - step 1: Creating server (SSH key generation → server provisioning)
/// - step 2: Installing WireGuard (SSH key cleanup)
/// - step 3: Connecting tunnel (WireGuard tunnel up → session creation)
#[derive(Clone, Serialize)]
pub struct ConnectProgress {
    pub step: u8,
}

// ── ConnectCleanup ──────────────────────────────────────────────────────────

/// Tracks cloud resources created during the connect flow so they can be
/// cleaned up if a later step fails. Implements Drop for automatic cleanup,
/// but `disarm()` must be called on success to prevent cleanup.
struct ConnectCleanup<'a> {
    provider: &'a dyn CloudProvider,
    api_key: String,
    ssh_key_id: Option<String>,
    server_id: Option<String>,
    armed: bool,
}

impl<'a> ConnectCleanup<'a> {
    fn new(provider: &'a dyn CloudProvider, api_key: &str) -> Self {
        Self {
            provider,
            api_key: api_key.to_string(),
            ssh_key_id: None,
            server_id: None,
            armed: true,
        }
    }

    fn track_ssh_key(&mut self, key_id: String) {
        self.ssh_key_id = Some(key_id);
    }

    fn track_server(&mut self, server_id: String) {
        self.server_id = Some(server_id);
    }

    fn clear_ssh_key(&mut self) {
        self.ssh_key_id = None;
    }

    /// Prevent cleanup from running. Call this after all steps succeed.
    fn disarm(mut self) {
        self.armed = false;
        // api_key is zeroed on drop via the Drop impl
    }

    /// Execute compensating actions synchronously (best-effort).
    ///
    /// Called from Drop. We cannot use async in Drop, so we use
    /// `block_in_place` + `block_on` to run cleanup futures.
    /// `block_in_place` moves the blocking call off the async worker thread,
    /// avoiding the "cannot block from within a runtime" panic.
    /// Errors are silently ignored -- the original error is more important.
    fn execute_cleanup(&mut self) {
        if !self.armed {
            return;
        }

        let handle = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(_) => return, // No runtime available -- cannot clean up
        };

        let server_id = self.server_id.take();
        let ssh_key_id = self.ssh_key_id.take();
        let api_key = self.api_key.clone();
        let provider = self.provider as *const dyn CloudProvider;

        // Safety: provider outlives this cleanup struct (lifetime 'a).
        // block_in_place runs synchronously before Drop returns.
        tokio::task::block_in_place(|| {
            let provider = unsafe { &*provider };
            handle.block_on(async {
                if let Some(server_id) = server_id {
                    let _ = provider.destroy_server(&api_key, &server_id).await;
                }
                if let Some(ssh_key_id) = ssh_key_id {
                    let _ = provider.delete_ssh_key(&api_key, &ssh_key_id).await;
                }
            });
        });
    }
}

impl<'a> Drop for ConnectCleanup<'a> {
    fn drop(&mut self) {
        self.execute_cleanup();
        self.api_key.zeroize();
    }
}

// ── ServerLifecycle::connect ────────────────────────────────────────────────

impl ServerLifecycle {
    /// Execute the full 11-step connect flow.
    ///
    /// # Steps
    ///
    /// 1. Verify no active session exists
    /// 2. Retrieve API key from Keychain
    /// 3. Generate SSH key pair (Ed25519)
    /// 4. Register SSH key with cloud provider
    /// 5. Generate WireGuard server + client key pairs
    /// 6. Build cloud-init script
    /// 7. Create server with cloud-init
    /// 8. Delete SSH key from provider (no longer needed)
    /// 9. Zero SSH key material
    /// 10. Bring up WireGuard tunnel
    /// 11. Create session + update preferences
    ///
    /// Auto-cleanup: if any step fails after creating cloud resources, the
    /// `ConnectCleanup` guard destroys them before returning the error.
    pub async fn connect(
        &self,
        provider: Provider,
        region: &str,
        registry: &tokio::sync::Mutex<ProviderRegistry>,
        app: &tauri::AppHandle,
    ) -> Result<SessionStatus, LifecycleError> {
        // Step 1: Verify no active session.
        if self.session_tracker.read_session()
            .map_err(|e| LifecycleError::PersistenceFailed(e.to_string()))?
            .is_some()
        {
            return Err(LifecycleError::SessionActive);
        }

        // Step 2: Retrieve API key from Keychain.
        let credential = KeychainAdapter::retrieve_credential(&provider)
            .map_err(|e| LifecycleError::KeychainFailed(e.to_string()))?
            .ok_or_else(|| LifecycleError::ProviderNotRegistered(
                format!("No credential found for provider: {provider}")
            ))?;

        let api_key = credential.api_key;

        // Lock the registry to get the provider implementation.
        let registry_guard = registry.lock().await;
        let cloud_provider = registry_guard.get(&provider)
            .ok_or_else(|| LifecycleError::ProviderNotRegistered(
                format!("Provider not registered: {provider}")
            ))?;

        // Initialize cleanup guard.
        let mut cleanup = ConnectCleanup::new(cloud_provider, &api_key);

        // Emit progress: step 1 -- Creating server.
        let _ = app.emit("connect-progress", ConnectProgress { step: 1 });
        update_tray_icon(app, VpnState::Connecting);

        // Step 3: Generate SSH key pair.
        let ssh_key_pair = SshKeyPair::generate()?;
        let ssh_public_key = ssh_key_pair.public_key_openssh()?;

        // Step 4: Register SSH key with provider.
        let ssh_key_label = format!("oh-my-vpn-{}", Utc::now().timestamp());
        let ssh_key_id = cloud_provider
            .create_ssh_key(&api_key, &ssh_public_key, &ssh_key_label)
            .await
            .map_err(|e| LifecycleError::SshKeyRegistrationFailed(format!("{e:?}")))?;
        cleanup.track_ssh_key(ssh_key_id.clone());

        // Step 5: Generate WireGuard server + client key pairs.
        let server_wg_key_pair = WireGuardKeyPair::generate();
        let client_wg_key_pair = WireGuardKeyPair::generate();

        // Step 6: Build cloud-init script.
        let cloud_init_script = build_cloud_init(
            &server_wg_key_pair.private_key_base64(),
            &client_wg_key_pair.public_key_base64(),
        );

        // Step 7: Create server.
        let server_info = cloud_provider
            .create_server(&api_key, region, &ssh_key_id, &cloud_init_script)
            .await
            .map_err(|e| LifecycleError::ProvisioningFailed(format!("{e:?}")))?;
        cleanup.track_server(server_info.server_id.clone());

        // Emit progress: step 2 -- Installing WireGuard.
        let _ = app.emit("connect-progress", ConnectProgress { step: 2 });

        // Step 8: Delete SSH key from provider (no longer needed).
        let _ = cloud_provider
            .delete_ssh_key(&api_key, &ssh_key_id)
            .await;
        cleanup.clear_ssh_key();

        // Step 9: Zero SSH key material (happens automatically via Drop/Zeroize,
        // but we drop explicitly here for clarity).
        drop(ssh_key_pair);

        // Emit progress: step 3 -- Connecting tunnel.
        let _ = app.emit("connect-progress", ConnectProgress { step: 3 });

        // Step 10: Bring up WireGuard tunnel.
        tunnel::tunnel_up(
            &client_wg_key_pair,
            &server_info.public_ip,
            &server_wg_key_pair.public_key_base64(),
            "10.0.0.2/32",
            "1.1.1.1, 1.0.0.1",
        )
        .await
        .map_err(|e| LifecycleError::TunnelFailed(format!("{e:?}")))?;
        update_tray_icon(app, VpnState::Connected);

        // Step 11: Create session + update preferences.
        // Look up hourly cost from the provider's region list.
        let (hourly_cost, region_display_name) = match cloud_provider.list_regions(&api_key).await {
            Ok(regions) => regions.iter()
                .find(|r| r.region == region)
                .map(|r| (r.hourly_cost, r.display_name.clone()))
                .unwrap_or((0.0, region.to_string())),
            Err(_) => (0.0, region.to_string()),
        };

        let session = ActiveSession {
            server_id: server_info.server_id.clone(),
            provider: provider.clone(),
            region: region.to_string(),
            region_display_name,
            server_ip: server_info.public_ip.clone(),
            created_at: Utc::now().to_rfc3339(),
            hourly_cost,
            ssh_key_id: None, // SSH key already deleted
            server_wireguard_public_key: Some(server_wg_key_pair.public_key_base64()),
            client_wireguard_private_key: Some(client_wg_key_pair.private_key_base64()),
        };

        self.session_tracker.create_session(&session)
            .map_err(|e| LifecycleError::PersistenceFailed(e.to_string()))?;

        // Update preferences with last used provider/region.
        let mut preferences = self.preferences_store.load()
            .map_err(|e| LifecycleError::PersistenceFailed(e.to_string()))?;
        preferences.last_provider = Some(provider);
        preferences.last_region = Some(region.to_string());
        self.preferences_store.save(&preferences)
            .map_err(|e| LifecycleError::PersistenceFailed(e.to_string()))?;

        // All steps succeeded -- disarm cleanup.
        cleanup.disarm();

        // Return current session status.
        self.session_tracker.get_status()
            .map_err(|e| LifecycleError::PersistenceFailed(e.to_string()))?
            .ok_or_else(|| LifecycleError::PersistenceFailed(
                "Session was created but could not be read back".to_string()
            ))
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ProviderError;
    use crate::provider_manager::CloudProvider;
    use crate::types::{RegionInfo, ServerInfo, ServerStatus};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // ── Mock Provider ───────────────────────────────────────────────────

    struct MockProvider {
        should_fail_create_server: bool,
        destroy_called: Arc<AtomicBool>,
        delete_ssh_key_called: Arc<AtomicBool>,
    }

    impl MockProvider {
        fn new() -> Self {
            Self {
                should_fail_create_server: false,
                destroy_called: Arc::new(AtomicBool::new(false)),
                delete_ssh_key_called: Arc::new(AtomicBool::new(false)),
            }
        }

        fn failing_create_server() -> Self {
            Self {
                should_fail_create_server: true,
                destroy_called: Arc::new(AtomicBool::new(false)),
                delete_ssh_key_called: Arc::new(AtomicBool::new(false)),
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
            if self.should_fail_create_server {
                return Err(ProviderError::ProvisioningFailed(
                    "Mock: create_server failed".to_string(),
                ));
            }
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
            self.destroy_called.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn get_server(
            &self,
            _api_key: &str,
            _server_id: &str,
        ) -> Result<Option<ServerInfo>, ProviderError> {
            Ok(None)
        }
    }

    // ── Cleanup Tests ──────────────────────────────────────────────────

    #[test]
    fn cleanup_destroys_server_on_drop() {
        let provider = MockProvider::new();
        let destroy_called = provider.destroy_called.clone();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut cleanup = ConnectCleanup::new(&provider, "test-key");
            cleanup.track_server("srv-123".to_string());
            // Drop cleanup while armed -- should call destroy_server
            drop(cleanup);
        });

        assert!(
            destroy_called.load(Ordering::SeqCst),
            "destroy_server should be called when cleanup drops with tracked server"
        );
    }

    #[test]
    fn cleanup_deletes_ssh_key_on_drop() {
        let provider = MockProvider::new();
        let delete_called = provider.delete_ssh_key_called.clone();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut cleanup = ConnectCleanup::new(&provider, "test-key");
            cleanup.track_ssh_key("key-456".to_string());
            drop(cleanup);
        });

        assert!(
            delete_called.load(Ordering::SeqCst),
            "delete_ssh_key should be called when cleanup drops with tracked SSH key"
        );
    }

    #[test]
    fn cleanup_does_not_run_when_disarmed() {
        let provider = MockProvider::new();
        let destroy_called = provider.destroy_called.clone();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut cleanup = ConnectCleanup::new(&provider, "test-key");
            cleanup.track_server("srv-123".to_string());
            cleanup.disarm();
        });

        assert!(
            !destroy_called.load(Ordering::SeqCst),
            "destroy_server should NOT be called when cleanup is disarmed"
        );
    }

    #[test]
    fn cleanup_clears_ssh_key_tracking() {
        let provider = MockProvider::new();
        let delete_called = provider.delete_ssh_key_called.clone();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut cleanup = ConnectCleanup::new(&provider, "test-key");
            cleanup.track_ssh_key("key-456".to_string());
            cleanup.clear_ssh_key();
            drop(cleanup);
        });

        assert!(
            !delete_called.load(Ordering::SeqCst),
            "delete_ssh_key should NOT be called after clear_ssh_key"
        );
    }

    // ── Connect Flow Tests (requires Keychain mock -- marked ignored) ──

    /// Verify that cleanup runs when create_server fails after SSH key registration.
    /// This test validates the ConnectCleanup pattern with a failing MockProvider.
    ///
    /// Note: The full connect() flow requires Keychain access and cannot be unit
    /// tested without a Keychain mock. The cleanup guard is tested independently above.
    #[test]
    fn cleanup_on_provisioning_failure_cleans_ssh_key() {
        let provider = MockProvider::failing_create_server();
        let delete_called = provider.delete_ssh_key_called.clone();

        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let mut cleanup = ConnectCleanup::new(&provider, "test-key");

            // Simulate: SSH key registered successfully
            cleanup.track_ssh_key("key-789".to_string());

            // Simulate: create_server fails -- cleanup should delete SSH key on drop
            let _result = provider
                .create_server("test-key", "fsn1", "key-789", "#!/bin/bash")
                .await;
            // cleanup drops here (armed) -- should clean up SSH key
        });

        assert!(
            delete_called.load(Ordering::SeqCst),
            "SSH key should be cleaned up when provisioning fails"
        );
    }
}
