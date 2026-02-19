use crate::capture::CaptureSource;
use crate::config::{Config, Quality};
use crate::encode::hw_probe::{EncoderInfo, HwAccelType};
use std::path::Path;

/// Builds FFmpeg argument vectors for recording commands
pub struct FfmpegCommandBuilder {
    args: Vec<String>,
}

impl FfmpegCommandBuilder {
    pub fn new() -> Self {
        Self {
            args: vec!["-y".to_string()],
        }
    }

    /// Add hardware device initialization (VA-API)
    pub fn with_hw_device(mut self, encoder: &EncoderInfo) -> Self {
        if let Some(device) = &encoder.device {
            if encoder.hw_accel == HwAccelType::Vaapi {
                self.args.extend([
                    "-vaapi_device".to_string(),
                    device.clone(),
                ]);
            }
        }
        self
    }

    /// Add video capture input
    pub fn with_capture_source(mut self, source: &CaptureSource) -> Self {
        self.args.extend(source.to_ffmpeg_args());
        self
    }

    /// Add PulseAudio/PipeWire audio input
    pub fn with_audio(mut self, source: &str) -> Self {
        self.args.extend([
            "-f".to_string(),
            "pulse".to_string(),
            "-i".to_string(),
            source.to_string(),
        ]);
        self
    }

    /// Add video encoding with hardware acceleration
    pub fn with_encoder(mut self, encoder: &EncoderInfo, quality: &Quality) -> Self {
        match encoder.hw_accel {
            HwAccelType::Vaapi => {
                self.args.extend([
                    "-filter_complex".to_string(),
                    "[0:v]hwupload,scale_vaapi=format=nv12[vout]".to_string(),
                    "-map".to_string(),
                    "[vout]".to_string(),
                ]);
                self.args.extend([
                    "-c:v".to_string(),
                    "h264_vaapi".to_string(),
                    "-rc_mode".to_string(),
                    "CQP".to_string(),
                ]);
                let qp = quality_to_qp(quality);
                self.args.extend(["-qp".to_string(), qp.to_string()]);
                self.args.extend(["-g".to_string(), "120".to_string()]);
            }
            HwAccelType::Nvenc => {
                self.args.extend([
                    "-map".to_string(), "0:v".to_string(),
                    "-c:v".to_string(), "h264_nvenc".to_string(),
                    "-preset".to_string(), "p4".to_string(),
                    "-rc".to_string(), "constqp".to_string(),
                ]);
                let qp = quality_to_qp(quality);
                self.args.extend(["-qp".to_string(), qp.to_string()]);
                self.args.extend(["-g".to_string(), "120".to_string()]);
            }
            HwAccelType::Qsv => {
                self.args.extend([
                    "-map".to_string(), "0:v".to_string(),
                    "-c:v".to_string(), "h264_qsv".to_string(),
                    "-preset".to_string(), "medium".to_string(),
                ]);
                let qp = quality_to_qp(quality);
                self.args.extend(["-global_quality".to_string(), qp.to_string()]);
                self.args.extend(["-g".to_string(), "120".to_string()]);
            }
            HwAccelType::Software => {
                self.args.extend([
                    "-map".to_string(), "0:v".to_string(),
                    "-c:v".to_string(), "libx264".to_string(),
                    "-preset".to_string(), "fast".to_string(),
                ]);
                let crf = quality_to_crf(quality);
                self.args.extend(["-crf".to_string(), crf.to_string()]);
                self.args.extend(["-g".to_string(), "120".to_string()]);
            }
        }
        self
    }

    /// Add audio encoding
    pub fn with_audio_encode(mut self, has_audio: bool) -> Self {
        if has_audio {
            self.args.extend([
                "-map".to_string(), "1:a".to_string(),
                "-c:a".to_string(), "aac".to_string(),
                "-b:a".to_string(), "192k".to_string(),
            ]);
        }
        self
    }

    /// Set output to a file
    pub fn with_output(mut self, path: &Path, container: &str) -> Self {
        let ffmpeg_format = container_to_ffmpeg_format(container);
        self.args.extend([
            "-f".to_string(),
            ffmpeg_format.to_string(),
            path.to_string_lossy().to_string(),
        ]);
        self
    }

    /// Set output to segmented files for replay buffer
    pub fn with_segment_output(
        mut self,
        segment_dir: &Path,
        segment_time: u32,
        segment_wrap: u32,
    ) -> Self {
        let segment_pattern = segment_dir.join("seg_%03d.mkv");
        let segment_list = segment_dir.join("segments.csv");

        self.args.extend([
            "-f".to_string(), "segment".to_string(),
            "-segment_time".to_string(), segment_time.to_string(),
            "-segment_format".to_string(), "matroska".to_string(),
            "-segment_wrap".to_string(), segment_wrap.to_string(),
            "-segment_list".to_string(), segment_list.to_string_lossy().to_string(),
            "-segment_list_type".to_string(), "csv".to_string(),
            "-reset_timestamps".to_string(), "1".to_string(),
            segment_pattern.to_string_lossy().to_string(),
        ]);
        self
    }

    pub fn build(self) -> Vec<String> {
        self.args
    }
}

/// Build a complete recording command
pub fn build_recording_command(
    config: &Config,
    encoder: &EncoderInfo,
    source: &CaptureSource,
    output: &Path,
) -> Vec<String> {
    let mut builder = FfmpegCommandBuilder::new()
        .with_hw_device(encoder)
        .with_capture_source(source);

    if config.recording.audio_enabled {
        builder = builder.with_audio(&config.recording.audio_source);
    }

    builder = builder
        .with_encoder(encoder, &config.recording.quality)
        .with_audio_encode(config.recording.audio_enabled)
        .with_output(output, &config.recording.container);

    builder.build()
}

/// Build a segmented recording command for replay buffer
pub fn build_replay_command(
    config: &Config,
    encoder: &EncoderInfo,
    source: &CaptureSource,
) -> Vec<String> {
    let mut builder = FfmpegCommandBuilder::new()
        .with_hw_device(encoder)
        .with_capture_source(source);

    if config.recording.audio_enabled {
        builder = builder.with_audio(&config.recording.audio_source);
    }

    builder = builder
        .with_encoder(encoder, &config.recording.quality)
        .with_audio_encode(config.recording.audio_enabled)
        .with_segment_output(
            &config.paths.replay_cache_dir,
            config.replay.segment_secs,
            config.replay.max_segments,
        );

    builder.build()
}

fn quality_to_qp(quality: &Quality) -> u32 {
    match quality {
        Quality::Low => 30,
        Quality::Medium => 25,
        Quality::High => 20,
        Quality::Lossless => 0,
        Quality::Custom { qp } => *qp,
    }
}

fn quality_to_crf(quality: &Quality) -> u32 {
    match quality {
        Quality::Low => 28,
        Quality::Medium => 23,
        Quality::High => 18,
        Quality::Lossless => 0,
        Quality::Custom { qp } => *qp,
    }
}

/// Map user-facing container names to FFmpeg format names
fn container_to_ffmpeg_format(container: &str) -> &str {
    match container {
        "mkv" => "matroska",
        "mp4" => "mp4",
        "webm" => "webm",
        "avi" => "avi",
        "mov" => "mov",
        "ts" => "mpegts",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quality_to_qp_values() {
        assert_eq!(quality_to_qp(&Quality::Low), 30);
        assert_eq!(quality_to_qp(&Quality::Medium), 25);
        assert_eq!(quality_to_qp(&Quality::High), 20);
        assert_eq!(quality_to_qp(&Quality::Lossless), 0);
        assert_eq!(quality_to_qp(&Quality::Custom { qp: 15 }), 15);
    }

    #[test]
    fn quality_to_crf_values() {
        assert_eq!(quality_to_crf(&Quality::Low), 28);
        assert_eq!(quality_to_crf(&Quality::Medium), 23);
        assert_eq!(quality_to_crf(&Quality::High), 18);
        assert_eq!(quality_to_crf(&Quality::Lossless), 0);
    }

    #[test]
    fn container_format_mapping() {
        assert_eq!(container_to_ffmpeg_format("mkv"), "matroska");
        assert_eq!(container_to_ffmpeg_format("mp4"), "mp4");
        assert_eq!(container_to_ffmpeg_format("ts"), "mpegts");
    }

    #[test]
    fn container_format_unknown_passthrough() {
        assert_eq!(container_to_ffmpeg_format("flv"), "flv");
        assert_eq!(container_to_ffmpeg_format("ogg"), "ogg");
    }

    #[test]
    fn ffmpeg_command_builder_basic_structure() {
        let builder = FfmpegCommandBuilder::new();
        let args = builder.build();
        assert_eq!(args, vec!["-y"]);
    }
}
