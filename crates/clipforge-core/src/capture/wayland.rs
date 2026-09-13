//! Portal-authorized PipeWire capture. GStreamer transports uncompressed frames;
//! FFmpeg remains responsible for encoding, audio and replay segmentation.
use crate::process::{capture_command, host_command};
use crate::{
    config::CaptureMode,
    error::{Error, Result},
};
use ashpd::desktop::{
    screencast::{CursorMode, Screencast, SourceType},
    PersistMode, Session,
};
use futures_util::StreamExt;
use std::{os::fd::OwnedFd, process::Stdio};
use tokio::process::Child;
use tokio::{sync::watch, task::JoinHandle};

pub struct WaylandCapture {
    session: Option<Session<'static, Screencast<'static>>>,
    remote: Option<OwnedFd>,
    node: u32,
    fps: u32,
    pub(crate) closed: watch::Receiver<bool>,
    closed_task: Option<JoinHandle<()>>,
}

impl WaylandCapture {
    pub async fn open(mode: &CaptureMode, fps: u32) -> Result<Self> {
        if matches!(mode, CaptureMode::Region { .. }) {
            return Err(Error::Other("Region capture is not available on Wayland yet. Choose fullscreen or window capture.".into()));
        }
        check_dependencies().await?;
        let portal = Screencast::new().await.map_err(portal_error)?;
        let session = portal.create_session().await.map_err(portal_error)?;
        let (closed_tx, closed) = watch::channel(false);
        // Own the session before any fallible request so cancellation cleans it up.
        let mut capture = Self {
            session: Some(session),
            remote: None,
            node: 0,
            fps,
            closed,
            closed_task: None,
        };
        let session = capture
            .session
            .as_ref()
            .ok_or_else(|| Error::Other("Screen sharing session is unavailable".into()))?;
        let mut closed_signal = session.receive_closed().await.map_err(portal_error)?;
        capture.closed_task = Some(tokio::spawn(async move {
            if closed_signal.next().await.is_some() {
                let _ = closed_tx.send(true);
            }
        }));
        let source = match mode {
            CaptureMode::Window { .. } => SourceType::Window,
            _ => SourceType::Monitor,
        };
        let cursors = portal
            .available_cursor_modes()
            .await
            .map_err(portal_error)?;
        let cursor = if cursors.contains(CursorMode::Embedded) {
            CursorMode::Embedded
        } else {
            CursorMode::Hidden
        };
        portal
            .select_sources(
                session,
                cursor,
                source.into(),
                false,
                None,
                PersistMode::DoNot,
            )
            .await
            .map_err(portal_error)?
            .response()
            .map_err(portal_error)?;
        let response = portal
            .start(session, None)
            .await
            .map_err(portal_error)?
            .response()
            .map_err(portal_error)?;
        let stream = response
            .streams()
            .first()
            .ok_or_else(|| Error::Other("No screen or window was selected.".into()))?;
        capture.node = stream.pipe_wire_node_id();
        capture.remote = Some(
            portal
                .open_pipe_wire_remote(session)
                .await
                .map_err(portal_error)?,
        );
        Ok(capture)
    }

    pub fn spawn_helper(&mut self) -> Result<Child> {
        let remote = self
            .remote
            .take()
            .ok_or_else(|| Error::Other("Capture stream already started".into()))?;
        let mut command = capture_command("gst-launch-1.0");
        command
            .args(pipeline_args(self.node, self.fps))
            // FD 0 is the portal's restricted PipeWire connection, not a node
            // discovered on the unrestricted user bus. Only this child inherits it.
            .stdin(Stdio::from(remote))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command.spawn().map_err(Error::Io)
    }

    pub async fn close(&mut self) {
        if let Some(task) = self.closed_task.take() {
            task.abort();
        }
        self.remote = None;
        if let Some(session) = self.session.take() {
            let _ = session.close().await;
        }
    }
}

impl Drop for WaylandCapture {
    fn drop(&mut self) {
        if let Some(task) = self.closed_task.take() {
            task.abort();
        }
        if let Some(session) = self.session.take() {
            if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                runtime.spawn(async move {
                    let _ = session.close().await;
                });
            }
        }
    }
}

fn portal_error(error: ashpd::Error) -> Error {
    Error::Other(format!("Screen sharing could not start: {error}. Select a screen/window in the desktop sharing dialog, or retry if it was cancelled."))
}

pub fn pipeline_args(node: u32, fps: u32) -> Vec<String> {
    [
        "-q".into(),
        "pipewiresrc".into(),
        "fd=0".into(),
        format!("path={node}"),
        "do-timestamp=true".into(),
        // Damage-driven compositors may send no frames while the screen is idle.
        // Keep timestamps advancing so startup, audio and replay continue.
        "keepalive-time=100".into(),
        "!".into(),
        // Request mappable system-memory frames from the compositor instead
        // of passing GPU-only DMA-BUF surfaces into CPU color conversion.
        "capsfilter".into(),
        "caps=video/x-raw".into(),
        "!".into(),
        "queue".into(),
        "max-size-buffers=2".into(),
        "max-size-bytes=0".into(),
        "max-size-time=0".into(),
        "!".into(),
        "videoconvert".into(),
        "!".into(),
        "videorate".into(),
        "!".into(),
        format!("video/x-raw,format=I420,framerate={fps}/1"),
        "!".into(),
        "y4menc".into(),
        "!".into(),
        "fdsink".into(),
        "fd=1".into(),
        "sync=false".into(),
    ]
    .into()
}

pub async fn check_dependencies() -> Result<()> {
    for element in [
        "pipewiresrc",
        "queue",
        "videoconvert",
        "videorate",
        "y4menc",
        "fdsink",
    ] {
        let output = host_command("gst-inspect-1.0").arg(element).output().await;
        if !matches!(output, Ok(ref o) if o.status.success()) {
            return Err(Error::Other(format!("Wayland capture needs GStreamer with {element}. Install gstreamer1.0-tools, gstreamer1.0-pipewire, gstreamer1.0-plugins-base and gstreamer1.0-plugins-good (Arch: gstreamer gst-plugin-pipewire gst-plugins-base gst-plugins-good).")));
        }
    }
    Ok(())
}
