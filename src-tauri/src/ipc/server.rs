//! IPC commands for server lifecycle management (IPC-SL).
//!
//! Covers VPN tunnel connect/disconnect and orphaned server detection
//! and resolution.

use crate::error::AppError;
use crate::types::Provider;

/// Provision a VPN server on the given provider and region, then bring up the
/// WireGuard tunnel.
///
/// Returns a `SessionStatus` JSON object on success.
#[tauri::command]
pub async fn connect(
    provider: Provider,
    region: String,
) -> Result<serde_json::Value, AppError> {
    let _ = (provider, region);
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "connect is not yet implemented",
        None,
    ))
}

/// Tear down the active WireGuard tunnel and destroy the remote server.
#[tauri::command]
pub async fn disconnect() -> Result<(), AppError> {
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "disconnect is not yet implemented",
        None,
    ))
}

/// Scan all registered providers for servers that were provisioned by this app
/// but are no longer tracked locally (orphaned after a crash or forced quit).
///
/// Returns a `Vec<OrphanedServer>` JSON array.
#[tauri::command]
pub async fn check_orphaned_servers() -> Result<Vec<serde_json::Value>, AppError> {
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "check_orphaned_servers is not yet implemented",
        None,
    ))
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
    server_id: String,
    action: String,
) -> Result<Option<serde_json::Value>, AppError> {
    let _ = (server_id, action);
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "resolve_orphaned_server is not yet implemented",
        None,
    ))
}
