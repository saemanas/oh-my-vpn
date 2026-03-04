//! CloudProvider trait -- the async abstraction implemented by each cloud provider.
//!
//! See API Design §4.F for the full contract specification.

use async_trait::async_trait;

use crate::error::ProviderError;
use crate::types::{RegionInfo, ServerInfo};

/// Async trait implemented by each cloud provider (Hetzner, AWS, GCP).
///
/// All methods receive an `api_key` parameter rather than storing credentials
/// internally -- keys are fetched from the Keychain on each call (ADR-0004).
#[async_trait]
pub trait CloudProvider: Send + Sync {
    /// Validate that the API credential has sufficient permissions.
    /// Returns `Ok(())` on success, `Err` with a specific permission error on failure.
    async fn validate_credential(&self, api_key: &str) -> Result<(), ProviderError>;

    /// List available regions with pricing information.
    /// Returns regions sorted by hourly cost ascending.
    async fn list_regions(&self, api_key: &str) -> Result<Vec<RegionInfo>, ProviderError>;

    /// Register an ephemeral SSH public key with the provider.
    /// Returns the provider-side key ID for later deletion.
    async fn create_ssh_key(
        &self,
        api_key: &str,
        public_key: &str,
        label: &str,
    ) -> Result<String, ProviderError>;

    /// Delete a previously registered SSH key by its provider-side ID.
    async fn delete_ssh_key(
        &self,
        api_key: &str,
        key_id: &str,
    ) -> Result<(), ProviderError>;

    /// Provision a new server with the given cloud-init script and SSH key.
    /// Returns server info (ID, public IP) once the server is running.
    async fn create_server(
        &self,
        api_key: &str,
        region: &str,
        ssh_key_id: &str,
        cloud_init: &str,
    ) -> Result<ServerInfo, ProviderError>;

    /// Destroy a server by its provider-side ID.
    async fn destroy_server(
        &self,
        api_key: &str,
        server_id: &str,
    ) -> Result<(), ProviderError>;

    /// Check if a server still exists. Used for orphan detection and deletion verification.
    async fn get_server(
        &self,
        api_key: &str,
        server_id: &str,
    ) -> Result<Option<ServerInfo>, ProviderError>;
}
