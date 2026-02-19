use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("FFmpeg process failed: {0}")]
    FfmpegFailed(String),

    #[error("FFmpeg not found in PATH")]
    FfmpegNotFound,

    #[error("FFmpeg process exited with code {0}")]
    FfmpegExitCode(i32),

    #[error("FFmpeg process was killed")]
    FfmpegKilled,

    #[error("No suitable encoder found")]
    NoEncoder,

    #[error("No display found (DISPLAY env not set)")]
    NoDisplay,

    #[error("Config error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Recording already in progress")]
    AlreadyRecording,

    #[error("No recording in progress")]
    NotRecording,

    #[error("Replay buffer not active")]
    ReplayNotActive,

    #[error("No segments available for replay save")]
    NoSegments,

    #[error("Export failed: {0}")]
    ExportFailed(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_output_for_known_variants() {
        assert_eq!(Error::FfmpegNotFound.to_string(), "FFmpeg not found in PATH");
        assert_eq!(Error::NoEncoder.to_string(), "No suitable encoder found");
        assert_eq!(Error::NoSegments.to_string(), "No segments available for replay save");
        assert_eq!(
            Error::Config("bad value".into()).to_string(),
            "Config error: bad value"
        );
    }

    #[test]
    fn serialize_produces_quoted_string() {
        let err = Error::FfmpegNotFound;
        let json = serde_json::to_string(&err).unwrap();
        assert_eq!(json, "\"FFmpeg not found in PATH\"");
    }

    #[test]
    fn io_wraps_inner_error_message() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = Error::Io(io_err);
        let msg = err.to_string();
        assert!(msg.contains("file missing"), "got: {msg}");
    }
}
