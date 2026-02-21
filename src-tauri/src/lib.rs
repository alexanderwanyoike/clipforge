mod commands;
mod state;
mod tray;

use clipforge_core::config::Config;
use clipforge_core::encode::hw_probe::probe_encoders;
use clipforge_core::library::Library;
use state::AppState;
use tauri::Manager;
use tracing::info;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "clipforge=debug,tauri=info".into()),
        )
        .init();

    let config = Config::load().unwrap_or_else(|e| {
        tracing::warn!(error = %e, "failed to load config, using defaults");
        Config::default()
    });

    if let Err(e) = config.ensure_dirs() {
        tracing::warn!(error = %e, "failed to create directories");
    }

    let app_state = AppState::new(config);

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::recording::start_recording,
            commands::recording::stop_recording,
            commands::recording::get_recording_status,
            commands::replay::toggle_replay_buffer,
            commands::replay::save_replay_clip,
            commands::replay::get_replay_status,
            commands::export::get_export_presets,
            commands::export::start_export,
            commands::library::get_recordings,
            commands::library::search_recordings,
            commands::library::delete_recording,
            commands::library::get_recording,
            commands::system::get_encoders,
            commands::system::get_audio_sources,
            commands::system::get_config,
            commands::system::update_config,
            commands::system::run_doctor,
        ])
        .setup(|app| {
            // Set window icon (taskbar / Alt+Tab)
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_icon(tauri::include_image!("icons/128x128.png"));
            }

            // Setup tray
            if let Err(e) = tray::setup_tray(app.handle()) {
                tracing::warn!(error = %e, "failed to setup tray");
            }

            // Probe encoders and init library in background
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();

                // Probe hardware encoders
                info!("probing hardware encoders...");
                let encoders = probe_encoders().await;
                info!(count = encoders.len(), "encoder probe complete");
                *state.encoders.write().await = encoders;

                // Initialize library database
                let config = state.config.read().await;
                let db_path = config
                    .paths
                    .recordings_dir
                    .parent()
                    .unwrap_or(&config.paths.recordings_dir)
                    .join("library.db");

                match Library::open(&db_path) {
                    Ok(lib) => {
                        *state.library.lock().await = Some(lib);
                        info!("library database initialized");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to open library database");
                    }
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
