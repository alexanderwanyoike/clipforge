use crate::state::AppState;
use clipforge_core::capture::x11::create_capture_source;
use clipforge_core::encode::ffmpeg::build_replay_command;
use clipforge_core::encode::hw_probe::select_best_encoder;
use clipforge_core::process::FfmpegProcess;
use clipforge_core::replay::ring::ReplayRing;
use clipforge_core::replay::save::save_replay;
use tauri::{AppHandle, Emitter, State};
use tracing::{error, info};

#[tauri::command]
pub async fn toggle_replay_buffer(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let mut replay = state.replay.lock().await;

    if replay.active {
        // Stop replay buffer
        if let Some(ref mut process) = replay.process {
            let _ = process.stop_graceful().await;
        }
        if let Some(ref ring) = replay.ring {
            let _ = ring.cleanup();
        }
        replay.process = None;
        replay.ring = None;
        replay.active = false;

        let _ = app.emit("replay-state-changed", false);
        info!("replay buffer stopped");
        Ok(false)
    } else {
        // Start replay buffer
        let config = state.config.read().await;
        let encoders = state.encoders.read().await;

        if encoders.is_empty() {
            return Err("No encoders available".to_string());
        }

        let encoder = select_best_encoder(&encoders);
        let source = create_capture_source(&config)
            .await
            .map_err(|e| e.to_string())?;

        // Ensure cache directory
        std::fs::create_dir_all(&config.paths.replay_cache_dir).map_err(|e| e.to_string())?;

        let ring = ReplayRing::new(
            &config.paths.replay_cache_dir,
            config.replay.segment_secs,
            config.replay.max_segments,
        );
        ring.cleanup().map_err(|e| e.to_string())?;

        let args = build_replay_command(&config, encoder, &source).await;

        match FfmpegProcess::spawn(args).await {
            Ok(process) => {
                replay.process = Some(process);
                replay.ring = Some(ring);
                replay.active = true;

                let _ = app.emit("replay-state-changed", true);
                info!("replay buffer started");
                Ok(true)
            }
            Err(e) => {
                error!(error = %e, "failed to start replay buffer");
                Err(e.to_string())
            }
        }
    }
}

#[tauri::command]
pub async fn save_replay_clip(
    app: AppHandle,
    state: State<'_, AppState>,
    seconds: Option<u32>,
) -> Result<String, String> {
    let replay = state.replay.lock().await;

    if !replay.active {
        return Err("Replay buffer is not active".to_string());
    }

    let ring = replay.ring.as_ref().ok_or("No replay ring")?;
    let config = state.config.read().await;

    let duration = seconds.unwrap_or(30);
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let filename = format!("replay_{}.mkv", timestamp);
    let output_path = config.paths.replays_dir.join(&filename);

    std::fs::create_dir_all(&config.paths.replays_dir).map_err(|e| e.to_string())?;

    let result = save_replay(ring, duration, &output_path)
        .await
        .map_err(|e| e.to_string())?;

    let path_str = result.to_string_lossy().to_string();
    let _ = app.emit("replay-saved", &path_str);

    info!(path = %path_str, seconds = duration, "replay saved");
    Ok(path_str)
}

#[tauri::command]
pub async fn get_replay_status(state: State<'_, AppState>) -> Result<bool, String> {
    let replay = state.replay.lock().await;
    Ok(replay.active)
}
