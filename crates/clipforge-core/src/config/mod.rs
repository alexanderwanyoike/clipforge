use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub recording: RecordingConfig,
    pub replay: ReplayConfig,
    pub export: ExportConfig,
    pub hotkeys: HotkeyConfig,
    pub paths: PathConfig,
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingConfig {
    pub fps: u32,
    pub encoder: EncoderPreference,
    pub quality: Quality,
    pub audio_enabled: bool,
    pub audio_source: String,
    pub container: String,
    pub capture_mode: CaptureMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EncoderPreference {
    Auto,
    Specific(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Quality {
    Low,
    Medium,
    High,
    Lossless,
    Custom { qp: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureMode {
    Fullscreen,
    Window { id: Option<String> },
    Region { x: u32, y: u32, w: u32, h: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayConfig {
    pub enabled: bool,
    pub duration_secs: u32,
    pub segment_secs: u32,
    pub max_segments: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    pub default_preset: String,
    pub output_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub toggle_recording: String,
    pub save_replay: String,
    pub toggle_replay_buffer: String,
    pub mark_highlight: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathConfig {
    pub recordings_dir: PathBuf,
    pub replays_dir: PathBuf,
    pub replay_cache_dir: PathBuf,
    pub thumbnails_dir: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub show_notifications: bool,
}

impl Default for Config {
    fn default() -> Self {
        // Use ~/Videos/ClipForge/ as the base for user-facing files
        let base_dir = directories::UserDirs::new()
            .and_then(|u| u.video_dir().map(|v| v.to_path_buf()))
            .unwrap_or_else(|| {
                directories::UserDirs::new()
                    .map(|u| u.home_dir().join("Videos"))
                    .unwrap_or_else(|| PathBuf::from("~/Videos"))
            })
            .join("ClipForge");

        // Thumbnails go in ~/.cache/clipforge (not user-facing)
        let cache_dir = directories::ProjectDirs::from("com", "clipforge", "ClipForge")
            .map(|d| d.cache_dir().to_path_buf())
            .unwrap_or_else(|| {
                directories::UserDirs::new()
                    .map(|u| u.home_dir().join(".cache").join("clipforge"))
                    .unwrap_or_else(|| PathBuf::from("/tmp/clipforge/cache"))
            });

        // Prefer /dev/shm for replay cache if available and has space
        let replay_cache_dir = if Path::new("/dev/shm").exists() {
            PathBuf::from("/dev/shm/clipforge-replay")
        } else {
            cache_dir.join("replay")
        };

        let recordings_dir = base_dir.join("recordings");
        let replays_dir = base_dir.join("replays");
        let thumbnails_dir = cache_dir.join("thumbnails");
        let exports_dir = base_dir.join("exports");

        Self {
            recording: RecordingConfig {
                fps: 60,
                encoder: EncoderPreference::Auto,
                quality: Quality::High,
                audio_enabled: true,
                audio_source: "default".to_string(),
                container: "mkv".to_string(),
                capture_mode: CaptureMode::Fullscreen,
            },
            replay: ReplayConfig {
                enabled: false,
                duration_secs: 120,
                segment_secs: 3,
                max_segments: 40,
            },
            export: ExportConfig {
                default_preset: "high_quality".to_string(),
                output_dir: exports_dir,
            },
            hotkeys: HotkeyConfig {
                toggle_recording: "Ctrl+Alt+R".to_string(),
                save_replay: "Ctrl+Alt+S".to_string(),
                toggle_replay_buffer: "Ctrl+Alt+B".to_string(),
                mark_highlight: "Ctrl+Alt+H".to_string(),
            },
            paths: PathConfig {
                recordings_dir,
                replays_dir,
                replay_cache_dir,
                thumbnails_dir,
            },
            ui: UiConfig {
                theme: "dark".to_string(),
                minimize_to_tray: true,
                start_minimized: false,
                show_notifications: true,
            },
        }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("com", "clipforge", "ClipForge")
            .ok_or_else(|| Error::Config("cannot determine config directory".into()))?;
        Ok(dirs.config_dir().join("config.json"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = std::fs::read_to_string(&path).map_err(Error::Io)?;
        let config: Self = serde_json::from_str(&content).map_err(Error::Json)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        let content = serde_json::to_string_pretty(self).map_err(Error::Json)?;
        std::fs::write(&path, content).map_err(Error::Io)?;
        Ok(())
    }

    /// Ensure all configured directories exist
    pub fn ensure_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.paths.recordings_dir).map_err(Error::Io)?;
        std::fs::create_dir_all(&self.paths.replays_dir).map_err(Error::Io)?;
        std::fs::create_dir_all(&self.paths.replay_cache_dir).map_err(Error::Io)?;
        std::fs::create_dir_all(&self.paths.thumbnails_dir).map_err(Error::Io)?;
        std::fs::create_dir_all(&self.export.output_dir).map_err(Error::Io)?;
        Ok(())
    }
}
