use crate::state::AppState;
use clipforge_core::library::db::Recording;
use tauri::State;

#[tauri::command]
pub async fn get_recordings(
    state: State<'_, AppState>,
    limit: Option<u32>,
    offset: Option<u32>,
) -> Result<Vec<Recording>, String> {
    let lib = state.library.lock().await;
    match lib.as_ref() {
        Some(lib) => lib
            .list(limit.unwrap_or(50), offset.unwrap_or(0))
            .map_err(|e| e.to_string()),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub async fn search_recordings(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<Recording>, String> {
    let lib = state.library.lock().await;
    match lib.as_ref() {
        Some(lib) => lib.search(&query).map_err(|e| e.to_string()),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub async fn delete_recording(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let lib = state.library.lock().await;
    match lib.as_ref() {
        Some(lib) => lib.delete(&id).map_err(|e| e.to_string()),
        None => Err("Library not initialized".to_string()),
    }
}

#[tauri::command]
pub async fn get_recording(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<Recording>, String> {
    let lib = state.library.lock().await;
    match lib.as_ref() {
        Some(lib) => lib.get(&id).map_err(|e| e.to_string()),
        None => Ok(None),
    }
}
