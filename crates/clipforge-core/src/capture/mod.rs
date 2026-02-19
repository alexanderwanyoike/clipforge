pub mod x11;

use serde::{Deserialize, Serialize};

/// Represents a video capture source for FFmpeg
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureSource {
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
            CaptureSource::X11Fullscreen { display, width, height, fps } => {
                vec![
                    "-f".to_string(), "x11grab".to_string(),
                    "-framerate".to_string(), fps.to_string(),
                    "-video_size".to_string(), format!("{}x{}", width, height),
                    "-i".to_string(), format!("{}.0", display),
                ]
            }
            CaptureSource::X11Window { display, window_id, fps } => {
                vec![
                    "-f".to_string(), "x11grab".to_string(),
                    "-framerate".to_string(), fps.to_string(),
                    "-window_id".to_string(), window_id.clone(),
                    "-i".to_string(), format!("{}.0", display),
                ]
            }
            CaptureSource::X11Region { display, x, y, width, height, fps } => {
                vec![
                    "-f".to_string(), "x11grab".to_string(),
                    "-framerate".to_string(), fps.to_string(),
                    "-video_size".to_string(), format!("{}x{}", width, height),
                    "-i".to_string(), format!("{}.0+{},{}", display, x, y),
                ]
            }
        }
    }
}
