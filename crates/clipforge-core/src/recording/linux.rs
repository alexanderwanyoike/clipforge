//! Linux adapter for the recording ports. Owns capture and encoder resources;
//! lifecycle policy is implemented once by RecordingService.
use super::{ActiveRecording, RecordingBackend, RecordingDestination, RecordingRequest};
use crate::{
    capture::x11::create_capture_source,
    encode::ffmpeg::{build_recording_command, build_replay_command},
    error::{Error, Result},
    process::{EncoderInput, FfmpegProcess},
    replay::ReplayRing,
};
use std::time::Duration;

#[derive(Clone, Default)]
pub struct LinuxRecordingBackend;

impl RecordingBackend for LinuxRecordingBackend {
    type Session = LinuxRecordingSession;

    async fn start(&self, request: RecordingRequest) -> Result<Self::Session> {
        let RecordingRequest {
            mut config,
            encoder,
            destination,
        } = request;
        let source = create_capture_source(&config).await?;
        let args = match destination {
            RecordingDestination::File(path) => {
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                build_recording_command(&config, &encoder, &source, &path).await
            }
            RecordingDestination::Replay {
                directory,
                segment_secs,
                max_segments,
            } => {
                std::fs::create_dir_all(&directory)?;
                ReplayRing::new(&directory, segment_secs, max_segments).cleanup()?;
                config.paths.replay_cache_dir = directory;
                config.replay.segment_secs = segment_secs;
                config.replay.max_segments = max_segments;
                build_replay_command(&config, &encoder, &source).await
            }
        };
        let encoder = FfmpegProcess::spawn(args, EncoderInput::Device).await?;
        let mut session = LinuxRecordingSession { encoder };
        if let Err(error) = session
            .encoder
            .wait_for_frame(Duration::from_secs(20))
            .await
        {
            let _ = session.stop().await;
            return Err(error);
        }
        Ok(session)
    }
}

pub struct LinuxRecordingSession {
    encoder: FfmpegProcess,
}

impl ActiveRecording for LinuxRecordingSession {
    async fn wait_failure(&mut self) -> Error {
        self.encoder.wait_failure().await
    }

    async fn stop(&mut self) -> Result<()> {
        self.encoder.stop().await
    }
}
