use crate::error::{Error, Result};
use crate::process::run_ffprobe;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path};
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: String,
    pub title: String,
    pub file_path: String,
    pub file_size: i64,
    pub duration: f64,
    pub resolution: String,
    pub fps: f64,
    pub codec: String,
    pub container: String,
    pub source_type: String,
    pub game_name: Option<String>,
    pub created_at: String,
    pub thumbnail_path: Option<String>,
}

pub struct Library {
    conn: Connection,
}

impl Library {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }

        let conn = Connection::open(db_path)
            .map_err(|e| Error::Database(e.to_string()))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS recordings (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                file_path TEXT NOT NULL UNIQUE,
                file_size INTEGER NOT NULL DEFAULT 0,
                duration REAL NOT NULL DEFAULT 0,
                resolution TEXT NOT NULL DEFAULT '',
                fps REAL NOT NULL DEFAULT 0,
                codec TEXT NOT NULL DEFAULT '',
                container TEXT NOT NULL DEFAULT '',
                source_type TEXT NOT NULL DEFAULT 'recording',
                game_name TEXT,
                created_at TEXT NOT NULL,
                thumbnail_path TEXT
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS recordings_fts USING fts5(
                title, game_name, content=recordings, content_rowid=rowid
            );

            CREATE TRIGGER IF NOT EXISTS recordings_ai AFTER INSERT ON recordings BEGIN
                INSERT INTO recordings_fts(rowid, title, game_name)
                VALUES (new.rowid, new.title, new.game_name);
            END;

            CREATE TRIGGER IF NOT EXISTS recordings_ad AFTER DELETE ON recordings BEGIN
                INSERT INTO recordings_fts(recordings_fts, rowid, title, game_name)
                VALUES ('delete', old.rowid, old.title, old.game_name);
            END;

            CREATE TRIGGER IF NOT EXISTS recordings_au AFTER UPDATE ON recordings BEGIN
                INSERT INTO recordings_fts(recordings_fts, rowid, title, game_name)
                VALUES ('delete', old.rowid, old.title, old.game_name);
                INSERT INTO recordings_fts(rowid, title, game_name)
                VALUES (new.rowid, new.title, new.game_name);
            END;"
        ).map_err(|e| Error::Database(e.to_string()))?;

        Ok(Self { conn })
    }

    pub fn insert(&self, recording: &Recording) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO recordings
             (id, title, file_path, file_size, duration, resolution, fps, codec,
              container, source_type, game_name, created_at, thumbnail_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                recording.id,
                recording.title,
                recording.file_path,
                recording.file_size,
                recording.duration,
                recording.resolution,
                recording.fps,
                recording.codec,
                recording.container,
                recording.source_type,
                recording.game_name,
                recording.created_at,
                recording.thumbnail_path,
            ],
        ).map_err(|e| Error::Database(e.to_string()))?;

        info!(id = %recording.id, title = %recording.title, "recording inserted into library");
        Ok(())
    }

    pub fn list(&self, limit: u32, offset: u32) -> Result<Vec<Recording>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, file_path, file_size, duration, resolution, fps, codec,
                    container, source_type, game_name, created_at, thumbnail_path
             FROM recordings ORDER BY created_at DESC LIMIT ?1 OFFSET ?2"
        ).map_err(|e| Error::Database(e.to_string()))?;

        let rows = stmt.query_map(params![limit, offset], |row| {
            Ok(Recording {
                id: row.get(0)?,
                title: row.get(1)?,
                file_path: row.get(2)?,
                file_size: row.get(3)?,
                duration: row.get(4)?,
                resolution: row.get(5)?,
                fps: row.get(6)?,
                codec: row.get(7)?,
                container: row.get(8)?,
                source_type: row.get(9)?,
                game_name: row.get(10)?,
                created_at: row.get(11)?,
                thumbnail_path: row.get(12)?,
            })
        }).map_err(|e| Error::Database(e.to_string()))?;

        let mut recordings = Vec::new();
        for row in rows {
            recordings.push(row.map_err(|e| Error::Database(e.to_string()))?);
        }
        Ok(recordings)
    }

    pub fn search(&self, query: &str) -> Result<Vec<Recording>> {
        let mut stmt = self.conn.prepare(
            "SELECT r.id, r.title, r.file_path, r.file_size, r.duration, r.resolution,
                    r.fps, r.codec, r.container, r.source_type, r.game_name,
                    r.created_at, r.thumbnail_path
             FROM recordings r
             JOIN recordings_fts f ON r.rowid = f.rowid
             WHERE recordings_fts MATCH ?1
             ORDER BY r.created_at DESC"
        ).map_err(|e| Error::Database(e.to_string()))?;

        let rows = stmt.query_map(params![query], |row| {
            Ok(Recording {
                id: row.get(0)?,
                title: row.get(1)?,
                file_path: row.get(2)?,
                file_size: row.get(3)?,
                duration: row.get(4)?,
                resolution: row.get(5)?,
                fps: row.get(6)?,
                codec: row.get(7)?,
                container: row.get(8)?,
                source_type: row.get(9)?,
                game_name: row.get(10)?,
                created_at: row.get(11)?,
                thumbnail_path: row.get(12)?,
            })
        }).map_err(|e| Error::Database(e.to_string()))?;

        let mut recordings = Vec::new();
        for row in rows {
            recordings.push(row.map_err(|e| Error::Database(e.to_string()))?);
        }
        Ok(recordings)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM recordings WHERE id = ?1", params![id])
            .map_err(|e| Error::Database(e.to_string()))?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<Recording>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, file_path, file_size, duration, resolution, fps, codec,
                    container, source_type, game_name, created_at, thumbnail_path
             FROM recordings WHERE id = ?1"
        ).map_err(|e| Error::Database(e.to_string()))?;

        let result = stmt.query_row(params![id], |row| {
            Ok(Recording {
                id: row.get(0)?,
                title: row.get(1)?,
                file_path: row.get(2)?,
                file_size: row.get(3)?,
                duration: row.get(4)?,
                resolution: row.get(5)?,
                fps: row.get(6)?,
                codec: row.get(7)?,
                container: row.get(8)?,
                source_type: row.get(9)?,
                game_name: row.get(10)?,
                created_at: row.get(11)?,
                thumbnail_path: row.get(12)?,
            })
        });

        match result {
            Ok(recording) => Ok(Some(recording)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(Error::Database(e.to_string())),
        }
    }
}

/// Probe a media file with ffprobe and extract metadata
pub async fn probe_media(file_path: &Path) -> Result<MediaInfo> {
    let output = run_ffprobe(&[
        "-v", "quiet",
        "-print_format", "json",
        "-show_format",
        "-show_streams",
        &file_path.to_string_lossy(),
    ]).await?;

    let json: serde_json::Value = serde_json::from_str(&output).map_err(Error::Json)?;

    let duration = json["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);

    let file_size = json["format"]["size"]
        .as_str()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    let video_stream = json["streams"]
        .as_array()
        .and_then(|streams| {
            streams.iter().find(|s| s["codec_type"].as_str() == Some("video"))
        });

    let (width, height, fps, codec) = if let Some(stream) = video_stream {
        let w = stream["width"].as_u64().unwrap_or(0) as u32;
        let h = stream["height"].as_u64().unwrap_or(0) as u32;
        let codec = stream["codec_name"].as_str().unwrap_or("unknown").to_string();
        let fps_str = stream["r_frame_rate"].as_str().unwrap_or("0/1");
        let fps = parse_frame_rate(fps_str);
        (w, h, fps, codec)
    } else {
        (0, 0, 0.0, "unknown".to_string())
    };

    Ok(MediaInfo {
        duration,
        file_size,
        width,
        height,
        fps,
        codec,
    })
}

#[derive(Debug, Clone)]
pub struct MediaInfo {
    pub duration: f64,
    pub file_size: i64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub codec: String,
}

fn parse_frame_rate(s: &str) -> f64 {
    if let Some((num, den)) = s.split_once('/') {
        let n: f64 = num.parse().unwrap_or(0.0);
        let d: f64 = den.parse().unwrap_or(1.0);
        if d > 0.0 { n / d } else { 0.0 }
    } else {
        s.parse().unwrap_or(0.0)
    }
}

/// Generate a thumbnail for a video file
pub async fn generate_thumbnail(input: &Path, output: &Path) -> Result<()> {
    crate::process::run_ffmpeg(&[
        "-ss", "5",
        "-i", &input.to_string_lossy(),
        "-frames:v", "1",
        "-vf", "scale=320:-1",
        "-y",
        &output.to_string_lossy(),
    ]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_recording(id: &str, title: &str, game: Option<&str>) -> Recording {
        Recording {
            id: id.to_string(),
            title: title.to_string(),
            file_path: format!("/tmp/test_{id}.mkv"),
            file_size: 1024000,
            duration: 60.5,
            resolution: "1920x1080".to_string(),
            fps: 60.0,
            codec: "h264".to_string(),
            container: "mkv".to_string(),
            source_type: "recording".to_string(),
            game_name: game.map(|s| s.to_string()),
            created_at: format!("2025-01-01T00:00:{:02}Z", id.len()),
            thumbnail_path: None,
        }
    }

    #[test]
    fn open_creates_db_and_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("sub").join("library.db");
        let _lib = Library::open(&db_path).unwrap();
        assert!(db_path.exists());
    }

    #[test]
    fn insert_and_get_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(&tmp.path().join("lib.db")).unwrap();
        let rec = sample_recording("r1", "First Recording", Some("Elden Ring"));
        lib.insert(&rec).unwrap();

        let fetched = lib.get("r1").unwrap().expect("should find r1");
        assert_eq!(fetched.id, "r1");
        assert_eq!(fetched.title, "First Recording");
        assert_eq!(fetched.game_name.as_deref(), Some("Elden Ring"));
        assert_eq!(fetched.duration, 60.5);
    }

    #[test]
    fn get_returns_none_for_missing_id() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(&tmp.path().join("lib.db")).unwrap();
        assert!(lib.get("nonexistent").unwrap().is_none());
    }

    #[test]
    fn list_pagination() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(&tmp.path().join("lib.db")).unwrap();

        for i in 0..5 {
            let mut rec = sample_recording(
                &format!("r{i}"),
                &format!("Recording {i}"),
                None,
            );
            rec.created_at = format!("2025-01-01T00:00:{:02}Z", i);
            lib.insert(&rec).unwrap();
        }

        let page1 = lib.list(3, 0).unwrap();
        assert_eq!(page1.len(), 3);

        let page2 = lib.list(3, 3).unwrap();
        assert_eq!(page2.len(), 2);
    }

    #[test]
    fn list_empty_returns_empty_vec() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(&tmp.path().join("lib.db")).unwrap();
        let result = lib.list(10, 0).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn delete_removes_record() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(&tmp.path().join("lib.db")).unwrap();
        lib.insert(&sample_recording("d1", "To Delete", None)).unwrap();
        assert!(lib.get("d1").unwrap().is_some());

        lib.delete("d1").unwrap();
        assert!(lib.get("d1").unwrap().is_none());
    }

    #[test]
    fn search_by_title() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(&tmp.path().join("lib.db")).unwrap();
        lib.insert(&sample_recording("s1", "Epic Elden Ring Boss Fight", Some("Elden Ring"))).unwrap();
        lib.insert(&sample_recording("s2", "Minecraft Building", Some("Minecraft"))).unwrap();

        let results = lib.search("Elden").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "s1");
    }

    #[test]
    fn search_by_game_name() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(&tmp.path().join("lib.db")).unwrap();
        lib.insert(&sample_recording("g1", "Session One", Some("Baldurs Gate"))).unwrap();
        lib.insert(&sample_recording("g2", "Session Two", Some("Factorio"))).unwrap();

        let results = lib.search("Baldurs").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "g1");
    }

    #[test]
    fn search_no_matches_returns_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(&tmp.path().join("lib.db")).unwrap();
        lib.insert(&sample_recording("n1", "Some Recording", None)).unwrap();

        let results = lib.search("nonexistent_query_xyz").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn insert_same_id_replaces() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(&tmp.path().join("lib.db")).unwrap();

        let mut rec = sample_recording("dup", "Original Title", None);
        rec.file_path = "/tmp/dup_1.mkv".to_string();
        lib.insert(&rec).unwrap();

        rec.title = "Updated Title".to_string();
        rec.file_path = "/tmp/dup_2.mkv".to_string();
        lib.insert(&rec).unwrap();

        let fetched = lib.get("dup").unwrap().unwrap();
        assert_eq!(fetched.title, "Updated Title");

        // Should only be 1 record total
        let all = lib.list(100, 0).unwrap();
        assert_eq!(all.len(), 1);
    }

    #[test]
    fn parse_frame_rate_fraction() {
        assert!((parse_frame_rate("30/1") - 30.0).abs() < 0.01);
        assert!((parse_frame_rate("30000/1001") - 29.97).abs() < 0.03);
    }

    #[test]
    fn parse_frame_rate_plain_number() {
        assert!((parse_frame_rate("60") - 60.0).abs() < 0.01);
        assert!((parse_frame_rate("29.97") - 29.97).abs() < 0.01);
    }

    #[test]
    fn parse_frame_rate_invalid() {
        assert_eq!(parse_frame_rate("abc"), 0.0);
        assert_eq!(parse_frame_rate(""), 0.0);
    }

    #[test]
    fn parse_frame_rate_divide_by_zero() {
        assert_eq!(parse_frame_rate("30/0"), 0.0);
    }
}
