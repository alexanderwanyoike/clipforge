use crate::state::{AppState, RecordingStatus};
use clipforge_core::capture::x11::create_capture_source;
use clipforge_core::encode::ffmpeg::build_recording_command;
use clipforge_core::encode::hw_probe::select_best_encoder;
use clipforge_core::process::FfmpegProcess;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tracing::{error, info};

#[derive(Debug, Clone, Serialize)]
pub struct RecordingState {
    pub status: RecordingStatus,
    pub elapsed_secs: u64,
    pub file_path: Option<String>,
}

#[tauri::command]
pub async fn start_recording(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let mut recorder = state.recorder.lock().await;

    if recorder.status != RecordingStatus::Idle {
        return Err("Already recording".to_string());
    }

    recorder.status = RecordingStatus::Starting;
    let _ = app.emit(
        "recording-state-changed",
        RecordingState {
            status: RecordingStatus::Starting,
            elapsed_secs: 0,
            file_path: None,
        },
    );

    let config = state.config.read().await;
    let encoders = state.encoders.read().await;

    if encoders.is_empty() {
        recorder.status = RecordingStatus::Idle;
        return Err("No encoders available. Run encoder probe first.".to_string());
    }

    let encoder = select_best_encoder(&encoders);
    let source = create_capture_source(&config)
        .await
        .map_err(|e| e.to_string())?;

    // Generate output filename
    let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
    let filename = format!("recording_{}.{}", timestamp, config.recording.container);
    let output_path = config.paths.recordings_dir.join(&filename);

    // Ensure recording directory exists
    std::fs::create_dir_all(&config.paths.recordings_dir).map_err(|e| e.to_string())?;

    let args = build_recording_command(&config, encoder, &source, &output_path).await;

    info!(output = %output_path.display(), "starting recording");

    match FfmpegProcess::spawn(args).await {
        Ok(process) => {
            recorder.process = Some(process);
            recorder.status = RecordingStatus::Recording;
            recorder.output_path = Some(output_path.clone());
            recorder.start_time = Some(std::time::Instant::now());

            let _ = app.emit(
                "recording-state-changed",
                RecordingState {
                    status: RecordingStatus::Recording,
                    elapsed_secs: 0,
                    file_path: Some(output_path.to_string_lossy().to_string()),
                },
            );

            // Start timer task
            let app_handle = app.clone();
            let recorder_state = state.recorder.clone();
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    let rec = recorder_state.lock().await;
                    if rec.status != RecordingStatus::Recording {
                        break;
                    }
                    let elapsed = rec.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0);
                    let _ = app_handle.emit("recording-timer", elapsed);
                }
            });

            Ok(())
        }
        Err(e) => {
            recorder.status = RecordingStatus::Idle;
            error!(error = %e, "failed to start recording");
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn stop_recording(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let mut recorder = state.recorder.lock().await;

    if recorder.status != RecordingStatus::Recording {
        return Err("Not recording".to_string());
    }

    recorder.status = RecordingStatus::Stopping;
    let _ = app.emit(
        "recording-state-changed",
        RecordingState {
            status: RecordingStatus::Stopping,
            elapsed_secs: 0,
            file_path: recorder
                .output_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
        },
    );

    if let Some(ref mut process) = recorder.process {
        process.stop_graceful().await.map_err(|e| e.to_string())?;
    }

    let output_path = recorder.output_path.take();
    recorder.process = None;
    recorder.status = RecordingStatus::Idle;
    recorder.start_time = None;

    let path_str = output_path
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    let _ = app.emit(
        "recording-state-changed",
        RecordingState {
            status: RecordingStatus::Idle,
            elapsed_secs: 0,
            file_path: None,
        },
    );

    // Index the recording in the library
    if let Some(ref path) = output_path {
        let state_clone = state.inner().library.clone();
        let config = state.config.read().await;
        let thumb_dir = config.paths.thumbnails_dir.clone();
        let path = path.clone();

        tokio::spawn(async move {
            if let Err(e) = index_recording(&state_clone, &path, &thumb_dir).await {
                error!(error = %e, "failed to index recording");
            }
        });
    }

    info!(path = %path_str, "recording stopped");
    Ok(path_str)
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
    let thumb_path = thumb_dir.join(format!("{}.jpg", id));
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

#[tauri::command]
pub async fn get_recording_status(state: State<'_, AppState>) -> Result<RecordingState, String> {
    let recorder = state.recorder.lock().await;
    let elapsed = recorder
        .start_time
        .map(|t| t.elapsed().as_secs())
        .unwrap_or(0);

    Ok(RecordingState {
        status: recorder.status,
        elapsed_secs: elapsed,
        file_path: recorder
            .output_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
    })
}
