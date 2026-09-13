use crate::state::AppState;
use clipforge_core::{
    encode::hw_probe::select_best_encoder,
    recording::{RecordingDestination, RecordingRequest, RecordingStatus},
    replay::{ring::ReplayRing, save::save_replay},
};
use tauri::{AppHandle, Emitter, State};
use tracing::info;

#[tauri::command]
pub async fn toggle_replay_buffer(state: State<'_, AppState>) -> Result<bool, String> {
    if state.replay.snapshot().status != RecordingStatus::Idle {
        state.replay.stop().await.map_err(|e| e.to_string())?;
        return Ok(false);
    }
    let config = state.config.read().await.clone();
    let encoders = state.encoders.read().await;
    let encoder = select_best_encoder(&encoders)
        .map_err(|e| e.to_string())?
        .clone();
    drop(encoders);
    let destination = RecordingDestination::replay(&config);
    state
        .replay
        .start(RecordingRequest {
            config,
            encoder,
            destination,
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn save_replay_clip(
    app: AppHandle,
    state: State<'_, AppState>,
    seconds: Option<u32>,
) -> Result<String, String> {
    let replay = state.replay.snapshot();

    if replay.status != RecordingStatus::Recording {
        return Err("Replay buffer is not active".to_string());
    }

    let ring = match replay.destination {
        Some(RecordingDestination::Replay {
            directory,
            segment_secs,
            max_segments,
        }) => ReplayRing::new(&directory, segment_secs, max_segments),
        _ => return Err("Replay destination is unavailable".into()),
    };
    let config = state.config.read().await.clone();

    let duration = seconds.unwrap_or(30);
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let filename = format!("replay_{timestamp}.mkv");
    let output_path = config.paths.replays_dir.join(&filename);

    std::fs::create_dir_all(&config.paths.replays_dir).map_err(|e| e.to_string())?;

    let result = save_replay(&ring, duration, &output_path)
        .await
        .map_err(|e| e.to_string())?;

    let path_str = result.to_string_lossy().to_string();
    let _ = app.emit("replay-saved", &path_str);

    info!(path = %path_str, seconds = duration, "replay saved");
    Ok(path_str)
}

#[tauri::command]
pub async fn get_replay_status(state: State<'_, AppState>) -> Result<bool, String> {
    let replay = state.replay.snapshot();
    Ok(replay.status == RecordingStatus::Recording)
}
