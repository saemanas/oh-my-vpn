fn main() {
    let attributes = tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "register_provider",
            "remove_provider",
            "list_providers",
            "list_regions",
            "connect",
            "disconnect",
            "check_orphaned_servers",
            "resolve_orphaned_server",
            "get_session_status",
            "get_preferences",
            "update_preferences",
        ]),
    );
    tauri_build::try_build(attributes).expect("error while running tauri-build");
}
