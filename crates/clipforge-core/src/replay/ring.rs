use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use tracing::debug;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_fields_correctly() {
        let dir = PathBuf::from("/tmp/test_segments");
        let ring = ReplayRing::new(&dir, 3, 40);
        assert_eq!(ring.segment_dir, dir);
        assert_eq!(ring.segment_list, dir.join("segments.csv"));
        assert_eq!(ring.segment_time, 3);
        assert_eq!(ring.max_segments, 40);
    }

    #[test]
    fn parse_segments_no_file_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let ring = ReplayRing::new(tmp.path(), 3, 40);
        // segment_list file doesn't exist
        let segments = ring.parse_segments().unwrap();
        assert!(segments.is_empty());
    }

    #[test]
    fn parse_segments_parses_csv_correctly() {
        let tmp = tempfile::tempdir().unwrap();
        let ring = ReplayRing::new(tmp.path(), 3, 40);
        let csv = "seg_000.mkv,0.000000,3.000000\nseg_001.mkv,3.000000,6.000000\nseg_002.mkv,6.000000,9.000000\n";
        std::fs::write(&ring.segment_list, csv).unwrap();

        let segments = ring.parse_segments().unwrap();
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].filename, "seg_000.mkv");
        assert!((segments[0].start_time - 0.0).abs() < 0.001);
        assert!((segments[0].end_time - 3.0).abs() < 0.001);
        assert_eq!(segments[2].filename, "seg_002.mkv");
        assert!((segments[2].end_time - 9.0).abs() < 0.001);
    }

    #[test]
    fn parse_segments_skips_malformed_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let ring = ReplayRing::new(tmp.path(), 3, 40);
        let csv = "seg_000.mkv,0.0,3.0\nbadline\nseg_001.mkv,3.0,6.0\nonly,two\n";
        std::fs::write(&ring.segment_list, csv).unwrap();

        let segments = ring.parse_segments().unwrap();
        assert_eq!(segments.len(), 2);
    }

    #[test]
    fn get_last_n_seconds_selects_correct_count() {
        let tmp = tempfile::tempdir().unwrap();
        let ring = ReplayRing::new(tmp.path(), 3, 40);

        // Create 5 segment files and CSV
        let mut csv = String::new();
        for i in 0..5 {
            let name = format!("seg_{i:03}.mkv");
            std::fs::write(tmp.path().join(&name), "data").unwrap();
            csv.push_str(&format!("{},{}.0,{}.0\n", name, i * 3, (i + 1) * 3));
        }
        std::fs::write(&ring.segment_list, &csv).unwrap();

        // 9 seconds / 3 second segments = 3 segments
        let paths = ring.get_last_n_seconds(9).unwrap();
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn get_last_n_seconds_caps_at_available() {
        let tmp = tempfile::tempdir().unwrap();
        let ring = ReplayRing::new(tmp.path(), 3, 40);

        // Only 2 segments
        let mut csv = String::new();
        for i in 0..2 {
            let name = format!("seg_{i:03}.mkv");
            std::fs::write(tmp.path().join(&name), "data").unwrap();
            csv.push_str(&format!("{},{}.0,{}.0\n", name, i * 3, (i + 1) * 3));
        }
        std::fs::write(&ring.segment_list, &csv).unwrap();

        // Request 30 seconds but only 2 segments exist
        let paths = ring.get_last_n_seconds(30).unwrap();
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn get_last_n_seconds_no_segments_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let ring = ReplayRing::new(tmp.path(), 3, 40);
        // Empty CSV
        std::fs::write(&ring.segment_list, "").unwrap();

        let result = ring.get_last_n_seconds(9);
        assert!(result.is_err());
    }

    #[test]
    fn cleanup_removes_and_recreates_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let seg_dir = tmp.path().join("segments");
        std::fs::create_dir_all(&seg_dir).unwrap();
        std::fs::write(seg_dir.join("old_file.mkv"), "old").unwrap();

        let ring = ReplayRing::new(&seg_dir, 3, 40);
        ring.cleanup().unwrap();

        assert!(seg_dir.exists());
        assert!(!seg_dir.join("old_file.mkv").exists());
    }
}
