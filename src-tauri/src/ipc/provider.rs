//! IPC commands for provider management (IPC-PM).
//!
//! Covers credential registration, removal, listing, and region enumeration
//! for the three supported cloud providers.

use tokio::sync::Mutex;

use crate::error::{codes, AppError};
use crate::keychain_adapter::KeychainAdapter;
use crate::provider_manager::ProviderRegistry;
use crate::types::{Provider, ProviderInfo, ProviderStatus, RegionInfo};

/// Register a cloud provider with its API key and an account label.
///
/// Validates the credential against the provider API, stores it in the
/// macOS Keychain, and invalidates any cached pricing data.
///
/// Returns a `ProviderInfo` on success.
#[tauri::command]
pub async fn register_provider(
    state: tauri::State<'_, Mutex<ProviderRegistry>>,
    provider: Provider,
    api_key: String,
    account_label: String,
) -> Result<ProviderInfo, AppError> {
    // Input validation at IPC boundary.
    if api_key.trim().is_empty() {
        return Err(AppError::new(
            codes::VALIDATION_EMPTY_API_KEY,
            "API key must not be empty",
            None,
        ));
    }
    if account_label.trim().is_empty() {
        return Err(AppError::new(
            codes::VALIDATION_EMPTY_ACCOUNT_LABEL,
            "Account label must not be empty",
            None,
        ));
    }

    // Validate credential against the cloud provider API.
    {
        let registry = state.lock().await;
        let cloud_provider = registry.get(&provider).ok_or_else(|| {
            AppError::new(
                codes::NOT_FOUND_PROVIDER,
                format!("Provider {provider} is not registered in the registry"),
                None,
            )
        })?;
        cloud_provider.validate_credential(&api_key).await?;
    }

    // Store credential in macOS Keychain.
    KeychainAdapter::store_credential(&provider, &account_label, &api_key)?;

    // Invalidate cached pricing data for this provider.
    {
        let mut registry = state.lock().await;
        registry.cache_mut().invalidate(&provider);
    }

    Ok(ProviderInfo {
        provider,
        status: ProviderStatus::Valid,
        account_label,
    })
}

/// Remove a previously registered cloud provider and delete its stored credential.
///
/// TODO: Check for active sessions using this provider (M4.1 SessionTracker).
/// When SessionTracker is implemented, return `CONFLICT_PROVIDER_IN_USE` if
/// there is an active session using this provider.
#[tauri::command]
pub async fn remove_provider(
    state: tauri::State<'_, Mutex<ProviderRegistry>>,
    provider: Provider,
) -> Result<(), AppError> {
    // TODO (M4): Check active session -- return CONFLICT_PROVIDER_IN_USE if active.

    // Delete credential from macOS Keychain.
    KeychainAdapter::delete_credential(&provider)?;

    // Invalidate cached pricing data.
    {
        let mut registry = state.lock().await;
        registry.cache_mut().invalidate(&provider);
    }

    Ok(())
}

/// List all registered providers and their current credential status.
///
/// Queries the macOS Keychain for stored credentials and returns a
/// `Vec<ProviderInfo>`. All returned providers have status `Valid`
/// because credentials are validated at registration time.
#[tauri::command]
pub async fn list_providers() -> Result<Vec<ProviderInfo>, AppError> {
    let credentials = KeychainAdapter::list_credentials()?;

    let providers = credentials
        .into_iter()
        .map(|(provider, account_label)| ProviderInfo {
            provider,
            status: ProviderStatus::Valid,
            account_label,
        })
        .collect();

    Ok(providers)
}

/// List available regions for a given provider, including pricing information.
///
/// Uses cached data when available. On cache miss, fetches from the provider
/// API and caches the result. If the API call fails but stale cached data
/// exists, returns the stale data as a fallback.
#[tauri::command]
pub async fn list_regions(
    state: tauri::State<'_, Mutex<ProviderRegistry>>,
    provider: Provider,
) -> Result<Vec<RegionInfo>, AppError> {
    // Check cache first (under lock, but clone immediately to release).
    {
        let registry = state.lock().await;
        if let Some(cached) = registry.cache().get(&provider) {
            return Ok(cached.to_vec());
        }
    }

    // Cache miss -- fetch from provider API.
    let credential = KeychainAdapter::retrieve_credential(&provider)?;
    let credential = credential.ok_or_else(|| {
        AppError::new(
            codes::NOT_FOUND_PROVIDER,
            format!("No credential found for provider {provider}"),
            None,
        )
    })?;

    let api_result = {
        let registry = state.lock().await;
        let cloud_provider = registry.get(&provider).ok_or_else(|| {
            AppError::new(
                codes::NOT_FOUND_PROVIDER,
                format!("Provider {provider} is not registered in the registry"),
                None,
            )
        })?;
        cloud_provider.list_regions(&credential.api_key).await
    };

    match api_result {
        Ok(mut regions) => {
            // Sort by hourly cost ascending.
            regions.sort_by(|a, b| a.hourly_cost.partial_cmp(&b.hourly_cost).unwrap_or(std::cmp::Ordering::Equal));

            // Cache the result.
            let mut registry = state.lock().await;
            registry.cache_mut().set(provider, regions.clone());

            Ok(regions)
        }
        Err(api_error) => {
            // Stale fallback: return expired cache data if available.
            let registry = state.lock().await;
            if let Some(stale) = registry.cache().get_stale(&provider) {
                return Ok(stale.to_vec());
            }
            // No stale cache -- propagate the API error.
            Err(AppError::from(api_error))
        }
    }
}
