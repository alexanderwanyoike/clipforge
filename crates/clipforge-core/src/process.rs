//! FFmpeg infrastructure: process I/O, progress and exit status.
//! Recording policy and capture resources belong to the recording service.
mod command;
pub(crate) mod diagnostics;
pub mod progress;

use crate::error::{Error, Result};
pub(crate) use command::{capture_command, host_command};
use diagnostics::Diagnostics;
pub use progress::FfmpegProgress;
use progress::ProgressDecoder;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Child,
    sync::watch,
    task::JoinHandle,
};

pub enum EncoderInput {
    /// FFmpeg opens its input itself; stdin remains available for commands.
    Device,
    /// Video occupies stdin, so stopping must use a signal instead of `q`.
    Video(Stdio),
}

pub struct FfmpegProcess {
    child: Child,
    video_stdin: bool,
    diagnostics: Diagnostics,
    progress: watch::Receiver<FfmpegProgress>,
    reader_error: watch::Receiver<Option<String>>,
    reader: JoinHandle<()>,
}

impl FfmpegProcess {
    pub async fn spawn(args: Vec<String>, input: EncoderInput) -> Result<Self> {
        let (stdin, video_stdin) = match input {
            EncoderInput::Device => (Stdio::piped(), false),
            EncoderInput::Video(stream) => (stream, true),
        };
        let mut child = capture_command("ffmpeg")
            .args([
                "-hide_banner",
                "-nostats",
                "-progress",
                "pipe:2",
                "-stats_period",
                "0.25",
            ])
            .args(args)
            .stdin(stdin)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => Error::FfmpegNotFound,
                _ => Error::Io(error),
            })?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::Other("FFmpeg stderr pipe was not created".into()))?;
        let diagnostics = Diagnostics::default();
        let (progress_tx, progress) = watch::channel(FfmpegProgress::default());
        let (error_tx, reader_error) = watch::channel(None);
        let reader = tokio::spawn(read_progress(
            BufReader::new(stderr),
            diagnostics.clone(),
            progress_tx,
            error_tx,
        ));
        Ok(Self {
            child,
            video_stdin,
            diagnostics,
            progress,
            reader_error,
            reader,
        })
    }

    pub async fn wait_for_frame(&mut self, timeout: Duration) -> Result<()> {
        let result = tokio::time::timeout(timeout, async {
            loop {
                if self.child.try_wait()?.is_some() {
                    return Err(self.failure("FFmpeg exited before capture started"));
                }
                if self.progress.borrow().frame > 0 {
                    return Ok(());
                }
                if let Some(error) = self.reader_error.borrow().clone() {
                    return Err(self.failure(&error));
                }
                if self.progress.changed().await.is_err() {
                    return Err(self.failure("FFmpeg produced no video frames"));
                }
            }
        })
        .await;
        match result {
            Ok(result) => result,
            Err(_) => Err(self.failure("Timed out waiting for an encoded video frame")),
        }
    }

    /// A recording input is continuous. Any unsolicited exit is a failure;
    /// finite exports use run_ffmpeg and its actual completion status instead.
    pub async fn wait_failure(&mut self) -> Error {
        let context = tokio::select! {
            result = self.child.wait() => match result {
                Ok(status) => format!("FFmpeg exited unexpectedly ({status})"),
                Err(error) => format!("Could not wait for FFmpeg: {error}"),
            },
            result = self.reader_error.wait_for(Option::is_some) => match result {
                Ok(error) => error.clone().unwrap_or_default(),
                Err(_) => "FFmpeg progress stream closed".into(),
            },
        };
        self.failure(&context)
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(status) = self.child.try_wait()? {
            return self.check_exit(status);
        }
        if self.video_stdin {
            if let Some(pid) = self.child.id() {
                // SAFETY: owned, unreaped child; SIGINT asks FFmpeg to flush its trailer.
                if unsafe { libc::kill(pid as libc::pid_t, libc::SIGINT) } != 0 {
                    return Err(Error::Io(std::io::Error::last_os_error()));
                }
            }
        } else if let Some(stdin) = self.child.stdin.as_mut() {
            stdin.write_all(b"q").await?;
        }
        match tokio::time::timeout(Duration::from_secs(10), self.child.wait()).await {
            Ok(status) => self.check_exit(status?),
            Err(_) => {
                let _ = self.child.kill().await;
                Err(self.failure("FFmpeg did not stop in time; the output may be incomplete"))
            }
        }
    }

    fn check_exit(&self, status: std::process::ExitStatus) -> Result<()> {
        if status.success() || status.code() == Some(255) {
            Ok(())
        } else {
            Err(self.failure(&format!("FFmpeg failed ({status})")))
        }
    }

    fn failure(&self, context: &str) -> Error {
        Error::FfmpegFailed(format!("{context}\n{}", self.diagnostics.detail()))
    }
}

impl Drop for FfmpegProcess {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

async fn read_progress(
    reader: impl AsyncBufRead + Unpin,
    diagnostics: Diagnostics,
    progress: watch::Sender<FfmpegProgress>,
    errors: watch::Sender<Option<String>>,
) {
    let mut lines = reader.lines();
    let mut decoder = ProgressDecoder::default();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                diagnostics.push(&line);
                if let Some(snapshot) = decoder.accept(&line) {
                    progress.send_replace(snapshot);
                }
            }
            Ok(None) => break,
            Err(error) => {
                let message = format!("Cannot read FFmpeg progress: {error}");
                diagnostics.push(&message);
                errors.send_replace(Some(message));
                break;
            }
        }
    }
}
/// Run FFmpeg with args and wait for completion. Returns stderr output.
pub async fn run_ffmpeg(args: &[&str]) -> Result<String> {
    let output = host_command("ffmpeg")
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
    let output = host_command("ffprobe")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn read_errors_are_reported_without_panicking() {
        let (progress, _) = watch::channel(FfmpegProgress::default());
        let (errors, error) = watch::channel(None);
        let diagnostics = Diagnostics::default();
        read_progress(
            BufReader::new(&b"\xff\n"[..]),
            diagnostics.clone(),
            progress,
            errors,
        )
        .await;
        assert!(error
            .borrow()
            .as_ref()
            .is_some_and(|message| message.contains("Cannot read FFmpeg progress")));
        assert!(diagnostics.detail().contains("Cannot read FFmpeg progress"));
    }

    #[tokio::test]
    async fn reader_publishes_records_and_handles_eof() {
        let (progress, result) = watch::channel(FfmpegProgress::default());
        let (errors, error) = watch::channel(None);
        let stream = b"frame=3\nfps=N/A\nprogress=continue\nframe=bad\nprogress=end\n";
        read_progress(
            BufReader::new(&stream[..]),
            Diagnostics::default(),
            progress,
            errors,
        )
        .await;
        assert_eq!(result.borrow().frame, 3);
        assert!(error.borrow().is_none());
    }
    #[tokio::test]
    #[ignore = "requires FFmpeg with libx264"]
    async fn process_media_device_input_flushes_on_stop() {
        let temp = tempfile::tempdir().unwrap();
        let output = temp.path().join("device.mkv");
        let args = [
            "-re",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=64x64:rate=30",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            output.to_str().unwrap(),
        ]
        .into_iter()
        .map(str::to_owned)
        .collect();
        let mut process = FfmpegProcess::spawn(args, EncoderInput::Device)
            .await
            .unwrap();
        process
            .wait_for_frame(Duration::from_secs(10))
            .await
            .unwrap();
        process.stop().await.unwrap();
        let metadata = run_ffprobe(&[
            "-v",
            "error",
            "-count_frames",
            "-show_entries",
            "stream=nb_read_frames",
            "-of",
            "json",
            output.to_str().unwrap(),
        ])
        .await
        .unwrap();
        let metadata: serde_json::Value = serde_json::from_str(&metadata).unwrap();
        assert!(
            metadata["streams"][0]["nb_read_frames"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap()
                > 0
        );
    }

    #[tokio::test]
    #[ignore = "requires FFmpeg"]
    async fn process_media_invalid_command_reports_failure_and_reaps_child() {
        let mut process = FfmpegProcess::spawn(
            vec!["-invalid-clipforge-option".into()],
            EncoderInput::Device,
        )
        .await
        .unwrap();
        assert!(process
            .wait_for_frame(Duration::from_secs(5))
            .await
            .is_err());
        assert!(process.stop().await.is_err());
        assert!(process.child.try_wait().unwrap().is_some());
    }
}
