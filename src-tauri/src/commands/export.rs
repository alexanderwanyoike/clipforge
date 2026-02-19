use crate::state::AppState;
use clipforge_core::export::pipeline::{ExportJob, ExportPipeline};
use clipforge_core::export::presets::ExportPreset;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, State};
use tracing::info;

#[tauri::command]
pub async fn get_export_presets() -> Result<Vec<ExportPreset>, String> {
    Ok(ExportPreset::all())
}

#[tauri::command]
pub async fn start_export(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
    preset_id: String,
    trim_start: Option<f64>,
    trim_end: Option<f64>,
    output: Option<String>,
) -> Result<String, String> {
    let preset = ExportPreset::all()
        .into_iter()
        .find(|p| p.id == preset_id)
        .ok_or_else(|| format!("Unknown preset: {preset_id}"))?;

    let config = state.config.read().await;

    let output_path = if let Some(out) = output {
        PathBuf::from(out)
    } else {
        let input_stem = PathBuf::from(&input)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "export".to_string());
        let filename = format!("{}_{}.{}", input_stem, preset_id, preset.container);
        config.export.output_dir.join(filename)
    };

    std::fs::create_dir_all(output_path.parent().unwrap_or(&config.export.output_dir))
        .map_err(|e| e.to_string())?;

    let job = ExportJob {
        input: PathBuf::from(&input),
        output: output_path.clone(),
        preset,
        trim_start,
        trim_end,
    };

    let _ = app.emit("export-started", &output_path.to_string_lossy().to_string());

    // Run export in background
    let app_handle = app.clone();
    let output_str = output_path.to_string_lossy().to_string();

    tokio::spawn(async move {
        match ExportPipeline::run(&job).await {
            Ok(()) => {
                info!(output = %output_str, "export completed");
                let _ = app_handle.emit("export-completed", &output_str);
            }
            Err(e) => {
                let _ = app_handle.emit("export-failed", e.to_string());
            }
        }
    });

    Ok(output_path.to_string_lossy().to_string())
}
