use clipforge_core::config::Config;
use clipforge_core::encode::hw_probe::EncoderInfo;
use clipforge_core::library::Library;
use clipforge_core::process::FfmpegProcess;
use clipforge_core::replay::ReplayRing;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub encoders: Arc<RwLock<Vec<EncoderInfo>>>,
    pub recorder: Arc<Mutex<RecorderState>>,
    pub replay: Arc<Mutex<ReplayState>>,
    pub library: Arc<Mutex<Option<Library>>>,
}

pub struct RecorderState {
    pub process: Option<FfmpegProcess>,
    pub status: RecordingStatus,
    pub output_path: Option<PathBuf>,
    pub start_time: Option<std::time::Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RecordingStatus {
    Idle,
    Starting,
    Recording,
    Stopping,
}

pub struct ReplayState {
    pub process: Option<FfmpegProcess>,
    pub ring: Option<ReplayRing>,
    pub active: bool,
}

impl Default for RecorderState {
    fn default() -> Self {
        Self {
            process: None,
            status: RecordingStatus::Idle,
            output_path: None,
            start_time: None,
        }
    }
}

impl Default for ReplayState {
    fn default() -> Self {
        Self {
            process: None,
            ring: None,
            active: false,
        }
    }
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            encoders: Arc::new(RwLock::new(Vec::new())),
            recorder: Arc::new(Mutex::new(RecorderState::default())),
            replay: Arc::new(Mutex::new(ReplayState::default())),
            library: Arc::new(Mutex::new(None)),
        }
    }
}
