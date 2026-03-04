//! IPC commands for session status tracking (IPC-SS).
//!
//! Exposes the current active VPN session, if any, to the frontend.

use crate::error::AppError;

/// Return the current active session status, or `null` if no session is active.
///
/// Returns an `Option<SessionStatus>` JSON value.
#[tauri::command]
pub async fn get_session_status() -> Result<Option<serde_json::Value>, AppError> {
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "get_session_status is not yet implemented",
        None,
    ))
}
