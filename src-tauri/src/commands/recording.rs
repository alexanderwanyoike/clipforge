use crate::state::AppState;
use clipforge_core::{
    encode::hw_probe::select_best_encoder,
    recording::{RecordingDestination, RecordingRequest, RecordingSnapshot, RecordingStatus},
};
use tauri::{AppHandle, Emitter, State};
use tracing::error;

#[tauri::command]
pub async fn start_recording(state: State<'_, AppState>) -> Result<(), String> {
    let config = state.config.read().await.clone();
    let encoders = state.encoders.read().await;
    let encoder = select_best_encoder(&encoders)
        .map_err(|e| e.to_string())?
        .clone();
    drop(encoders);
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let output = config.paths.recordings_dir.join(format!(
        "recording_{}.{}",
        timestamp, config.recording.container
    ));
    state
        .recorder
        .start(RecordingRequest {
            config,
            encoder,
            destination: RecordingDestination::File(output),
        })
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn stop_recording(state: State<'_, AppState>) -> Result<String, String> {
    let output = state.recorder.snapshot().file_path;
    state.recorder.stop().await.map_err(|e| e.to_string())?;
    if let Some(path) = output.as_ref() {
        let library = state.library.clone();
        let thumbnails = state.config.read().await.paths.thumbnails_dir.clone();
        let path = std::path::PathBuf::from(path);
        tokio::spawn(async move {
            if let Err(error) = index_recording(&library, &path, &thumbnails).await {
                error!(%error, "failed to index recording");
            }
        });
    }
    Ok(output.unwrap_or_default())
}

#[tauri::command]
pub async fn get_recording_status(state: State<'_, AppState>) -> Result<RecordingSnapshot, String> {
    Ok(state.recorder.snapshot())
}

/// Translate service snapshots into the existing UI events. Both recording
/// and replay use the same service; this adapter contains no process supervision.
pub fn forward_events(
    app: AppHandle,
    mut snapshots: tokio::sync::watch::Receiver<RecordingSnapshot>,
    replay: bool,
) {
    tauri::async_runtime::spawn(async move {
        loop {
            let snapshot = snapshots.borrow_and_update().clone();
            if replay {
                let _ = app.emit(
                    "replay-state-changed",
                    snapshot.status == RecordingStatus::Recording,
                );
            } else {
                let _ = app.emit("recording-state-changed", &snapshot);
                let _ = app.emit("recording-timer", snapshot.elapsed_secs);
            }
            if let Some(error) = snapshot.error {
                let _ = app.emit("recording-error", error);
            }
            if snapshots.changed().await.is_err() {
                break;
            }
        }
    });
}

async fn index_recording(
    library: &std::sync::Arc<tokio::sync::Mutex<Option<clipforge_core::library::Library>>>,
    path: &std::path::Path,
    thumb_dir: &std::path::Path,
) -> Result<(), String> {
    use clipforge_core::library::db::{generate_thumbnail, probe_media, Recording};

    let info = probe_media(path).await.map_err(|e| e.to_string())?;

    let id = uuid::Uuid::new_v4().to_string();
    let title = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Untitled".to_string());

    // Generate thumbnail
    let _ = std::fs::create_dir_all(thumb_dir);
    let thumb_path = thumb_dir.join(format!("{id}.jpg"));
    let _ = generate_thumbnail(path, &thumb_path).await;

    let recording = Recording {
        id,
        title,
        file_path: path.to_string_lossy().to_string(),
        file_size: info.file_size,
        duration: info.duration,
        resolution: format!("{}x{}", info.width, info.height),
        fps: info.fps,
        codec: info.codec,
        container: path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default(),
        source_type: "recording".to_string(),
        game_name: None,
        created_at: chrono::Local::now().to_rfc3339(),
        thumbnail_path: if thumb_path.exists() {
            Some(thumb_path.to_string_lossy().to_string())
        } else {
            None
        },
    };

    let lib = library.lock().await;
    if let Some(ref lib) = *lib {
        lib.insert(&recording).map_err(|e| e.to_string())?;
    }

    Ok(())
}
