use crate::capture::CaptureSource;
use crate::config::{CaptureMode, Config};
use crate::error::{Error, Result};
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

/// Get the current X11 display string
pub fn get_display() -> Result<String> {
    std::env::var("DISPLAY").map_err(|_| Error::NoDisplay)
}

/// Get screen resolution via xdpyinfo or xrandr
pub async fn get_screen_resolution() -> Result<(u32, u32)> {
    // Try xdpyinfo first
    let output = Command::new("xdpyinfo")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await;

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            let line = line.trim();
            if line.starts_with("dimensions:") {
                // "dimensions:    1920x1080 pixels (...)"
                if let Some(dims) = line.split_whitespace().nth(1) {
                    if let Some((w, h)) = dims.split_once('x') {
                        if let (Ok(w), Ok(h)) = (w.parse(), h.parse()) {
                            debug!(width = w, height = h, "detected screen resolution");
                            return Ok((w, h));
                        }
                    }
                }
            }
        }
    }

    // Fallback to xrandr
    let output = Command::new("xrandr")
        .arg("--current")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(Error::Io)?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.contains(" connected") && line.contains('x') {
            // "eDP-1 connected primary 1920x1080+0+0 ..."
            for word in line.split_whitespace() {
                if word.contains('x') && word.contains('+') {
                    let res_part = word.split('+').next().unwrap_or("");
                    if let Some((w, h)) = res_part.split_once('x') {
                        if let (Ok(w), Ok(h)) = (w.parse(), h.parse()) {
                            debug!(width = w, height = h, "detected screen resolution via xrandr");
                            return Ok((w, h));
                        }
                    }
                }
            }
        }
    }

    // Default fallback
    Ok((1920, 1080))
}

/// Create a CaptureSource from the current config
pub async fn create_capture_source(config: &Config) -> Result<CaptureSource> {
    let display = get_display()?;
    let fps = config.recording.fps;

    match &config.recording.capture_mode {
        CaptureMode::Fullscreen => {
            let (width, height) = get_screen_resolution().await?;
            Ok(CaptureSource::X11Fullscreen {
                display,
                width,
                height,
                fps,
            })
        }
        CaptureMode::Window { id } => {
            let window_id = match id {
                Some(id) => id.clone(),
                None => select_window().await?,
            };
            Ok(CaptureSource::X11Window {
                display,
                window_id,
                fps,
            })
        }
        CaptureMode::Region { x, y, w, h } => {
            Ok(CaptureSource::X11Region {
                display,
                x: *x,
                y: *y,
                width: *w,
                height: *h,
                fps,
            })
        }
    }
}

/// Use xdotool to let user click a window to select it
async fn select_window() -> Result<String> {
    let output = Command::new("xdotool")
        .arg("selectwindow")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(Error::Io)?;

    if output.status.success() {
        let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(id)
    } else {
        Err(Error::Other("failed to select window via xdotool".into()))
    }
}
