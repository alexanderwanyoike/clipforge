use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub resolution: Option<(u32, u32)>,
    pub fps: Option<u32>,
    pub codec: String,
    pub bitrate: Option<String>,
    pub crop_aspect: Option<(u32, u32)>,
    pub loudnorm: bool,
    pub container: String,
}

impl ExportPreset {
    pub fn shorts() -> Self {
        Self {
            id: "shorts".to_string(),
            name: "TikTok / Shorts".to_string(),
            description: "Vertical 9:16 for TikTok, YouTube Shorts, Reels".to_string(),
            resolution: Some((1080, 1920)),
            fps: Some(60),
            codec: "libx264".to_string(),
            bitrate: Some("8M".to_string()),
            crop_aspect: Some((9, 16)),
            loudnorm: true,
            container: "mp4".to_string(),
        }
    }

    pub fn youtube() -> Self {
        Self {
            id: "youtube".to_string(),
            name: "YouTube 16:9".to_string(),
            description: "Standard 16:9 for YouTube".to_string(),
            resolution: Some((1920, 1080)),
            fps: Some(60),
            codec: "libx264".to_string(),
            bitrate: Some("12M".to_string()),
            crop_aspect: None,
            loudnorm: true,
            container: "mp4".to_string(),
        }
    }

    pub fn trailer() -> Self {
        Self {
            id: "trailer".to_string(),
            name: "Trailer Cut".to_string(),
            description: "High quality with intro/outro card slots".to_string(),
            resolution: Some((1920, 1080)),
            fps: Some(60),
            codec: "libx264".to_string(),
            bitrate: Some("15M".to_string()),
            crop_aspect: None,
            loudnorm: true,
            container: "mp4".to_string(),
        }
    }

    pub fn high_quality() -> Self {
        Self {
            id: "high_quality".to_string(),
            name: "High Quality".to_string(),
            description: "Source resolution and FPS, high bitrate VBR".to_string(),
            resolution: None,
            fps: None,
            codec: "libx264".to_string(),
            bitrate: Some("20M".to_string()),
            crop_aspect: None,
            loudnorm: false,
            container: "mp4".to_string(),
        }
    }

    pub fn all() -> Vec<Self> {
        vec![Self::shorts(), Self::youtube(), Self::trailer(), Self::high_quality()]
    }
}
