fn main() {
    // Only these commands may be called from the web view. Nothing else is exposed.
    let app = tauri_build::AppManifest::new().commands(&[
        "auth_status", "auth_login", "auth_logout", "pack_status", "pack_update", "game_play",
        "game_stop", "settings_get", "settings_set", "open_game_folder", "news_get", "open_url",
    ]);
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(app)).expect("tauri build");
}
