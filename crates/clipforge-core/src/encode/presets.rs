use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityPreset {
    pub name: String,
    pub description: String,
    pub resolution: Option<(u32, u32)>,
    pub fps: Option<u32>,
    pub codec: String,
    pub bitrate: Option<String>,
    pub crop: Option<CropMode>,
    pub loudnorm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CropMode {
    /// Crop to aspect ratio (e.g., 9:16 for vertical)
    AspectRatio { w: u32, h: u32 },
    /// Exact pixel crop
    Exact { x: u32, y: u32, w: u32, h: u32 },
}

impl QualityPreset {
    pub fn shorts() -> Self {
        Self {
            name: "TikTok/Shorts".to_string(),
            description: "Vertical 9:16 for TikTok, YouTube Shorts, Reels".to_string(),
            resolution: Some((1080, 1920)),
            fps: Some(60),
            codec: "h264".to_string(),
            bitrate: Some("8M".to_string()),
            crop: Some(CropMode::AspectRatio { w: 9, h: 16 }),
            loudnorm: true,
        }
    }

    pub fn youtube() -> Self {
        Self {
            name: "YouTube 16:9".to_string(),
            description: "Standard 16:9 for YouTube".to_string(),
            resolution: Some((1920, 1080)),
            fps: Some(60),
            codec: "h264".to_string(),
            bitrate: Some("12M".to_string()),
            crop: None,
            loudnorm: true,
        }
    }

    pub fn trailer() -> Self {
        Self {
            name: "Trailer Cut".to_string(),
            description: "High quality with intro/outro card slots".to_string(),
            resolution: Some((1920, 1080)),
            fps: Some(60),
            codec: "h264".to_string(),
            bitrate: Some("15M".to_string()),
            crop: None,
            loudnorm: true,
        }
    }

    pub fn high_quality() -> Self {
        Self {
            name: "High Quality".to_string(),
            description: "Source resolution and FPS, high bitrate".to_string(),
            resolution: None,
            fps: None,
            codec: "h264".to_string(),
            bitrate: Some("20M".to_string()),
            crop: None,
            loudnorm: false,
        }
    }

    pub fn all_presets() -> Vec<Self> {
        vec![
            Self::shorts(),
            Self::youtube(),
            Self::trailer(),
            Self::high_quality(),
        ]
    }
}
