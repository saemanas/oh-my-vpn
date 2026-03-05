//! Application-level IPC commands.

use std::sync::atomic::Ordering;

use tauri::AppHandle;

use crate::tray::QUIT_PENDING;

/// Exit the application gracefully.
///
/// Called from the frontend after a quit-while-connected confirmation
/// dialog completes the disconnect flow.
#[tauri::command]
pub fn quit_app(app: AppHandle) {
    QUIT_PENDING.store(false, Ordering::Relaxed);
    app.exit(0);
}

/// Cancel a pending quit confirmation.
///
/// Called from the frontend when the user dismisses the quit-while-connected
/// dialog. Clears the `QUIT_PENDING` flag so hide-on-blur resumes.
#[tauri::command]
pub fn cancel_quit() {
    QUIT_PENDING.store(false, Ordering::Relaxed);
}
