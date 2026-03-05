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
#[allow(unused)]
mod vpn_manager;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, PhysicalPosition, Position,
};

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

            let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit_item])?;

            let _tray = TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app: &tauri::AppHandle, event| {
                    if event.id().as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray: &tauri::tray::TrayIcon, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(webview_window) = app.get_webview_window("main") {
                            if let Ok(visible) = webview_window.is_visible() {
                                if visible {
                                    let _ = webview_window.hide();
                                } else {
                                    let (pos_x, pos_y) = match rect.position {
                                        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
                                        tauri::Position::Logical(p) => (p.x, p.y),
                                    };
                                    let (size_w, size_h) = match rect.size {
                                        tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
                                        tauri::Size::Logical(s) => (s.width, s.height),
                                    };
                                    let x = (pos_x + size_w / 2.0 - 160.0) as i32;
                                    let y = (pos_y + size_h) as i32;
                                    let _ = webview_window.set_position(
                                        Position::Physical(PhysicalPosition::new(x, y)),
                                    );
                                    let _ = webview_window.unminimize();
                                    let _ = webview_window.show();
                                    let _ = webview_window.set_focus();
                                }
                            }
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Focused(false) = event {
                let _ = window.hide();
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
