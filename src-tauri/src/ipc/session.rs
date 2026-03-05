//! IPC commands for session status tracking (IPC-SS).
//!
//! Exposes the current active VPN session, if any, to the frontend.

use crate::error::AppError;
use crate::server_lifecycle::ServerLifecycle;
use crate::session_tracker::SessionStatus;

/// Return the current active session status, or `null` if no session is active.
///
/// Returns an `Option<SessionStatus>` JSON value.
#[tauri::command]
pub async fn get_session_status(
    lifecycle: tauri::State<'_, ServerLifecycle>,
) -> Result<Option<SessionStatus>, AppError> {
    let status = lifecycle.session_tracker.get_status()?;
    Ok(status)
}
