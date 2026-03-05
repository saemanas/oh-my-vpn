//! Server provisioning and destruction lifecycle.
//!
//! Coordinates the full server lifecycle: create VM, configure
//! WireGuard, establish tunnel, and destroy on disconnect.
//! Implements the stepper flow (provision → configure → connect).

pub mod cleanup;
pub mod cloud_init;
pub mod connect;
pub mod disconnect;
pub mod orphan;
pub mod ssh_keys;

use std::fmt;
use std::path::PathBuf;

use crate::preferences_store::PreferencesStore;
use crate::session_tracker::SessionTracker;

// -- LifecycleError

/// Internal error type produced by server lifecycle operations.
///
/// This enum is **not** serialized directly. It is converted into an
/// `AppError` via `From<LifecycleError> for AppError` at the IPC boundary.
#[derive(Debug)]
pub enum LifecycleError {
    /// An active session already exists -- cannot connect.
    SessionActive,
    /// The requested provider is not registered in the ProviderRegistry.
    ProviderNotRegistered(String),
    /// Failed to retrieve the API key from the Keychain.
    KeychainFailed(String),
    /// SSH key generation failed.
    SshKeyGenerationFailed(String),
    /// Registering the SSH key with the cloud provider failed.
    SshKeyRegistrationFailed(String),
    /// Server provisioning failed.
    ProvisioningFailed(String),
    /// WireGuard tunnel setup failed.
    TunnelFailed(String),
    /// Session tracking or preferences persistence failed.
    PersistenceFailed(String),
    /// A cloud provider API call failed during lifecycle.
    Provider(crate::error::ProviderError),
    /// Disconnect was called but no active session exists.
    NoActiveSession,
    /// Server destruction failed persistently after all retry attempts.
    DestructionFailed(String),
    /// Orphan detection failed (e.g., provider API unreachable during scan).
    OrphanDetectionFailed(String),
    /// Reconnecting to an orphaned server failed.
    OrphanReconnectFailed(String),
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LifecycleError::SessionActive => write!(f, "An active session already exists"),
            LifecycleError::ProviderNotRegistered(p) => {
                write!(f, "Provider not registered: {p}")
            }
            LifecycleError::KeychainFailed(msg) => {
                write!(f, "Keychain retrieval failed: {msg}")
            }
            LifecycleError::SshKeyGenerationFailed(msg) => {
                write!(f, "SSH key generation failed: {msg}")
            }
            LifecycleError::SshKeyRegistrationFailed(msg) => {
                write!(f, "SSH key registration failed: {msg}")
            }
            LifecycleError::ProvisioningFailed(msg) => {
                write!(f, "Server provisioning failed: {msg}")
            }
            LifecycleError::TunnelFailed(msg) => {
                write!(f, "Tunnel setup failed: {msg}")
            }
            LifecycleError::PersistenceFailed(msg) => {
                write!(f, "Persistence failed: {msg}")
            }
            LifecycleError::Provider(err) => {
                write!(f, "Provider error: {err:?}")
            }
            LifecycleError::NoActiveSession => write!(f, "No active session exists"),
            LifecycleError::DestructionFailed(msg) => {
                write!(f, "Server destruction failed: {msg}")
            }
            LifecycleError::OrphanDetectionFailed(msg) => {
                write!(f, "Orphan detection failed: {msg}")
            }
            LifecycleError::OrphanReconnectFailed(msg) => {
                write!(f, "Orphan reconnect failed: {msg}")
            }
        }
    }
}

impl std::error::Error for LifecycleError {}

impl From<crate::error::ProviderError> for LifecycleError {
    fn from(error: crate::error::ProviderError) -> Self {
        LifecycleError::Provider(error)
    }
}

// -- ServerLifecycle

/// Orchestrates the full connect/disconnect lifecycle.
///
/// Holds references to `SessionTracker` and `PreferencesStore` for state
/// persistence. Provider access is passed per-call via `ProviderRegistry`.
pub struct ServerLifecycle {
    pub session_tracker: SessionTracker,
    pub preferences_store: PreferencesStore,
}

impl ServerLifecycle {
    /// Create a new ServerLifecycle with the given data directory.
    ///
    /// Both `SessionTracker` and `PreferencesStore` share the same `data_dir`.
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            session_tracker: SessionTracker::new(data_dir.clone()),
            preferences_store: PreferencesStore::new(data_dir),
        }
    }
}
