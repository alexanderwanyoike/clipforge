use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use tracing::{debug};

/// Manages the circular segment buffer for instant replay
pub struct ReplayRing {
    pub segment_dir: PathBuf,
    pub segment_list: PathBuf,
    pub segment_time: u32,
    pub max_segments: u32,
}

#[derive(Debug, Clone)]
pub struct SegmentEntry {
    pub filename: String,
    pub start_time: f64,
    pub end_time: f64,
}

impl ReplayRing {
    pub fn new(segment_dir: &Path, segment_time: u32, max_segments: u32) -> Self {
        Self {
            segment_dir: segment_dir.to_path_buf(),
            segment_list: segment_dir.join("segments.csv"),
            segment_time,
            max_segments,
        }
    }

    /// Parse FFmpeg's segment list CSV to get current segments
    pub fn parse_segments(&self) -> Result<Vec<SegmentEntry>> {
        if !self.segment_list.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(&self.segment_list).map_err(Error::Io)?;
        let mut segments = Vec::new();

        for line in content.lines() {
            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() >= 3 {
                let filename = parts[0].trim().to_string();
                let start_time: f64 = parts[1].trim().parse().unwrap_or(0.0);
                let end_time: f64 = parts[2].trim().parse().unwrap_or(0.0);

                segments.push(SegmentEntry {
                    filename,
                    start_time,
                    end_time,
                });
            }
        }

        Ok(segments)
    }

    /// Get segments covering the last N seconds, handling ring buffer wrap
    pub fn get_last_n_seconds(&self, seconds: u32) -> Result<Vec<PathBuf>> {
        let segments = self.parse_segments()?;
        if segments.is_empty() {
            return Err(Error::NoSegments);
        }

        let needed_segments = (seconds / self.segment_time).max(1) as usize;
        let total = segments.len();

        // Take the last N segments (they're in chronological order in the CSV)
        let start_idx = total.saturating_sub(needed_segments);
        let selected: Vec<PathBuf> = segments[start_idx..]
            .iter()
            .map(|s| self.segment_dir.join(&s.filename))
            .filter(|p| p.exists())
            .collect();

        if selected.is_empty() {
            return Err(Error::NoSegments);
        }

        debug!(
            count = selected.len(),
            seconds = seconds,
            "selected segments for replay save"
        );

        Ok(selected)
    }

    /// Clean up segment directory
    pub fn cleanup(&self) -> Result<()> {
        if self.segment_dir.exists() {
            std::fs::remove_dir_all(&self.segment_dir).map_err(Error::Io)?;
        }
        std::fs::create_dir_all(&self.segment_dir).map_err(Error::Io)?;
        Ok(())
    }
}
