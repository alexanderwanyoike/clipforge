use crate::state::AppState;
use clipforge_core::audio::{list_audio_sources, AudioSource};
use clipforge_core::config::Config;
use clipforge_core::doctor::{run_diagnostics, DiagnosticReport};
use clipforge_core::encode::hw_probe::EncoderInfo;
use tauri::State;

#[tauri::command]
pub async fn get_encoders(state: State<'_, AppState>) -> Result<Vec<EncoderInfo>, String> {
    let encoders = state.encoders.read().await;
    Ok(encoders.clone())
}

#[tauri::command]
pub async fn get_audio_sources() -> Result<Vec<AudioSource>, String> {
    list_audio_sources().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    let config = state.config.read().await;
    Ok(config.clone())
}

#[tauri::command]
pub async fn update_config(state: State<'_, AppState>, config: Config) -> Result<(), String> {
    config.save().map_err(|e| e.to_string())?;
    *state.config.write().await = config;
    Ok(())
}

#[tauri::command]
pub async fn run_doctor() -> Result<DiagnosticReport, String> {
    Ok(run_diagnostics().await)
}
