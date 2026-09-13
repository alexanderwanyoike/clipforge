//! Linux adapter for the recording ports. Owns capture and encoder resources;
//! lifecycle policy is implemented once by RecordingService.
use super::{ActiveRecording, RecordingBackend, RecordingDestination, RecordingRequest};
use crate::{
    capture::{prepare_capture, wayland::WaylandCapture, PreparedCapture},
    encode::ffmpeg::{build_recording_command, build_replay_command},
    error::{Error, Result},
    process::{diagnostics::Diagnostics, EncoderInput, FfmpegProcess},
    replay::ReplayRing,
};
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Child,
    task::JoinHandle,
};

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
        let capture = prepare_capture(&config).await?;
        let source = capture.source();
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
        let (resources, input) = CaptureResources::start(capture)?;
        LinuxRecordingSession::start_encoder(args, resources, input).await
    }
}

pub struct LinuxRecordingSession {
    encoder: FfmpegProcess,
    capture: CaptureResources,
}

impl LinuxRecordingSession {
    async fn start_encoder(
        args: Vec<String>,
        capture: CaptureResources,
        input: EncoderInput,
    ) -> Result<Self> {
        let encoder = FfmpegProcess::spawn(args, input).await?;
        let mut session = Self { encoder, capture };
        let ready = tokio::select! {
            result = session.encoder.wait_for_frame(Duration::from_secs(20)) => result,
            error = session.capture.wait_failure() => Err(error),
        };
        if let Err(error) = ready {
            let _ = session.stop().await;
            return Err(error);
        }
        Ok(session)
    }
}

impl ActiveRecording for LinuxRecordingSession {
    async fn wait_failure(&mut self) -> Error {
        tokio::select! {
            error = self.encoder.wait_failure() => Error::Other(format!("{error}\n{}", self.capture.diagnostics())),
            error = self.capture.wait_failure() => error,
        }
    }

    async fn stop(&mut self) -> Result<()> {
        let result = self.encoder.stop().await;
        self.capture.close().await;
        result
    }
}

enum CaptureResources {
    Device,
    Portal {
        portal: WaylandCapture,
        feed: Box<VideoFeed>,
    },
    #[cfg(test)]
    TestPattern(VideoFeed),
}

impl CaptureResources {
    fn start(capture: PreparedCapture) -> Result<(Self, EncoderInput)> {
        match capture {
            PreparedCapture::X11(_) => Ok((Self::Device, EncoderInput::Device)),
            PreparedCapture::Wayland(mut portal) => {
                let (feed, input) = VideoFeed::new(portal.spawn_helper()?)?;
                Ok((
                    Self::Portal {
                        portal,
                        feed: Box::new(feed),
                    },
                    input,
                ))
            }
        }
    }

    async fn wait_failure(&mut self) -> Error {
        match self {
            Self::Device => std::future::pending().await,
            Self::Portal { portal, feed } => tokio::select! {
                _ = portal.closed.wait_for(|closed| *closed) => Error::Other("Desktop screen sharing was stopped".into()),
                result = feed.child.wait() => Error::Other(format!("PipeWire video helper exited: {result:?}\n{}", feed.diagnostics.detail())),
            },
            #[cfg(test)]
            Self::TestPattern(feed) => {
                let result = feed.child.wait().await;
                Error::Other(format!("Test video helper exited: {result:?}"))
            }
        }
    }

    fn diagnostics(&self) -> String {
        match self {
            Self::Device => String::new(),
            Self::Portal { feed, .. } => feed.diagnostics.detail(),
            #[cfg(test)]
            Self::TestPattern(feed) => feed.diagnostics.detail(),
        }
    }

    async fn close(&mut self) {
        match self {
            Self::Device => {}
            Self::Portal { portal, feed } => {
                feed.stop().await;
                portal.close().await;
            }
            #[cfg(test)]
            Self::TestPattern(feed) => feed.stop().await,
        }
    }
}

struct VideoFeed {
    child: Child,
    diagnostics: Diagnostics,
    reader: JoinHandle<()>,
}

impl VideoFeed {
    fn new(mut child: Child) -> Result<(Self, EncoderInput)> {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Other("Video helper stdout pipe was not created".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::Other("Video helper stderr pipe was not created".into()))?;
        let input = EncoderInput::Video(Stdio::from(stdout.into_owned_fd()?));
        let diagnostics = Diagnostics::default();
        let log = diagnostics.clone();
        let reader = tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => log.push(line),
                    Ok(None) => break,
                    Err(error) => {
                        log.push(format!("Cannot read video helper diagnostics: {error}"));
                        break;
                    }
                }
            }
        });
        Ok((
            Self {
                child,
                diagnostics,
                reader,
            },
            input,
        ))
    }

    async fn stop(&mut self) {
        let _ = self.child.kill().await;
    }
}

impl Drop for VideoFeed {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;
