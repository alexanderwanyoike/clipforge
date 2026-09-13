use clipforge_core::config::Config;
use clipforge_core::encode::hw_probe::EncoderInfo;
use clipforge_core::library::Library;
use clipforge_core::recording::{linux::LinuxRecordingBackend, RecordingService};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub type Recorder = RecordingService<LinuxRecordingBackend>;

pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub encoders: Arc<RwLock<Vec<EncoderInfo>>>,
    pub recorder: Recorder,
    pub replay: Recorder,
    pub library: Arc<Mutex<Option<Library>>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            encoders: Arc::new(RwLock::new(Vec::new())),
            recorder: Recorder::new(LinuxRecordingBackend),
            replay: Recorder::new(LinuxRecordingBackend),
            library: Arc::new(Mutex::new(None)),
        }
    }
}
