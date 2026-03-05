mod error;
mod ipc;
pub mod types;
pub(crate) mod keychain_adapter;
#[allow(unused)]
mod preferences_store;
mod provider_manager;
mod server_lifecycle;
#[allow(unused)]
mod session_tracker;
pub mod tray;
#[allow(unused)]
mod vpn_manager;

use tauri::{Emitter, Manager, RunEvent};

use provider_manager::{AwsProvider, GcpProvider, HetznerProvider, ProviderRegistry};
use server_lifecycle::ServerLifecycle;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Initialize ProviderRegistry with all supported providers.
            let mut registry = ProviderRegistry::new();
            registry.register(types::Provider::Hetzner, Box::new(HetznerProvider::new()));
            registry.register(types::Provider::Aws, Box::new(AwsProvider::new()));
            registry.register(types::Provider::Gcp, Box::new(GcpProvider::new()));
            app.manage(tokio::sync::Mutex::new(registry));

            // Initialize ServerLifecycle with app data directory.
            let data_dir = app.path().app_data_dir()
                .expect("app data directory should be resolvable");
            app.manage(ServerLifecycle::new(data_dir));

            // Set up system tray icon and event handlers.
            tray::setup_tray(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(false) = event {
                if !tray::QUIT_PENDING.load(std::sync::atomic::Ordering::Relaxed) {
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            ipc::provider::register_provider,
            ipc::provider::remove_provider,
            ipc::provider::list_providers,
            ipc::provider::list_regions,
            ipc::server::connect,
            ipc::server::disconnect,
            ipc::server::check_orphaned_servers,
            ipc::server::resolve_orphaned_server,
            ipc::session::get_session_status,
            ipc::preferences::get_preferences,
            ipc::preferences::update_preferences,
            ipc::app::quit_app,
            ipc::app::cancel_quit,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let RunEvent::ExitRequested { api, .. } = &event {
                let lifecycle = app.state::<ServerLifecycle>();
                let has_session = lifecycle
                    .session_tracker
                    .read_session()
                    .ok()
                    .flatten()
                    .is_some();

                if has_session {
                    api.prevent_exit();
                    tray::QUIT_PENDING.store(true, std::sync::atomic::Ordering::Relaxed);

                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                    let _ = app.emit("quit-requested", ());
                }
            }
        });
}
