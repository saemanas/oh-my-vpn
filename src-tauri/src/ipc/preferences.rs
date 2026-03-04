//! IPC commands for user preferences management (IPC-PS).
//!
//! Covers reading and updating persistent user preferences stored via the
//! preferences store.

use crate::error::AppError;

/// Return the current user preferences.
///
/// Returns a `UserPreferences` JSON object.
#[tauri::command]
pub async fn get_preferences() -> Result<serde_json::Value, AppError> {
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "get_preferences is not yet implemented",
        None,
    ))
}

/// Merge the given partial preferences object with the stored preferences and
/// persist the result.
///
/// `preferences` is a `PartialUserPreferences` JSON object (all fields optional).
/// Returns the full merged `UserPreferences` JSON object.
#[tauri::command]
pub async fn update_preferences(
    preferences: serde_json::Value,
) -> Result<serde_json::Value, AppError> {
    let _ = preferences;
    Err(AppError::new(
        "NOT_IMPLEMENTED",
        "update_preferences is not yet implemented",
        None,
    ))
}
