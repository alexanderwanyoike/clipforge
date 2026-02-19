use crate::error::{Error, Result};
use crate::process::run_ffmpeg;
use crate::replay::ring::ReplayRing;
use std::path::{Path, PathBuf};
use tracing::info;

/// Save the last N seconds from the replay ring buffer
pub async fn save_replay(ring: &ReplayRing, seconds: u32, output: &Path) -> Result<PathBuf> {
    let segments = ring.get_last_n_seconds(seconds)?;

    // Create concat file
    let concat_file = ring.segment_dir.join("concat.txt");
    let concat_content: String = segments
        .iter()
        .map(|p| format!("file '{}'", p.to_string_lossy()))
        .collect::<Vec<_>>()
        .join("\n");

    std::fs::write(&concat_file, &concat_content).map_err(Error::Io)?;

    // Run ffmpeg concat (copy, no re-encode)
    let output_path = output.to_path_buf();
    run_ffmpeg(&[
        "-f", "concat",
        "-safe", "0",
        "-i", &concat_file.to_string_lossy(),
        "-c", "copy",
        &output_path.to_string_lossy(),
    ])
    .await?;

    // Cleanup concat file
    let _ = std::fs::remove_file(&concat_file);

    info!(
        output = %output_path.display(),
        segments = segments.len(),
        seconds = seconds,
        "replay saved"
    );

    Ok(output_path)
}
