pub mod wayland;
pub mod x11;

use crate::{
    config::Config,
    error::{Error, Result},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureBackend {
    X11,
    Wayland,
}

pub fn detect_backend(
    session: Option<&str>,
    wayland: Option<&str>,
    display: Option<&str>,
) -> Result<CaptureBackend> {
    if session == Some("wayland") || wayland.is_some_and(|s| !s.is_empty()) {
        Ok(CaptureBackend::Wayland)
    } else if display.is_some_and(|s| !s.is_empty()) {
        Ok(CaptureBackend::X11)
    } else {
        Err(Error::NoDisplay)
    }
}

pub fn current_backend() -> Result<CaptureBackend> {
    detect_backend(
        std::env::var("XDG_SESSION_TYPE").ok().as_deref(),
        std::env::var("WAYLAND_DISPLAY").ok().as_deref(),
        std::env::var("DISPLAY").ok().as_deref(),
    )
}

pub enum PreparedCapture {
    X11(CaptureSource),
    Wayland(wayland::WaylandCapture),
}

impl PreparedCapture {
    pub fn source(&self) -> CaptureSource {
        match self {
            Self::X11(source) => source.clone(),
            Self::Wayland(_) => CaptureSource::Wayland,
        }
    }
}

pub async fn prepare_capture(config: &Config) -> Result<PreparedCapture> {
    if !(1..=240).contains(&config.recording.fps) {
        return Err(Error::Config(
            "Recording FPS must be between 1 and 240".into(),
        ));
    }
    match current_backend()? {
        CaptureBackend::X11 => Ok(PreparedCapture::X11(
            x11::create_capture_source(config).await?,
        )),
        CaptureBackend::Wayland => Ok(PreparedCapture::Wayland(
            wayland::WaylandCapture::open(&config.recording.capture_mode, config.recording.fps)
                .await?,
        )),
    }
}

use serde::{Deserialize, Serialize};

/// Represents a video capture source for FFmpeg
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureSource {
    Wayland,
    X11Fullscreen {
        display: String,
        width: u32,
        height: u32,
        fps: u32,
    },
    X11Window {
        display: String,
        window_id: String,
        fps: u32,
    },
    X11Region {
        display: String,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
        fps: u32,
    },
}

impl CaptureSource {
    /// Convert to FFmpeg input arguments
    pub fn to_ffmpeg_args(&self) -> Vec<String> {
        match self {
            CaptureSource::Wayland => [
                "-thread_queue_size",
                "8",
                "-probesize",
                "32",
                "-analyzeduration",
                "0",
                "-f",
                "yuv4mpegpipe",
                "-i",
                "pipe:0",
            ]
            .map(String::from)
            .into(),
            CaptureSource::X11Fullscreen {
                display,
                width,
                height,
                fps,
            } => {
                vec![
                    "-f".to_string(),
                    "x11grab".to_string(),
                    "-framerate".to_string(),
                    fps.to_string(),
                    "-video_size".to_string(),
                    format!("{}x{}", width, height),
                    "-i".to_string(),
                    format!("{}.0", display),
                ]
            }
            CaptureSource::X11Window {
                display,
                window_id,
                fps,
            } => {
                vec![
                    "-f".to_string(),
                    "x11grab".to_string(),
                    "-framerate".to_string(),
                    fps.to_string(),
                    "-window_id".to_string(),
                    window_id.clone(),
                    "-i".to_string(),
                    format!("{}.0", display),
                ]
            }
            CaptureSource::X11Region {
                display,
                x,
                y,
                width,
                height,
                fps,
            } => {
                vec![
                    "-f".to_string(),
                    "x11grab".to_string(),
                    "-framerate".to_string(),
                    fps.to_string(),
                    "-video_size".to_string(),
                    format!("{}x{}", width, height),
                    "-i".to_string(),
                    format!("{}.0+{},{}", display, x, y),
                ]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fullscreen_args() {
        let source = CaptureSource::X11Fullscreen {
            display: ":0".to_string(),
            width: 1920,
            height: 1080,
            fps: 60,
        };
        let args = source.to_ffmpeg_args();
        assert!(args.contains(&"x11grab".to_string()));
        assert!(args.contains(&"1920x1080".to_string()));
        assert!(args.contains(&"60".to_string()));
        assert!(args.contains(&":0.0".to_string()));
    }

    #[test]
    fn region_args_include_offset() {
        let source = CaptureSource::X11Region {
            display: ":0".to_string(),
            x: 100,
            y: 200,
            width: 800,
            height: 600,
            fps: 30,
        };
        let args = source.to_ffmpeg_args();
        assert!(args.contains(&"x11grab".to_string()));
        assert!(args.contains(&"800x600".to_string()));
        assert!(args.contains(&":0.0+100,200".to_string()));
    }

    #[test]
    fn window_args_include_window_id() {
        let source = CaptureSource::X11Window {
            display: ":0".to_string(),
            window_id: "0x3a00004".to_string(),
            fps: 60,
        };
        let args = source.to_ffmpeg_args();
        assert!(args.contains(&"-window_id".to_string()));
        assert!(args.contains(&"0x3a00004".to_string()));
    }
}
