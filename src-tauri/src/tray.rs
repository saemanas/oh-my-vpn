use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    App, AppHandle, Manager, PhysicalPosition, Position,
};

/// Global flag to signal the connecting animation task to stop.
static ANIMATION_STOP: AtomicBool = AtomicBool::new(false);

/// VPN connection state for tray icon indication.
#[derive(Debug, Clone, PartialEq)]
pub enum VpnState {
    Disconnected,
    Connecting,
    Connected,
}

/// Build and register the system tray icon with all event handlers.
///
/// Migrates all inline tray setup from lib.rs. Left-click toggles the main
/// popover window; right-click shows the Quit context menu.
pub fn setup_tray(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let quit_item = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&quit_item])?;

    TrayIconBuilder::with_id("main-tray")
        .icon(tauri::include_image!("icons/tray/disconnected.png"))
        .icon_as_template(true)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app: &AppHandle, event| {
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
}

/// Update the tray icon to reflect the current VPN state.
///
/// - `Disconnected`: static disconnected icon
/// - `Connecting`: animated frames alternating every 500ms
/// - `Connected`: static connected icon
///
/// Any running animation is stopped before applying the new state.
pub fn update_tray_icon(app_handle: &AppHandle, state: VpnState) {
    // Signal any running animation loop to exit.
    ANIMATION_STOP.store(true, Ordering::Relaxed);

    let Some(tray) = app_handle.tray_by_id("main-tray") else {
        return;
    };

    match state {
        VpnState::Disconnected => {
            let _ = tray.set_icon(Some(tauri::include_image!("icons/tray/disconnected.png")));
            let _ = tray.set_icon_as_template(true);
        }
        VpnState::Connected => {
            let _ = tray.set_icon(Some(tauri::include_image!("icons/tray/connected.png")));
            let _ = tray.set_icon_as_template(true);
        }
        VpnState::Connecting => {
            // Reset the stop flag before spawning so the new loop does not
            // exit immediately.
            ANIMATION_STOP.store(false, Ordering::Relaxed);

            let handle = app_handle.clone();
            tokio::spawn(async move {
                let mut use_frame_1 = true;
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_millis(500));

                loop {
                    interval.tick().await;

                    if ANIMATION_STOP.load(Ordering::Relaxed) {
                        break;
                    }

                    let Some(tray) = handle.tray_by_id("main-tray") else {
                        break;
                    };

                    if use_frame_1 {
                        let _ = tray
                            .set_icon(Some(tauri::include_image!("icons/tray/connecting-1.png")));
                    } else {
                        let _ = tray
                            .set_icon(Some(tauri::include_image!("icons/tray/connecting-2.png")));
                    }
                    let _ = tray.set_icon_as_template(true);
                    use_frame_1 = !use_frame_1;
                }
            });
        }
    }
}
