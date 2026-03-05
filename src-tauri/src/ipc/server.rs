//! IPC commands for server lifecycle management (IPC-SL).
//!
//! Covers VPN tunnel connect/disconnect and orphaned server detection
//! and resolution.

use tokio::sync::Mutex;

use crate::error::{codes, AppError};
use crate::provider_manager::ProviderRegistry;
use crate::server_lifecycle::ServerLifecycle;
use crate::session_tracker::SessionStatus;
use crate::types::{OrphanAction, OrphanedServer, Provider};

/// Provision a VPN server on the given provider and region, then bring up the
/// WireGuard tunnel.
///
/// Returns a `SessionStatus` JSON object on success.
#[tauri::command]
pub async fn connect(
    lifecycle: tauri::State<'_, ServerLifecycle>,
    registry: tauri::State<'_, Mutex<ProviderRegistry>>,
    provider: Provider,
    region: String,
) -> Result<SessionStatus, AppError> {
    // Input validation: verify provider is registered in registry.
    {
        let reg = registry.lock().await;
        if reg.get(&provider).is_none() {
            return Err(AppError::new(
                codes::NOT_FOUND_PROVIDER,
                format!("Provider {provider} is not registered"),
                None,
            ));
        }
    }

    // Check for active session before proceeding.
    if lifecycle.session_tracker.read_session()
        .map_err(|e| AppError::new(codes::INTERNAL_UNEXPECTED, e.to_string(), None))?
        .is_some()
    {
        return Err(AppError::new(
            codes::CONFLICT_SESSION_ACTIVE,
            "An active session already exists",
            None,
        ));
    }

    // Delegate to ServerLifecycle::connect().
    let status = lifecycle.connect(provider, &region, registry.inner()).await?;
    Ok(status)
}

/// Tear down the active WireGuard tunnel and destroy the remote server.
#[tauri::command]
pub async fn disconnect(
    lifecycle: tauri::State<'_, ServerLifecycle>,
    registry: tauri::State<'_, Mutex<ProviderRegistry>>,
) -> Result<(), AppError> {
    let status = lifecycle.disconnect(registry.inner()).await?;
    Ok(status)
}

/// Scan all registered providers for servers that were provisioned by this app
/// but are no longer tracked locally (orphaned after a crash or forced quit).
///
/// Returns a `Vec<OrphanedServer>` JSON array.
#[tauri::command]
pub async fn check_orphaned_servers(
    lifecycle: tauri::State<'_, ServerLifecycle>,
    registry: tauri::State<'_, Mutex<ProviderRegistry>>,
) -> Result<Vec<OrphanedServer>, AppError> {
    let orphans = lifecycle.check_orphaned_servers(registry.inner()).await?;
    Ok(orphans)
}

/// Resolve a single orphaned server by either destroying it or reconnecting to
/// it.
///
/// `action` must be `"destroy"` or `"reconnect"`.
/// Returns an `Option<SessionStatus>` JSON value:
/// - `SessionStatus` when action is `"reconnect"`
/// - `null` when action is `"destroy"`
#[tauri::command]
pub async fn resolve_orphaned_server(
    lifecycle: tauri::State<'_, ServerLifecycle>,
    registry: tauri::State<'_, Mutex<ProviderRegistry>>,
    server_id: String,
    action: OrphanAction,
) -> Result<Option<SessionStatus>, AppError> {
    let result = lifecycle
        .resolve_orphaned_server(&server_id, action, registry.inner())
        .await?;
    Ok(result)
}
