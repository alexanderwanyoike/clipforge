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

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct FfmpegProgress {
    pub frame: u64,
    pub fps: f64,
    pub time: String,
    pub speed: String,
    pub size_kb: u64,
}

pub struct FfmpegProcess {
    child: Child,
    state_tx: watch::Sender<ProcessState>,
    state_rx: watch::Receiver<ProcessState>,
    progress_tx: watch::Sender<FfmpegProgress>,
    progress_rx: watch::Receiver<FfmpegProgress>,
}

impl FfmpegProcess {
    pub async fn spawn(args: Vec<String>) -> Result<Self> {
        info!(args = ?args, "spawning ffmpeg");

        let mut child = Command::new("ffmpeg")
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
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
        let stderr = child.stderr.take().expect("stderr was piped");
        let progress_tx_clone = progress_tx.clone();
        let state_tx_clone = state_tx.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            let mut saw_output = false;

            while let Ok(Some(line)) = lines.next_line().await {
                debug!(line = %line, "ffmpeg stderr");

                if !saw_output && (line.contains("Output #0") || line.contains("frame=")) {
                    saw_output = true;
                    let _ = state_tx_clone.send(ProcessState::Running);
                }

                if let Some(progress) = parse_progress(&line) {
                    let _ = progress_tx_clone.send(progress);
                }

                if line.contains("Exiting normally") {
                    debug!("ffmpeg exiting normally");
                }
            }
        });

        // Mark as starting (will transition to Running once output is detected)
        let _ = state_tx.send(ProcessState::Starting);

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
        match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            self.child.wait(),
        )
        .await
        {
            Ok(Ok(status)) => {
                let new_state = if status.success() {
                    ProcessState::Stopped
                } else {
                    // FFmpeg often exits with code 255 on 'q' which is fine
                    ProcessState::Stopped
                };
                let _ = self.state_tx.send(new_state);
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

fn parse_progress(line: &str) -> Option<FfmpegProgress> {
    // FFmpeg progress lines look like:
    // frame=  123 fps= 60 q=20.0 size=    1234kB time=00:00:02.05 speed=1.00x
    if !line.contains("frame=") || !line.contains("time=") {
        return None;
    }

    let mut progress = FfmpegProgress::default();

    for part in line.split_whitespace() {
        if let Some(val) = part.strip_prefix("frame=") {
            progress.frame = val.parse().unwrap_or(0);
        } else if let Some(val) = part.strip_prefix("fps=") {
            progress.fps = val.parse().unwrap_or(0.0);
        } else if let Some(val) = part.strip_prefix("time=") {
            progress.time = val.to_string();
        } else if let Some(val) = part.strip_prefix("speed=") {
            progress.speed = val.to_string();
        } else if let Some(val) = part.strip_prefix("size=") {
            let val = val.trim_end_matches("kB").trim_end_matches("KiB");
            progress.size_kb = val.trim().parse().unwrap_or(0);
        }
    }

    // Also handle "key=  value" with separate whitespace
    if progress.frame == 0 {
        if let Some(idx) = line.find("frame=") {
            let rest = &line[idx + 6..];
            let val: String = rest.chars().take_while(|c| c.is_ascii_digit() || c.is_whitespace()).collect();
            progress.frame = val.trim().parse().unwrap_or(0);
        }
    }

    if progress.time.is_empty() {
        if let Some(idx) = line.find("time=") {
            let rest = &line[idx + 5..];
            let val: String = rest.chars().take_while(|c| *c != ' ').collect();
            progress.time = val.trim().to_string();
        }
    }

    if progress.speed.is_empty() {
        if let Some(idx) = line.find("speed=") {
            let rest = &line[idx + 6..];
            let val: String = rest.chars().take_while(|c| !c.is_whitespace()).collect();
            progress.speed = val.trim().to_string();
        }
    }

    Some(progress)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_progress_standard_line() {
        let line = "frame=  123 fps= 60.0 q=20.0 size=    1234kB time=00:00:02.05 speed=1.00x";
        let progress = parse_progress(line).expect("should parse");
        assert_eq!(progress.frame, 123);
        assert_eq!(progress.time, "00:00:02.05");
        assert_eq!(progress.speed, "1.00x");
    }

    #[test]
    fn parse_progress_non_progress_line_returns_none() {
        assert!(parse_progress("Input #0, matroska,webm").is_none());
        assert!(parse_progress("Stream #0:0: Video").is_none());
        assert!(parse_progress("").is_none());
    }

    #[test]
    fn parse_progress_compact_format() {
        let line = "frame=500 fps=60 q=20.0 size=5000kB time=00:00:08.33 speed=1.02x";
        let progress = parse_progress(line).expect("should parse");
        assert_eq!(progress.frame, 500);
        assert!((progress.fps - 60.0).abs() < 0.1);
        assert_eq!(progress.time, "00:00:08.33");
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
