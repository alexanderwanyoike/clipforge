use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HwAccelType {
    Vaapi,
    Nvenc,
    Qsv,
    Software,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncoderInfo {
    pub name: String,
    pub hw_accel: HwAccelType,
    pub available: bool,
    pub device: Option<String>,
}

impl EncoderInfo {
    pub fn codec_name(&self) -> &str {
        &self.name
    }

    pub fn is_hardware(&self) -> bool {
        self.hw_accel != HwAccelType::Software
    }
}

/// Probe available hardware encoders by running test encodes.
/// Returns a list sorted by priority (best first).
pub async fn probe_encoders() -> Vec<EncoderInfo> {
    let mut encoders = Vec::new();

    // Test VA-API
    let vaapi_devices = find_vaapi_devices().await;
    for device in &vaapi_devices {
        if test_vaapi_encoder(device).await {
            info!(device = %device, "VA-API encoder available");
            encoders.push(EncoderInfo {
                name: "h264_vaapi".to_string(),
                hw_accel: HwAccelType::Vaapi,
                available: true,
                device: Some(device.clone()),
            });
            break; // Use first working device
        }
    }

    // Test NVENC
    if test_nvenc_encoder().await {
        info!("NVENC encoder available");
        encoders.push(EncoderInfo {
            name: "h264_nvenc".to_string(),
            hw_accel: HwAccelType::Nvenc,
            available: true,
            device: None,
        });
    }

    // Test QSV
    if test_qsv_encoder().await {
        info!("QSV encoder available");
        encoders.push(EncoderInfo {
            name: "h264_qsv".to_string(),
            hw_accel: HwAccelType::Qsv,
            available: true,
            device: None,
        });
    }

    // Software fallback always available
    encoders.push(EncoderInfo {
        name: "libx264".to_string(),
        hw_accel: HwAccelType::Software,
        available: true,
        device: None,
    });

    encoders
}

/// Find VA-API render devices
async fn find_vaapi_devices() -> Vec<String> {
    let mut devices = Vec::new();
    for i in 128..136 {
        let path = format!("/dev/dri/renderD{i}");
        if tokio::fs::metadata(&path).await.is_ok() {
            devices.push(path);
        }
    }
    devices
}

/// Test VA-API encoder with a 1-frame encode
async fn test_vaapi_encoder(device: &str) -> bool {
    let result = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=0.1:size=64x64:rate=1",
            "-vaapi_device",
            device,
            "-vf",
            "format=nv12,hwupload",
            "-c:v",
            "h264_vaapi",
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await;

    match result {
        Ok(output) => {
            if output.status.success() {
                true
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                debug!(stderr = %stderr, "VA-API test failed for {}", device);
                false
            }
        }
        Err(e) => {
            debug!(error = %e, "failed to run VA-API test");
            false
        }
    }
}

/// Test NVENC encoder
async fn test_nvenc_encoder() -> bool {
    let result = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=0.1:size=64x64:rate=1",
            "-c:v",
            "h264_nvenc",
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await;

    match result {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Test QSV encoder
async fn test_qsv_encoder() -> bool {
    let result = Command::new("ffmpeg")
        .args([
            "-y",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=duration=0.1:size=64x64:rate=1",
            "-c:v",
            "h264_qsv",
            "-frames:v",
            "1",
            "-f",
            "null",
            "-",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await;

    match result {
        Ok(output) => output.status.success(),
        Err(_) => false,
    }
}

/// Select the best encoder from probed results
pub fn select_best_encoder(encoders: &[EncoderInfo]) -> &EncoderInfo {
    encoders
        .iter()
        .find(|e| e.available)
        .expect("at least software encoder should be available")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_encoder(name: &str, hw: HwAccelType, available: bool) -> EncoderInfo {
        EncoderInfo {
            name: name.to_string(),
            hw_accel: hw,
            available,
            device: None,
        }
    }

    #[test]
    fn select_best_prefers_first_available() {
        let encoders = vec![
            make_encoder("h264_vaapi", HwAccelType::Vaapi, true),
            make_encoder("libx264", HwAccelType::Software, true),
        ];
        let best = select_best_encoder(&encoders);
        assert_eq!(best.name, "h264_vaapi");
    }

    #[test]
    fn select_best_skips_unavailable() {
        let encoders = vec![
            make_encoder("h264_vaapi", HwAccelType::Vaapi, false),
            make_encoder("h264_nvenc", HwAccelType::Nvenc, false),
            make_encoder("libx264", HwAccelType::Software, true),
        ];
        let best = select_best_encoder(&encoders);
        assert_eq!(best.name, "libx264");
    }

    #[test]
    fn is_hardware_true_for_hw_types() {
        assert!(make_encoder("vaapi", HwAccelType::Vaapi, true).is_hardware());
        assert!(make_encoder("nvenc", HwAccelType::Nvenc, true).is_hardware());
        assert!(make_encoder("qsv", HwAccelType::Qsv, true).is_hardware());
        assert!(!make_encoder("sw", HwAccelType::Software, true).is_hardware());
    }

    #[test]
    fn codec_name_returns_name() {
        let enc = make_encoder("h264_vaapi", HwAccelType::Vaapi, true);
        assert_eq!(enc.codec_name(), "h264_vaapi");
    }
}
