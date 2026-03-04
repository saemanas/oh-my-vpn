//! Unified error type for the Oh My VPN backend.
//!
//! All Tauri IPC commands return `Result<T, AppError>`. Domain-specific errors
//! (KeychainError, PreferencesError, ProviderError) convert into AppError via
//! From trait implementations at the IPC boundary.

use serde::Serialize;

use crate::keychain_adapter::KeychainError;
use crate::preferences_store::PreferencesError;

// ── AppError ────────────────────────────────────────────────────────────────

/// Serializable error type returned by every Tauri IPC command.
///
/// The `code` field identifies the error category (see API Design §6.C).
/// The `message` field carries a human-readable description.
/// The `details` field carries optional structured data (e.g., retry_after).
#[derive(Debug, Serialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl AppError {
    /// Construct an AppError from its three constituent parts.
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Option<serde_json::Value>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details,
        }
    }
}

// ── ProviderError ────────────────────────────────────────────────────────────

/// Internal error type produced by cloud-provider operations.
///
/// This enum is **not** serialized directly. It is converted into an
/// `AppError` via `From<ProviderError> for AppError` at the IPC boundary.
#[derive(Debug)]
pub enum ProviderError {
    /// The cloud provider API rejected the API key.
    AuthInvalidKey(String),
    /// The API key is valid but lacks required permissions.
    AuthInsufficientPermissions(String),
    /// The cloud provider rate-limited the request.
    RateLimited { retry_after_seconds: u64 },
    /// The cloud provider returned a 5xx server error.
    ServerError(String),
    /// The cloud provider request timed out.
    Timeout,
    /// The requested cloud resource was not found.
    NotFound(String),
    /// Server provisioning failed before reaching the running state.
    ProvisioningFailed(String),
    /// Server destruction failed after all retry attempts.
    DestructionFailed(String),
    /// Catch-all for unexpected provider errors.
    Other(anyhow::Error),
}

// ── From<KeychainError> for AppError ────────────────────────────────────────

impl From<KeychainError> for AppError {
    fn from(error: KeychainError) -> Self {
        match error {
            KeychainError::AccessDenied(msg) => AppError::new(
                "KEYCHAIN_ACCESS_DENIED",
                msg,
                None,
            ),
            KeychainError::WriteFailed(msg) => AppError::new(
                "KEYCHAIN_WRITE_FAILED",
                msg,
                None,
            ),
            KeychainError::NotFound(msg) => AppError::new(
                "NOT_FOUND_PROVIDER",
                msg,
                None,
            ),
            KeychainError::SearchFailed(msg) => AppError::new(
                "INTERNAL_UNEXPECTED",
                msg,
                None,
            ),
        }
    }
}

// ── From<PreferencesError> for AppError ─────────────────────────────────────

impl From<PreferencesError> for AppError {
    fn from(error: PreferencesError) -> Self {
        let message = error.to_string();
        AppError::new("INTERNAL_UNEXPECTED", message, None)
    }
}

// ── From<ProviderError> for AppError ────────────────────────────────────────

impl From<ProviderError> for AppError {
    fn from(error: ProviderError) -> Self {
        match error {
            ProviderError::AuthInvalidKey(msg) => AppError::new(
                "AUTH_INVALID_KEY",
                msg,
                None,
            ),
            ProviderError::AuthInsufficientPermissions(msg) => AppError::new(
                "AUTH_INSUFFICIENT_PERMISSIONS",
                msg,
                None,
            ),
            ProviderError::RateLimited { retry_after_seconds } => AppError::new(
                "PROVIDER_RATE_LIMITED",
                "Cloud API rate limited -- retrying",
                Some(serde_json::json!({ "retry_after_seconds": retry_after_seconds })),
            ),
            ProviderError::ServerError(msg) => AppError::new(
                "PROVIDER_SERVER_ERROR",
                msg,
                None,
            ),
            ProviderError::Timeout => AppError::new(
                "PROVIDER_TIMEOUT",
                "Cloud provider timeout -- retrying",
                None,
            ),
            ProviderError::NotFound(msg) => AppError::new(
                "NOT_FOUND_PROVIDER",
                msg,
                None,
            ),
            ProviderError::ProvisioningFailed(msg) => AppError::new(
                "PROVIDER_PROVISIONING_FAILED",
                msg,
                None,
            ),
            ProviderError::DestructionFailed(msg) => AppError::new(
                "PROVIDER_DESTRUCTION_FAILED",
                msg,
                None,
            ),
            ProviderError::Other(err) => AppError::new(
                "INTERNAL_UNEXPECTED",
                format!("{err:?}"),
                None,
            ),
        }
    }
}

// ── Error Code Constants ─────────────────────────────────────────────────────

/// All valid AppError codes as string constants (API Design §6.C).
pub mod codes {
    // Validation
    pub const VALIDATION_INVALID_PROVIDER: &str = "VALIDATION_INVALID_PROVIDER";
    pub const VALIDATION_EMPTY_API_KEY: &str = "VALIDATION_EMPTY_API_KEY";
    pub const VALIDATION_EMPTY_ACCOUNT_LABEL: &str = "VALIDATION_EMPTY_ACCOUNT_LABEL";
    pub const VALIDATION_INVALID_REGION: &str = "VALIDATION_INVALID_REGION";

    // Authentication
    pub const AUTH_INVALID_KEY: &str = "AUTH_INVALID_KEY";
    pub const AUTH_INSUFFICIENT_PERMISSIONS: &str = "AUTH_INSUFFICIENT_PERMISSIONS";

    // Not Found
    pub const NOT_FOUND_PROVIDER: &str = "NOT_FOUND_PROVIDER";
    pub const NOT_FOUND_SESSION: &str = "NOT_FOUND_SESSION";

    // Conflict
    pub const CONFLICT_SESSION_ACTIVE: &str = "CONFLICT_SESSION_ACTIVE";
    pub const CONFLICT_PROVIDER_IN_USE: &str = "CONFLICT_PROVIDER_IN_USE";

    // Provider
    pub const PROVIDER_RATE_LIMITED: &str = "PROVIDER_RATE_LIMITED";
    pub const PROVIDER_SERVER_ERROR: &str = "PROVIDER_SERVER_ERROR";
    pub const PROVIDER_TIMEOUT: &str = "PROVIDER_TIMEOUT";
    pub const PROVIDER_PROVISIONING_FAILED: &str = "PROVIDER_PROVISIONING_FAILED";
    pub const PROVIDER_DESTRUCTION_FAILED: &str = "PROVIDER_DESTRUCTION_FAILED";

    // Tunnel
    pub const TUNNEL_SETUP_FAILED: &str = "TUNNEL_SETUP_FAILED";
    pub const TUNNEL_TEARDOWN_FAILED: &str = "TUNNEL_TEARDOWN_FAILED";

    // Keychain
    pub const KEYCHAIN_ACCESS_DENIED: &str = "KEYCHAIN_ACCESS_DENIED";
    pub const KEYCHAIN_WRITE_FAILED: &str = "KEYCHAIN_WRITE_FAILED";

    // Internal
    pub const INTERNAL_UNEXPECTED: &str = "INTERNAL_UNEXPECTED";
}
