use crate::error::{Error, Result};
use crate::export::presets::ExportPreset;
use crate::process::FfmpegProcess;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportJob {
    pub input: PathBuf,
    pub output: PathBuf,
    pub preset: ExportPreset,
    pub trim_start: Option<f64>,
    pub trim_end: Option<f64>,
}

pub struct ExportPipeline;

impl ExportPipeline {
    /// Build FFmpeg args for an export job
    pub fn build_args(job: &ExportJob) -> Vec<String> {
        let mut args = vec!["-y".to_string()];

        // Input with optional seek
        if let Some(start) = job.trim_start {
            args.extend(["-ss".to_string(), format!("{:.3}", start)]);
        }
        args.extend(["-i".to_string(), job.input.to_string_lossy().to_string()]);

        if let Some(end) = job.trim_end {
            let duration = end - job.trim_start.unwrap_or(0.0);
            args.extend(["-t".to_string(), format!("{:.3}", duration)]);
        }

        // Build filter chain
        let mut filters = Vec::new();

        // Crop to aspect ratio
        if let Some((aw, ah)) = job.preset.crop_aspect {
            filters.push(format!("crop=ih*{}/{}:ih", aw, ah));
        }

        // Scale to target resolution
        if let Some((w, h)) = job.preset.resolution {
            filters.push(format!("scale={}:{}:flags=lanczos", w, h));
        }

        // FPS
        if let Some(fps) = job.preset.fps {
            filters.push(format!("fps={}", fps));
        }

        if !filters.is_empty() {
            args.extend(["-vf".to_string(), filters.join(",")]);
        }

        // Video codec
        args.extend(["-c:v".to_string(), job.preset.codec.clone()]);
        if let Some(ref bitrate) = job.preset.bitrate {
            args.extend(["-b:v".to_string(), bitrate.clone()]);
        }

        // Audio
        if job.preset.loudnorm {
            args.extend([
                "-af".to_string(),
                "loudnorm=I=-14:TP=-1:LRA=11".to_string(),
            ]);
        }
        args.extend([
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "192k".to_string(),
        ]);

        // Output
        args.extend([
            "-f".to_string(),
            job.preset.container.clone(),
            "-movflags".to_string(),
            "+faststart".to_string(),
            job.output.to_string_lossy().to_string(),
        ]);

        args
    }

    /// Run an export job
    pub async fn run(job: &ExportJob) -> Result<()> {
        let args = Self::build_args(job);
        info!(args = ?args, "starting export");

        let process = FfmpegProcess::spawn(args).await?;

        // Wait for completion by watching state
        let mut state_rx = process.subscribe_state();
        loop {
            state_rx
                .changed()
                .await
                .map_err(|_| Error::Other("state channel closed".into()))?;
            let state = *state_rx.borrow();
            match state {
                crate::process::ProcessState::Stopped => break,
                crate::process::ProcessState::Failed => {
                    return Err(Error::ExportFailed("FFmpeg process failed".into()));
                }
                _ => continue,
            }
        }

        info!(output = %job.output.display(), "export completed");
        Ok(())
    }
}
