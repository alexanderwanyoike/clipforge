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
