//! IPC commands for provider management (IPC-PM).
//!
//! Covers credential registration, removal, listing, and region enumeration
//! for the three supported cloud providers.

use crate::error::AppError;
use crate::types::Provider;

/// Register a cloud provider with its API key and an optional account label.
///
/// Returns a `ProviderInfo` JSON object on success.
#[tauri::command]
pub async fn register_provider(
    provider: Provider,
    api_key: String,
    account_label: String,
) -> Result<serde_json::Value, AppError> {
    let _ = (provider, api_key, account_label);
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "register_provider is not yet implemented",
        None,
    ))
}

/// Remove a previously registered cloud provider and delete its stored credential.
#[tauri::command]
pub async fn remove_provider(provider: Provider) -> Result<(), AppError> {
    let _ = provider;
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "remove_provider is not yet implemented",
        None,
    ))
}

/// List all registered providers and their current credential status.
///
/// Returns a `Vec<ProviderInfo>` JSON array.
#[tauri::command]
pub async fn list_providers() -> Result<Vec<serde_json::Value>, AppError> {
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "list_providers is not yet implemented",
        None,
    ))
}

/// List available regions for a given provider, including pricing information.
///
/// Returns a `Vec<RegionInfo>` JSON array.
#[tauri::command]
pub async fn list_regions(provider: Provider) -> Result<Vec<serde_json::Value>, AppError> {
    let _ = provider;
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "list_regions is not yet implemented",
        None,
    ))
}
