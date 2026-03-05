//! Standalone cleanup utilities for post-crash resource recovery.
//!
//! Provides best-effort cleanup for cloud resources that may have been
//! orphaned by a crash or force-quit during the connect flow.

use crate::provider_manager::CloudProvider;

/// Delete an SSH key from the cloud provider (best-effort).
///
/// Called during orphan resolution when the session file indicates an SSH key
/// was registered but never deleted (crash between SSH key registration and
/// deletion in the connect flow).
///
/// Errors are logged but not propagated -- the SSH key is a secondary resource
/// and its cleanup should not block server destruction or reconnection.
pub async fn cleanup_ssh_key(
    provider: &dyn CloudProvider,
    api_key: &str,
    ssh_key_id: &str,
) {
    if let Err(e) = provider.delete_ssh_key(api_key, ssh_key_id).await {
        eprintln!(
            "[warn] Failed to clean up orphaned SSH key {ssh_key_id}: {e:?}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ProviderError;
    use crate::types::{RegionInfo, ServerInfo, ServerStatus};
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct MockProvider {
        delete_ssh_key_called: Arc<AtomicBool>,
        should_fail: bool,
    }

    impl MockProvider {
        fn success() -> Self {
            Self {
                delete_ssh_key_called: Arc::new(AtomicBool::new(false)),
                should_fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                delete_ssh_key_called: Arc::new(AtomicBool::new(false)),
                should_fail: true,
            }
        }
    }

    #[async_trait]
    impl CloudProvider for MockProvider {
        async fn validate_credential(&self, _api_key: &str) -> Result<(), ProviderError> {
            Ok(())
        }

        async fn list_regions(&self, _api_key: &str) -> Result<Vec<RegionInfo>, ProviderError> {
            Ok(vec![])
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
            if self.should_fail {
                Err(ProviderError::NotFound("SSH key not found".to_string()))
            } else {
                Ok(())
            }
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

    #[tokio::test]
    async fn cleanup_ssh_key_calls_delete() {
        let mock = MockProvider::success();
        let called = mock.delete_ssh_key_called.clone();

        cleanup_ssh_key(&mock, "api-key", "ssh-key-123").await;

        assert!(
            called.load(Ordering::SeqCst),
            "delete_ssh_key should be called"
        );
    }

    #[tokio::test]
    async fn cleanup_ssh_key_swallows_error() {
        let mock = MockProvider::failing();
        let called = mock.delete_ssh_key_called.clone();

        // Should not panic or return error -- best-effort.
        cleanup_ssh_key(&mock, "api-key", "ssh-key-456").await;

        assert!(
            called.load(Ordering::SeqCst),
            "delete_ssh_key should be called even when it fails"
        );
    }
}
