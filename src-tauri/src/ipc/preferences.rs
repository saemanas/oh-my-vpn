//! IPC commands for user preferences management (IPC-PS).
//!
//! Covers reading and updating persistent user preferences stored via the
//! preferences store.

use crate::error::AppError;
use crate::preferences_store::{PartialUserPreferences, UserPreferencesResponse};
use crate::server_lifecycle::ServerLifecycle;

/// Return the current user preferences.
///
/// Returns a `UserPreferencesResponse` JSON object (excludes schema_version).
#[tauri::command]
pub async fn get_preferences(
    lifecycle: tauri::State<'_, ServerLifecycle>,
) -> Result<UserPreferencesResponse, AppError> {
    let preferences = lifecycle.preferences_store.load()?;
    Ok(preferences.to_response())
}

/// Merge the given partial preferences object with the stored preferences and
/// persist the result.
///
/// `partial` is a `PartialUserPreferences` JSON object (all fields optional).
/// Returns the full merged `UserPreferencesResponse` JSON object.
#[tauri::command]
pub async fn update_preferences(
    lifecycle: tauri::State<'_, ServerLifecycle>,
    preferences: PartialUserPreferences,
) -> Result<UserPreferencesResponse, AppError> {
    let mut current = lifecycle.preferences_store.load()?;
    current.merge(preferences);
    lifecycle.preferences_store.save(&current)?;
    Ok(current.to_response())
}
