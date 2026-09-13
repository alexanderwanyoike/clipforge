//! Recording application service and its domain-facing ports.
//! The service owns lifecycle policy; adapters own operating-system resources.
pub mod linux;

use crate::{
    config::Config,
    encode::hw_probe::EncoderInfo,
    error::{Error, Result},
};
use std::{
    future::Future,
    path::PathBuf,
    sync::{Arc, Weak},
    time::Instant,
};
use tokio::sync::{oneshot, watch, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum RecordingStatus {
    Idle,
    Starting,
    Recording,
    Stopping,
}

#[derive(Debug, Clone, serde::Serialize)]
pub enum RecordingDestination {
    File(PathBuf),
    Replay {
        directory: PathBuf,
        segment_secs: u32,
        max_segments: u32,
    },
}

impl RecordingDestination {
    pub fn replay(config: &Config) -> Self {
        Self::Replay {
            directory: config.paths.replay_cache_dir.clone(),
            segment_secs: config.replay.segment_secs,
            max_segments: config.replay.max_segments,
        }
    }

    pub fn file_path(&self) -> Option<String> {
        match self {
            Self::File(path) => Some(path.to_string_lossy().into_owned()),
            Self::Replay { .. } => None,
        }
    }
}

#[derive(Clone)]
pub struct RecordingRequest {
    pub config: Config,
    pub encoder: EncoderInfo,
    pub destination: RecordingDestination,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RecordingSnapshot {
    pub status: RecordingStatus,
    pub elapsed_secs: u64,
    pub file_path: Option<String>,
    pub destination: Option<RecordingDestination>,
    pub error: Option<String>,
}

impl Default for RecordingSnapshot {
    fn default() -> Self {
        Self {
            status: RecordingStatus::Idle,
            elapsed_secs: 0,
            file_path: None,
            destination: None,
            error: None,
        }
    }
}

/// Implementations may use X11, a portal, or a fake backend. The lifecycle
/// service has no knowledge of PipeWire, FFmpeg, child processes or Tauri.
pub trait RecordingBackend: Clone + Send + Sync + 'static {
    type Session: ActiveRecording;
    fn start(
        &self,
        request: RecordingRequest,
    ) -> impl Future<Output = Result<Self::Session>> + Send;
}

/// wait_failure must be cancellation-safe: the service also waits for stop/timer events.
pub trait ActiveRecording: Send + 'static {
    fn wait_failure(&mut self) -> impl Future<Output = Error> + Send;
    fn stop(&mut self) -> impl Future<Output = Result<()>> + Send;
}

type Completion = std::result::Result<(), String>;
struct SessionControl {
    stop: watch::Sender<bool>,
    completion: watch::Receiver<Option<Completion>>,
}
struct Controller {
    active: Mutex<Option<SessionControl>>,
    state: watch::Sender<RecordingSnapshot>,
}

/// One recording at a time. A worker owns the running session; the mutex only
/// reserves/releases that slot and is never held during external I/O or a picker.
#[derive(Clone)]
pub struct RecordingService<B: RecordingBackend> {
    backend: B,
    controller: Arc<Controller>,
}

impl<B: RecordingBackend> RecordingService<B> {
    pub fn new(backend: B) -> Self {
        let (state, _) = watch::channel(RecordingSnapshot::default());
        Self {
            backend,
            controller: Arc::new(Controller {
                active: Mutex::new(None),
                state,
            }),
        }
    }

    pub fn snapshot(&self) -> RecordingSnapshot {
        self.controller.state.borrow().clone()
    }
    pub fn subscribe(&self) -> watch::Receiver<RecordingSnapshot> {
        self.controller.state.subscribe()
    }

    pub async fn start(&self, request: RecordingRequest) -> Result<()> {
        let mut active = self.controller.active.lock().await;
        if active.is_some() {
            return Err(Error::AlreadyRecording);
        }
        let (stop, stop_rx) = watch::channel(false);
        let (completed, completion) = watch::channel(None);
        let (started, ready) = oneshot::channel();
        *active = Some(SessionControl { stop, completion });
        let snapshot = RecordingSnapshot {
            status: RecordingStatus::Starting,
            file_path: request.destination.file_path(),
            destination: Some(request.destination.clone()),
            ..Default::default()
        };
        self.controller.state.send_replace(snapshot.clone());
        let worker = Worker {
            controller: Arc::downgrade(&self.controller),
            state: self.controller.state.clone(),
            stop: stop_rx,
            completed,
            snapshot,
        };
        let backend = self.backend.clone();
        tokio::spawn(async move {
            worker.run(backend, request, started).await;
        });
        drop(active);
        ready
            .await
            .map_err(|_| Error::Other("Recording startup task ended unexpectedly".into()))?
            .map_err(Error::Other)
    }

    pub async fn stop(&self) -> Result<()> {
        let mut completion = {
            let active = self.controller.active.lock().await;
            let control = active.as_ref().ok_or(Error::NotRecording)?;
            control.stop.send_replace(true);
            control.completion.clone()
        };
        wait_completion(&mut completion).await
    }

    pub async fn wait_finished(&self) -> Result<()> {
        let completion = {
            let active = self.controller.active.lock().await;
            active.as_ref().map(|control| control.completion.clone())
        };
        match completion {
            Some(mut completion) => wait_completion(&mut completion).await,
            None => match self.snapshot().error {
                Some(error) => Err(Error::Other(error)),
                None => Ok(()),
            },
        }
    }
}

async fn wait_completion(completion: &mut watch::Receiver<Option<Completion>>) -> Result<()> {
    loop {
        if let Some(result) = completion.borrow_and_update().clone() {
            return result.map_err(Error::Other);
        }
        completion.changed().await.map_err(|_| {
            Error::Other("Recording worker ended without a completion result".into())
        })?;
    }
}

struct Worker {
    controller: Weak<Controller>,
    state: watch::Sender<RecordingSnapshot>,
    stop: watch::Receiver<bool>,
    completed: watch::Sender<Option<Completion>>,
    snapshot: RecordingSnapshot,
}

impl Worker {
    async fn run<B: RecordingBackend>(
        mut self,
        backend: B,
        request: RecordingRequest,
        started: oneshot::Sender<Completion>,
    ) {
        let startup = tokio::select! {
            biased;
            _ = self.stop.changed() => Err(Error::Other("Recording startup cancelled".into())),
            result = backend.start(request) => result,
        };
        let result = match startup {
            Ok(mut session) => {
                self.publish(RecordingStatus::Recording);
                let _ = started.send(Ok(()));
                let outcome = self.record(&mut session).await;
                self.publish(RecordingStatus::Stopping);
                let stopped = session.stop().await.map_err(|error| error.to_string());
                outcome.and(stopped)
            }
            Err(error) => {
                let result = Err(error.to_string());
                self.finish(result.clone()).await;
                let _ = started.send(result);
                return;
            }
        };
        self.finish(result).await;
    }

    async fn record(&mut self, session: &mut impl ActiveRecording) -> Completion {
        let start = Instant::now();
        let mut timer = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            tokio::select! {
                biased;
                _ = self.stop.changed() => return Ok(()),
                error = session.wait_failure() => return Err(error.to_string()),
                _ = timer.tick() => {
                    self.snapshot.elapsed_secs = start.elapsed().as_secs();
                    self.state.send_replace(self.snapshot.clone());
                }
            }
        }
    }

    fn publish(&mut self, status: RecordingStatus) {
        self.snapshot.status = status;
        self.state.send_replace(self.snapshot.clone());
    }

    async fn finish(&mut self, result: Completion) {
        self.snapshot.error = result.as_ref().err().cloned();
        // Serialize releasing the slot and publishing Idle against a new start.
        if let Some(controller) = self.controller.upgrade() {
            let mut active = controller.active.lock().await;
            self.publish(RecordingStatus::Idle);
            *active = None;
        } else {
            self.publish(RecordingStatus::Idle);
        }
        self.completed.send_replace(Some(result));
    }
}

#[cfg(test)]
mod tests;
