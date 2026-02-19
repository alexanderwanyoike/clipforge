use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub checks: Vec<DiagnosticCheck>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticCheck {
    pub name: String,
    pub status: CheckStatus,
    pub detail: String,
    pub recommendation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckStatus::Pass => write!(f, "PASS"),
            CheckStatus::Warn => write!(f, "WARN"),
            CheckStatus::Fail => write!(f, "FAIL"),
        }
    }
}

pub async fn run_diagnostics() -> DiagnosticReport {
    let mut checks = Vec::new();

    // Run all checks concurrently
    let (os, display, ffmpeg, vaapi, pipewire, audio, disk) = tokio::join!(
        check_os(),
        check_display_server(),
        check_ffmpeg(),
        check_vaapi(),
        check_pipewire(),
        check_audio_sources(),
        check_disk_space(),
    );

    checks.push(os);
    checks.push(display);
    checks.push(ffmpeg);
    checks.push(vaapi);
    checks.push(pipewire);
    checks.push(audio);
    checks.push(disk);

    DiagnosticReport { checks }
}

async fn check_os() -> DiagnosticCheck {
    let kernel = run_command("uname", &["-r"]).await.unwrap_or_default();
    let os_release = tokio::fs::read_to_string("/etc/os-release")
        .await
        .unwrap_or_default();

    let pretty_name = os_release
        .lines()
        .find(|l| l.starts_with("PRETTY_NAME="))
        .map(|l| l.trim_start_matches("PRETTY_NAME=").trim_matches('"'))
        .unwrap_or("Unknown")
        .to_string();

    DiagnosticCheck {
        name: "Operating System".to_string(),
        status: CheckStatus::Pass,
        detail: format!("{} (kernel {})", pretty_name, kernel.trim()),
        recommendation: None,
    }
}

async fn check_display_server() -> DiagnosticCheck {
    let display = std::env::var("DISPLAY").ok();
    let wayland = std::env::var("WAYLAND_DISPLAY").ok();

    let (status, detail, rec) = match (&display, &wayland) {
        (Some(d), _) => (CheckStatus::Pass, format!("X11 (DISPLAY={})", d), None),
        (None, Some(w)) => (
            CheckStatus::Warn,
            format!("Wayland only (WAYLAND_DISPLAY={})", w),
            Some("X11 capture is the primary path. Consider running under XWayland.".to_string()),
        ),
        _ => (
            CheckStatus::Fail,
            "No display server detected".to_string(),
            Some("ClipForge requires X11 or Wayland with XWayland".to_string()),
        ),
    };

    DiagnosticCheck {
        name: "Display Server".to_string(),
        status,
        detail,
        recommendation: rec,
    }
}

async fn check_ffmpeg() -> DiagnosticCheck {
    match run_command("ffmpeg", &["-version"]).await {
        Ok(output) => {
            let version_line = output.lines().next().unwrap_or("unknown");
            DiagnosticCheck {
                name: "FFmpeg".to_string(),
                status: CheckStatus::Pass,
                detail: version_line.to_string(),
                recommendation: None,
            }
        }
        Err(_) => DiagnosticCheck {
            name: "FFmpeg".to_string(),
            status: CheckStatus::Fail,
            detail: "FFmpeg not found in PATH".to_string(),
            recommendation: Some("Install FFmpeg: sudo apt install ffmpeg".to_string()),
        },
    }
}

async fn check_vaapi() -> DiagnosticCheck {
    let device = "/dev/dri/renderD128";
    if !std::path::Path::new(device).exists() {
        return DiagnosticCheck {
            name: "VA-API".to_string(),
            status: CheckStatus::Warn,
            detail: "No render device found at /dev/dri/renderD128".to_string(),
            recommendation: Some("Check GPU drivers are installed".to_string()),
        };
    }

    // Test with vainfo
    match run_command("vainfo", &["--display", "drm", "--device", device]).await {
        Ok(output) => {
            let profiles: Vec<&str> = output
                .lines()
                .filter(|l| l.contains("VAProfileH264"))
                .collect();
            if profiles.is_empty() {
                DiagnosticCheck {
                    name: "VA-API".to_string(),
                    status: CheckStatus::Warn,
                    detail: format!("VA-API device found but no H.264 profiles ({})", device),
                    recommendation: Some(
                        "Install VA-API drivers: sudo apt install intel-media-va-driver-non-free"
                            .to_string(),
                    ),
                }
            } else {
                DiagnosticCheck {
                    name: "VA-API".to_string(),
                    status: CheckStatus::Pass,
                    detail: format!("Device {} with {} H.264 profiles", device, profiles.len()),
                    recommendation: None,
                }
            }
        }
        Err(_) => DiagnosticCheck {
            name: "VA-API".to_string(),
            status: CheckStatus::Warn,
            detail: "vainfo not found; cannot verify VA-API support".to_string(),
            recommendation: Some("Install vainfo: sudo apt install vainfo".to_string()),
        },
    }
}

async fn check_pipewire() -> DiagnosticCheck {
    match run_command("pw-cli", &["info", "0"]).await {
        Ok(_output) => DiagnosticCheck {
            name: "PipeWire".to_string(),
            status: CheckStatus::Pass,
            detail: "PipeWire daemon running".to_string(),
            recommendation: None,
        },
        Err(_) => {
            // Check PulseAudio as fallback
            match run_command("pactl", &["info"]).await {
                Ok(_) => DiagnosticCheck {
                    name: "PipeWire".to_string(),
                    status: CheckStatus::Pass,
                    detail: "PulseAudio running (PipeWire not detected)".to_string(),
                    recommendation: None,
                },
                Err(_) => DiagnosticCheck {
                    name: "PipeWire".to_string(),
                    status: CheckStatus::Fail,
                    detail: "No audio server detected".to_string(),
                    recommendation: Some("Install PipeWire or PulseAudio".to_string()),
                },
            }
        }
    }
}

async fn check_audio_sources() -> DiagnosticCheck {
    match crate::audio::list_audio_sources().await {
        Ok(sources) => {
            let monitors: Vec<_> = sources
                .iter()
                .filter(|s| matches!(s.source_type, crate::audio::AudioSourceType::Monitor))
                .collect();
            let inputs: Vec<_> = sources
                .iter()
                .filter(|s| matches!(s.source_type, crate::audio::AudioSourceType::Input))
                .collect();

            DiagnosticCheck {
                name: "Audio Sources".to_string(),
                status: if monitors.is_empty() {
                    CheckStatus::Warn
                } else {
                    CheckStatus::Pass
                },
                detail: format!("{} monitors, {} inputs", monitors.len(), inputs.len()),
                recommendation: if monitors.is_empty() {
                    Some(
                        "No desktop audio monitor found. Check PulseAudio/PipeWire config."
                            .to_string(),
                    )
                } else {
                    None
                },
            }
        }
        Err(_) => DiagnosticCheck {
            name: "Audio Sources".to_string(),
            status: CheckStatus::Fail,
            detail: "Failed to enumerate audio sources".to_string(),
            recommendation: Some(
                "Check pactl is installed and audio server is running".to_string(),
            ),
        },
    }
}

async fn check_disk_space() -> DiagnosticCheck {
    match run_command("df", &["-h", "/"]).await {
        Ok(output) => {
            let line = output.lines().nth(1).unwrap_or("");
            let parts: Vec<&str> = line.split_whitespace().collect();
            let available = parts.get(3).unwrap_or(&"unknown");
            let use_pct = parts.get(4).unwrap_or(&"?");

            DiagnosticCheck {
                name: "Disk Space".to_string(),
                status: CheckStatus::Pass,
                detail: format!(
                    "{} available ({}% used)",
                    available,
                    use_pct.trim_end_matches('%')
                ),
                recommendation: None,
            }
        }
        Err(_) => DiagnosticCheck {
            name: "Disk Space".to_string(),
            status: CheckStatus::Warn,
            detail: "Could not check disk space".to_string(),
            recommendation: None,
        },
    }
}

async fn run_command(cmd: &str, args: &[&str]) -> std::result::Result<String, ()> {
    let output = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .map_err(|_| ())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(())
    }
}
