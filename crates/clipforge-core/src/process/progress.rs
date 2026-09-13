//! Decoder for FFmpeg's `-progress` protocol. No process or recording state lives here.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize)]
pub struct FfmpegProgress {
    pub frame: u64,
    pub fps: f64,
    pub time: String,
    pub speed: String,
    pub size_kb: u64,
}

#[derive(Default)]
pub struct ProgressDecoder {
    current: FfmpegProgress,
}

impl ProgressDecoder {
    /// Emit a snapshot at a record boundary. Malformed/unavailable fields leave
    /// the last valid value intact; diagnostics and unknown keys are ignored.
    pub fn accept(&mut self, line: &str) -> Option<FfmpegProgress> {
        let (key, value) = line.split_once('=')?;
        let value = value.trim();
        match key {
            "frame" => update_number(&mut self.current.frame, value),
            "fps" => {
                if let Ok(fps) = value.parse::<f64>() {
                    if fps.is_finite() && fps >= 0.0 {
                        self.current.fps = fps;
                    }
                }
            }
            "out_time" if value != "N/A" => self.current.time = value.into(),
            "speed" if value != "N/A" => self.current.speed = value.into(),
            "total_size" => {
                if let Ok(bytes) = value.parse::<u64>() {
                    self.current.size_kb = bytes / 1024;
                }
            }
            "progress" if matches!(value, "continue" | "end") => return Some(self.current.clone()),
            _ => {}
        }
        None
    }
}

fn update_number(target: &mut u64, value: &str) {
    if let Ok(number) = value.parse() {
        *target = number;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_complete_records_only() {
        let mut decoder = ProgressDecoder::default();
        for line in [
            "frame= 12",
            "fps=29.97",
            "out_time=00:00:00.40",
            "speed=1.0x",
            "total_size=4096",
        ] {
            assert_eq!(decoder.accept(line), None);
        }
        assert_eq!(
            decoder.accept("progress=continue"),
            Some(FfmpegProgress {
                frame: 12,
                fps: 29.97,
                time: "00:00:00.40".into(),
                speed: "1.0x".into(),
                size_kb: 4,
            })
        );
    }

    #[test]
    fn malformed_fields_preserve_last_valid_values() {
        let mut decoder = ProgressDecoder::default();
        decoder.accept("frame=10");
        decoder.accept("fps=30");
        decoder.accept("total_size=2048");
        let previous = decoder.accept("progress=continue");
        for line in [
            "frame=N/A",
            "frame=-1",
            "frame=18446744073709551616",
            "fps=NaN",
            "fps=inf",
            "fps=-1",
            "total_size=N/A",
            "total_size=-2",
            "",
            "💥",
            "progress=garbage",
            "ffmpeg diagnostic",
        ] {
            assert_eq!(decoder.accept(line), None);
        }
        assert_eq!(decoder.accept("progress=end"), previous);
    }

    #[test]
    fn accepts_future_keys_and_unicode_without_panicking() {
        let mut decoder = ProgressDecoder::default();
        for value in ["", "=", "unknown=123", "frame=🌍", "progress=", "frame=1=2"] {
            decoder.accept(value);
        }
        assert_eq!(
            decoder.accept("progress=end"),
            Some(FfmpegProgress::default())
        );
    }
}
