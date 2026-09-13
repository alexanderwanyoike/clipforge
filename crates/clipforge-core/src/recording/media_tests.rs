use super::*;
use crate::process::{run_ffmpeg, run_ffprobe};

async fn start_test_encoder(args: Vec<String>, helper: Child) -> Result<LinuxRecordingSession> {
    let (feed, input) = VideoFeed::new(helper)?;
    LinuxRecordingSession::start_encoder(args, CaptureResources::TestPattern(feed), input).await
}

async fn test_video_helper() -> Child {
    let mut args = crate::capture::wayland::pipeline_args(0, 30);
    // Replace only the portal source; exercise the production conversion,
    // rate negotiation and Y4M transport with animated test frames.
    let source_end = args.iter().position(|arg| arg == "!").unwrap();
    args.splice(
        1..source_end,
        [
            "videotestsrc".into(),
            "is-live=true".into(),
            "pattern=ball".into(),
        ],
    );
    args.splice(
        4..4,
        ["!".into(), "video/x-raw,width=320,height=240".into()],
    );
    crate::process::capture_command("gst-launch-1.0")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap()
}

#[tokio::test]
#[ignore = "requires FFmpeg and GStreamer base/good plugins"]
async fn wayland_transport_encodes_frames_and_stops_cleanly() {
    let temp = tempfile::tempdir().unwrap();
    let output = temp.path().join("capture.mkv");
    let mut args = crate::capture::CaptureSource::Wayland.to_ffmpeg_args();
    args.extend([
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "ultrafast".into(),
        output.to_string_lossy().into_owned(),
    ]);
    let helper = test_video_helper().await;
    let helper_pid = helper.id().unwrap();
    let mut process = start_test_encoder(args, helper).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    process.stop().await.unwrap();

    assert!(!std::path::Path::new(&format!("/proc/{helper_pid}")).exists());
    let info = run_ffprobe(&[
        "-v",
        "error",
        "-count_frames",
        "-show_entries",
        "stream=width,height,nb_read_frames",
        "-of",
        "json",
        output.to_str().unwrap(),
    ])
    .await
    .unwrap();
    let info: serde_json::Value = serde_json::from_str(&info).unwrap();
    assert_eq!(info["streams"][0]["width"], 320);
    assert!(
        info["streams"][0]["nb_read_frames"]
            .as_str()
            .unwrap()
            .parse::<u64>()
            .unwrap()
            >= 30
    );
    let decoded = run_ffmpeg(&[
        "-i",
        output.to_str().unwrap(),
        "-frames:v",
        "1",
        "-vf",
        "signalstats,metadata=print",
        "-f",
        "null",
        "-",
    ])
    .await
    .unwrap();
    let maximum = decoded
        .lines()
        .find_map(|l| l.split("lavfi.signalstats.YMAX=").nth(1))
        .unwrap();
    assert!(
        maximum.parse::<f64>().unwrap() > 100.0,
        "encoded frame must contain the test pattern"
    );
}

#[tokio::test]
#[ignore = "requires FFmpeg and GStreamer base/good plugins"]
async fn wayland_transport_replay_segments_are_decodable() {
    let temp = tempfile::tempdir().unwrap();
    let mut config = crate::config::Config::default();
    config.recording.audio_enabled = false;
    config.paths.replay_cache_dir = temp.path().to_path_buf();
    config.replay.segment_secs = 2;
    let encoder = crate::encode::hw_probe::EncoderInfo {
        name: "libx264".into(),
        hw_accel: crate::encode::hw_probe::HwAccelType::Software,
        device: None,
        available: true,
    };
    let args = crate::encode::ffmpeg::build_replay_command(
        &config,
        &encoder,
        &crate::capture::CaptureSource::Wayland,
    )
    .await;
    let mut process = start_test_encoder(args, test_video_helper().await)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    process.stop().await.unwrap();
    let segments: Vec<_> = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(|e| {
            let p = e.ok()?.path();
            (p.extension()? == "mkv").then_some(p)
        })
        .collect();
    assert!(segments.len() >= 2);
    for segment in segments {
        run_ffmpeg(&[
            "-v",
            "error",
            "-xerror",
            "-i",
            segment.to_str().unwrap(),
            "-f",
            "null",
            "-",
        ])
        .await
        .unwrap();
    }
}

#[tokio::test]
#[ignore = "requires FFmpeg and GStreamer base/good plugins"]
async fn wayland_transport_failure_cleans_up_helper() {
    let helper = test_video_helper().await;
    let helper_pid = helper.id().unwrap();
    let result = start_test_encoder(vec!["-invalid-clipforge-option".into()], helper).await;
    assert!(result.is_err());
    assert!(!std::path::Path::new(&format!("/proc/{helper_pid}")).exists());
}
