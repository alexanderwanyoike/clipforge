use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Clone)]
struct FakeBackend {
    gate: watch::Receiver<bool>,
    failure: watch::Receiver<bool>,
    fail_start: Arc<AtomicBool>,
    stopped: Arc<AtomicUsize>,
    dropped: Arc<AtomicUsize>,
}
struct FakeSession {
    backend: FakeBackend,
}
impl RecordingBackend for FakeBackend {
    type Session = FakeSession;
    async fn start(&self, _: RecordingRequest) -> Result<FakeSession> {
        let mut gate = self.gate.clone();
        gate.wait_for(|ready| *ready)
            .await
            .map_err(|_| Error::Other("gate closed".into()))?;
        if self.fail_start.swap(false, Ordering::SeqCst) {
            return Err(Error::Other("backend failed".into()));
        }
        Ok(FakeSession {
            backend: self.clone(),
        })
    }
}
impl ActiveRecording for FakeSession {
    async fn wait_failure(&mut self) -> Error {
        let _ = self.backend.failure.wait_for(|failed| *failed).await;
        Error::Other("source disconnected".into())
    }
    async fn stop(&mut self) -> Result<()> {
        self.backend.stopped.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
impl Drop for FakeSession {
    fn drop(&mut self) {
        self.backend.dropped.fetch_add(1, Ordering::SeqCst);
    }
}
fn fixture(
    ready: bool,
) -> (
    RecordingService<FakeBackend>,
    watch::Sender<bool>,
    watch::Sender<bool>,
) {
    let (gate_tx, gate) = watch::channel(ready);
    let (failure_tx, failure) = watch::channel(false);
    let backend = FakeBackend {
        gate,
        failure,
        fail_start: Arc::new(AtomicBool::new(false)),
        stopped: Arc::new(AtomicUsize::new(0)),
        dropped: Arc::new(AtomicUsize::new(0)),
    };
    (RecordingService::new(backend), gate_tx, failure_tx)
}
fn request() -> RecordingRequest {
    RecordingRequest {
        config: Config::default(),
        encoder: EncoderInfo {
            name: "test".into(),
            device: None,
            available: true,
            hw_accel: crate::encode::hw_probe::HwAccelType::Software,
        },
        destination: RecordingDestination::File("test.mkv".into()),
    }
}

#[tokio::test]
async fn duplicate_start_is_rejected_and_stop_releases_resources_once() {
    let (service, _gate, _failure) = fixture(true);
    service.start(request()).await.unwrap();
    assert!(matches!(
        service.start(request()).await,
        Err(Error::AlreadyRecording)
    ));
    let (first, second) = tokio::join!(service.stop(), service.stop());
    first.unwrap();
    second.unwrap();
    assert_eq!(service.snapshot().status, RecordingStatus::Idle);
    assert_eq!(service.backend.stopped.load(Ordering::SeqCst), 1);
    assert_eq!(service.backend.dropped.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn picker_wait_does_not_block_status_or_cancellation() {
    let (service, _gate, _failure) = fixture(false);
    let other = service.clone();
    let start = tokio::spawn(async move { other.start(request()).await });
    service
        .subscribe()
        .wait_for(|state| state.status == RecordingStatus::Starting)
        .await
        .unwrap();
    assert_eq!(service.snapshot().status, RecordingStatus::Starting);
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), service.stop())
            .await
            .unwrap()
            .is_err()
    );
    assert!(start.await.unwrap().is_err());
    assert_eq!(service.snapshot().status, RecordingStatus::Idle);
}

#[tokio::test]
async fn startup_failure_can_be_retried_immediately() {
    let (service, _gate, _failure) = fixture(true);
    service.backend.fail_start.store(true, Ordering::SeqCst);
    assert!(service.start(request()).await.is_err());
    assert_eq!(service.snapshot().status, RecordingStatus::Idle);
    service.start(request()).await.unwrap();
    assert!(service.snapshot().error.is_none());
    service.stop().await.unwrap();
}

#[tokio::test]
async fn disconnect_uses_the_same_cleanup_as_user_stop() {
    let (service, _gate, failure) = fixture(true);
    service.start(request()).await.unwrap();
    failure.send_replace(true);
    assert!(service.wait_finished().await.is_err());
    assert_eq!(service.snapshot().status, RecordingStatus::Idle);
    assert_eq!(service.backend.stopped.load(Ordering::SeqCst), 1);
    assert_eq!(
        service.snapshot().error.as_deref(),
        Some("source disconnected")
    );
}

#[tokio::test]
async fn dropping_last_service_handle_stops_active_recording() {
    let (service, _gate, _failure) = fixture(true);
    let backend = service.backend.clone();
    service.start(request()).await.unwrap();
    let mut state = service.subscribe();
    drop(service);
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        state.wait_for(|s| s.status == RecordingStatus::Idle),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(backend.stopped.load(Ordering::SeqCst), 1);
    assert_eq!(backend.dropped.load(Ordering::SeqCst), 1);
}
