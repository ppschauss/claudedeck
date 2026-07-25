mod commands;
mod error;
mod reconnect_supervisor;
mod state;

use reconnect_supervisor::ReconnectSupervisor;
use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .manage(AppState::new())
        .manage(ReconnectSupervisor::new())
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::accept_hostkey_and_connect,
            commands::disconnect,
            commands::get_config,
            commands::set_config,
            commands::save_secret,
            commands::has_secret,
            commands::delete_secret,
            commands::list_sessions,
            commands::open_session,
            commands::start_project,
            commands::write_session,
            commands::resize_session,
            commands::close_session,
            commands::kill_session,
            commands::list_commands,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            // Task 6: Reconnect-Supervisor + periodischer Keepalive laufen für die gesamte
            // App-Lebensdauer, unabhängig vom aktuellen Verbindungsstatus (siehe
            // `reconnect_supervisor::spawn`).
            reconnect_supervisor::spawn(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
