#[allow(unused)]
mod error;
#[allow(unused)]
mod ipc;
pub mod types;
#[allow(unused)]
pub(crate) mod keychain_adapter;
#[allow(unused)]
mod preferences_store;
#[allow(unused)]
mod provider_manager;
#[allow(unused)]
mod server_lifecycle;
#[allow(unused)]
mod session_tracker;
#[allow(unused)]
mod vpn_manager;

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
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
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(webview_window) = app.get_webview_window("main") {
                            let _ = webview_window.unminimize();
                            let _ = webview_window.show();
                            let _ = webview_window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
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
