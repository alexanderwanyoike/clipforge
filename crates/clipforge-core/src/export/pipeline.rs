use crate::error::Result;
use crate::export::presets::ExportPreset;
use crate::process::run_ffmpeg;
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
            args.extend(["-ss".to_string(), format!("{start:.3}")]);
        }
        args.extend(["-i".to_string(), job.input.to_string_lossy().to_string()]);

        if let Some(end) = job.trim_end {
            let duration = end - job.trim_start.unwrap_or(0.0);
            args.extend(["-t".to_string(), format!("{duration:.3}")]);
        }

        // Build filter chain
        let mut filters = Vec::new();

        // Crop to aspect ratio
        if let Some((aw, ah)) = job.preset.crop_aspect {
            filters.push(format!("crop=ih*{aw}/{ah}:ih"));
        }

        // Scale to target resolution
        if let Some((w, h)) = job.preset.resolution {
            filters.push(format!("scale={w}:{h}:flags=lanczos"));
        }

        // FPS
        if let Some(fps) = job.preset.fps {
            filters.push(format!("fps={fps}"));
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
            args.extend(["-af".to_string(), "loudnorm=I=-14:TP=-1:LRA=11".to_string()]);
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

        // Finite exports can finish before a recording startup probe sees its
        // first frame. Wait for the actual exit status instead of capture state.
        let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
        run_ffmpeg(&borrowed).await?;

        info!(output = %job.output.display(), "export completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_job(preset: ExportPreset, trim_start: Option<f64>, trim_end: Option<f64>) -> ExportJob {
        ExportJob {
            input: PathBuf::from("/tmp/input.mkv"),
            output: PathBuf::from("/tmp/output.mp4"),
            preset,
            trim_start,
            trim_end,
        }
    }

    #[test]
    fn basic_args_structure() {
        let job = make_job(ExportPreset::high_quality(), None, None);
        let args = ExportPipeline::build_args(&job);
        assert_eq!(args[0], "-y");
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"/tmp/input.mkv".to_string()));
        assert!(args.contains(&"/tmp/output.mp4".to_string()));
        assert!(args.contains(&"-c:v".to_string()));
    }

    #[test]
    fn trim_adds_ss_and_duration() {
        let job = make_job(ExportPreset::high_quality(), Some(10.0), Some(25.0));
        let args = ExportPipeline::build_args(&job);
        assert!(args.contains(&"-ss".to_string()));
        assert!(args.contains(&"-t".to_string()));
        // duration = 25 - 10 = 15
        let t_idx = args.iter().position(|a| a == "-t").unwrap();
        let duration: f64 = args[t_idx + 1].parse().unwrap();
        assert!((duration - 15.0).abs() < 0.01);
    }

    #[test]
    fn shorts_preset_produces_crop_and_scale() {
        let job = make_job(ExportPreset::shorts(), None, None);
        let args = ExportPipeline::build_args(&job);
        assert!(args.contains(&"-vf".to_string()));
        let vf_idx = args.iter().position(|a| a == "-vf").unwrap();
        let filter = &args[vf_idx + 1];
        assert!(
            filter.contains("crop="),
            "expected crop filter, got: {filter}"
        );
        assert!(
            filter.contains("scale="),
            "expected scale filter, got: {filter}"
        );
    }

    #[test]
    fn youtube_preset_produces_loudnorm() {
        let job = make_job(ExportPreset::youtube(), None, None);
        let args = ExportPipeline::build_args(&job);
        assert!(args.contains(&"-af".to_string()));
        let af_idx = args.iter().position(|a| a == "-af").unwrap();
        assert!(args[af_idx + 1].contains("loudnorm"));
    }

    #[test]
    fn high_quality_no_loudnorm() {
        let job = make_job(ExportPreset::high_quality(), None, None);
        let args = ExportPipeline::build_args(&job);
        assert!(!args.contains(&"-af".to_string()));
    }
}

#[cfg(test)]
mod completion_tests {
    use crate::process::run_ffmpeg;
    #[tokio::test]
    #[ignore = "requires FFmpeg with libx264"]
    async fn short_export_completes() {
        let temp = tempfile::tempdir().unwrap();
        let input = temp.path().join("input.mkv");
        run_ffmpeg(&[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x240:rate=30",
            "-t",
            "0.2",
            "-c:v",
            "libx264",
            input.to_str().unwrap(),
        ])
        .await
        .unwrap();
        let job = crate::export::pipeline::ExportJob {
            input,
            output: temp.path().join("export.mp4"),
            preset: crate::export::presets::ExportPreset::high_quality(),
            trim_start: None,
            trim_end: None,
        };
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            crate::export::pipeline::ExportPipeline::run(&job),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(job.output.exists());
    }
}
