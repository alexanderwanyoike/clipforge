use crate::error::{Error, Result};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::watch;
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ProcessState {
    Starting,
    Running,
    Stopping,
    Stopped,
    Failed,
}

pub mod progress;
pub use progress::FfmpegProgress;
use progress::ProgressDecoder;

pub struct FfmpegProcess {
    child: Child,
    state_tx: watch::Sender<ProcessState>,
    state_rx: watch::Receiver<ProcessState>,
    #[allow(dead_code)]
    progress_tx: watch::Sender<FfmpegProgress>,
    progress_rx: watch::Receiver<FfmpegProgress>,
}

impl FfmpegProcess {
    pub async fn spawn(args: Vec<String>) -> Result<Self> {
        info!(args = ?args, "spawning ffmpeg");

        let mut child = Command::new("ffmpeg")
            .args(["-nostats", "-progress", "pipe:2", "-stats_period", "0.25"])
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Error::FfmpegNotFound
                } else {
                    Error::Io(e)
                }
            })?;

        let (state_tx, state_rx) = watch::channel(ProcessState::Starting);
        let (progress_tx, progress_rx) = watch::channel(FfmpegProgress::default());

        // Spawn stderr reader for progress parsing
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::Other("FFmpeg stderr pipe was not created".into()))?;
        let progress_tx_clone = progress_tx.clone();
        let state_tx_clone = state_tx.clone();

        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            let mut decoder = ProgressDecoder::default();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        debug!(%line, "ffmpeg stderr");
                        if let Some(progress) = decoder.accept(&line) {
                            if progress.frame > 0
                                && *state_tx_clone.borrow() == ProcessState::Starting
                            {
                                let _ = state_tx_clone.send(ProcessState::Running);
                            }
                            let _ = progress_tx_clone.send(progress);
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        warn!(%error, "cannot read FFmpeg progress");
                        let _ = state_tx_clone.send(ProcessState::Failed);
                        break;
                    }
                }
            }
        });

        Ok(Self {
            child,
            state_tx,
            state_rx,
            progress_tx,
            progress_rx,
        })
    }

    /// Sends 'q' to stdin for graceful FFmpeg shutdown
    pub async fn stop_graceful(&mut self) -> Result<()> {
        let _ = self.state_tx.send(ProcessState::Stopping);

        if let Some(stdin) = self.child.stdin.as_mut() {
            if let Err(e) = stdin.write_all(b"q").await {
                warn!(error = %e, "failed to write 'q' to ffmpeg stdin, force killing");
                return self.kill().await;
            }
        }

        // Wait for process to exit with timeout
        match tokio::time::timeout(std::time::Duration::from_secs(10), self.child.wait()).await {
            Ok(Ok(status)) => {
                if !status.success() && status.code() != Some(255) {
                    let _ = self.state_tx.send(ProcessState::Failed);
                    return Err(Error::FfmpegFailed(format!("FFmpeg failed ({status})")));
                }
                let _ = self.state_tx.send(ProcessState::Stopped);
                info!(status = ?status, "ffmpeg stopped");
                Ok(())
            }
            Ok(Err(e)) => {
                let _ = self.state_tx.send(ProcessState::Failed);
                Err(Error::Io(e))
            }
            Err(_) => {
                warn!("ffmpeg didn't exit within 10s, force killing");
                self.kill().await
            }
        }
    }

    /// Force kill the FFmpeg process
    pub async fn kill(&mut self) -> Result<()> {
        let _ = self.state_tx.send(ProcessState::Stopping);
        self.child.kill().await.map_err(Error::Io)?;
        let _ = self.child.wait().await;
        let _ = self.state_tx.send(ProcessState::Stopped);
        info!("ffmpeg force killed");
        Ok(())
    }

    pub fn state(&self) -> ProcessState {
        *self.state_rx.borrow()
    }

    pub fn subscribe_state(&self) -> watch::Receiver<ProcessState> {
        self.state_rx.clone()
    }

    pub fn progress(&self) -> FfmpegProgress {
        self.progress_rx.borrow().clone()
    }

    pub fn subscribe_progress(&self) -> watch::Receiver<FfmpegProgress> {
        self.progress_rx.clone()
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.id()
    }
}

/// Run FFmpeg with args and wait for completion. Returns stderr output.
pub async fn run_ffmpeg(args: &[&str]) -> Result<String> {
    let output = Command::new("ffmpeg")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::FfmpegNotFound
            } else {
                Error::Io(e)
            }
        })?;

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok(stderr)
    } else {
        Err(Error::FfmpegFailed(stderr))
    }
}

/// Run ffprobe and return stdout
pub async fn run_ffprobe(args: &[&str]) -> Result<String> {
    let output = Command::new("ffprobe")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(Error::Io)?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(Error::FfmpegFailed(stderr))
    }
}
