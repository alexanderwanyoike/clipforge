use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;
use tracing::debug;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSource {
    pub id: String,
    pub name: String,
    pub source_type: AudioSourceType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioSourceType {
    Monitor, // Desktop audio (monitor of output sink)
    Input,   // Microphone
}

/// List available PulseAudio/PipeWire audio sources
///
/// Uses verbose `pactl list sources` to extract human-readable device descriptions
/// (e.g. "WH-CH520" instead of `bluez_output.E8_9E_13_A3_79_AF.1.monitor`).
/// Falls back to the raw PulseAudio name if a description isn't available.
pub async fn list_audio_sources() -> Result<Vec<AudioSource>> {
    let output = Command::new("pactl")
        .args(["list", "sources"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(Error::Io)?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut sources = Vec::new();

    let mut current_name: Option<String> = None;
    let mut current_description: Option<String> = None;

    for line in stdout.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("Name: ") {
            // Flush previous source if any
            if let Some(name) = current_name.take() {
                let desc = current_description.take().unwrap_or_else(|| name.clone());
                let source_type = if name.contains(".monitor") {
                    AudioSourceType::Monitor
                } else {
                    AudioSourceType::Input
                };
                debug!(id = %name, description = %desc, "found audio source");
                sources.push(AudioSource {
                    id: name,
                    name: desc,
                    source_type,
                });
            }
            current_name = Some(trimmed.trim_start_matches("Name: ").to_string());
            current_description = None;
        } else if trimmed.starts_with("device.description = ") {
            let desc = trimmed
                .trim_start_matches("device.description = ")
                .trim_matches('"')
                .to_string();
            current_description = Some(desc);
        }
    }

    // Flush last source
    if let Some(name) = current_name {
        let desc = current_description.unwrap_or_else(|| name.clone());
        let source_type = if name.contains(".monitor") {
            AudioSourceType::Monitor
        } else {
            AudioSourceType::Input
        };
        debug!(id = %name, description = %desc, "found audio source");
        sources.push(AudioSource {
            id: name,
            name: desc,
            source_type,
        });
    }

    Ok(sources)
}

/// Get the default audio source (desktop audio monitor)
pub async fn get_default_monitor() -> Result<Option<String>> {
    let sources = list_audio_sources().await?;
    Ok(sources
        .into_iter()
        .find(|s| matches!(s.source_type, AudioSourceType::Monitor))
        .map(|s| s.id))
}

/// Resolve an audio source name for FFmpeg.
///
/// If the source is `"default"`, queries PulseAudio/PipeWire for the current
/// default sink and returns its `.monitor` source — this captures desktop audio
/// (whatever is playing through speakers/headphones) instead of the microphone.
///
/// Non-`"default"` values pass through unchanged.
pub async fn resolve_audio_source(source: &str) -> Result<String> {
    if source != "default" {
        return Ok(source.to_string());
    }

    let output = Command::new("pactl")
        .args(["get-default-sink"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
        .map_err(Error::Io)?;

    if !output.status.success() {
        debug!("pactl get-default-sink failed, falling back to 'default'");
        return Ok("default".to_string());
    }

    let sink = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if sink.is_empty() {
        return Ok("default".to_string());
    }

    let monitor = format!("{}.monitor", sink);
    debug!(sink = %sink, monitor = %monitor, "resolved default audio source");
    Ok(monitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolve_audio_source_passthrough() {
        // Non-"default" values should pass through unchanged
        let result = resolve_audio_source("bluez_output.E8_9E_13_A3_79_AF.1.monitor")
            .await
            .unwrap();
        assert_eq!(result, "bluez_output.E8_9E_13_A3_79_AF.1.monitor");

        let result = resolve_audio_source("alsa_input.pci-0000_00_1f.3.analog-stereo")
            .await
            .unwrap();
        assert_eq!(result, "alsa_input.pci-0000_00_1f.3.analog-stereo");
    }

    #[test]
    fn source_type_classification() {
        // .monitor suffix → Monitor type
        let name = "bluez_output.E8_9E_13_A3_79_AF.1.monitor";
        let source_type = if name.contains(".monitor") {
            AudioSourceType::Monitor
        } else {
            AudioSourceType::Input
        };
        assert!(matches!(source_type, AudioSourceType::Monitor));

        // No .monitor suffix → Input type
        let name = "alsa_input.pci-0000_00_1f.3.analog-stereo";
        let source_type = if name.contains(".monitor") {
            AudioSourceType::Monitor
        } else {
            AudioSourceType::Input
        };
        assert!(matches!(source_type, AudioSourceType::Input));
    }
}
