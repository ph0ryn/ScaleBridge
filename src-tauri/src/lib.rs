mod commands;
mod services;
mod state;
mod tray;
mod window;

use std::fs;

use scalebridge_storage::Storage;
use tauri::Manager;
use tauri_plugin_autostart::MacosLauncher;

use crate::state::AppState;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .macos_launcher(MacosLauncher::LaunchAgent)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::get_current_status,
            commands::list_recent_measurements,
            commands::list_devices,
            commands::list_recent_events,
            commands::list_recent_raw_packets,
            commands::start_watcher,
            commands::stop_watcher,
            commands::get_autostart_status,
            commands::set_autostart_enabled,
        ])
        .setup(|app| {
            tray::create_tray(app)?;

            let data_dir = app.path().app_data_dir()?;
            fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("scalebridge.sqlite");
            let storage = Storage::open(db_path)?;
            let state = AppState::new(storage);
            app.manage(state.clone());

            if let Err(error) = services::start_watcher(app.handle().clone(), state) {
                eprintln!("failed to start watcher: {error}");
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running ScaleBridge");
}
