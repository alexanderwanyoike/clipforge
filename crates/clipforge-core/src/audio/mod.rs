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
    Monitor,  // Desktop audio (monitor of output sink)
    Input,    // Microphone
}

/// List available PulseAudio/PipeWire audio sources
pub async fn list_audio_sources() -> Result<Vec<AudioSource>> {
    let output = Command::new("pactl")
        .args(["list", "short", "sources"])
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

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 2 {
            let name = parts[1].to_string();
            let source_type = if name.contains(".monitor") {
                AudioSourceType::Monitor
            } else {
                AudioSourceType::Input
            };

            debug!(name = %name, "found audio source");

            sources.push(AudioSource {
                id: name.clone(),
                name,
                source_type,
            });
        }
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
