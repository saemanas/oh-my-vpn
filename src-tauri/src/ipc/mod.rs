//! Tauri IPC command handlers.
//!
//! Defines the whitelisted command interface between the TypeScript
//! frontend and the Rust backend. Each command maps to a domain
//! module operation.

pub mod app;
pub mod preferences;
pub mod provider;
pub mod server;
pub mod session;

pub use app::{cancel_quit, quit_app};
pub use preferences::{get_preferences, update_preferences};
pub use provider::{list_providers, list_regions, register_provider, remove_provider};
pub use server::{check_orphaned_servers, connect, disconnect, resolve_orphaned_server};
pub use session::get_session_status;
