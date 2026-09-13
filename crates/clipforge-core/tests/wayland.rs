use clipforge_core::capture::{detect_backend, CaptureBackend, CaptureSource};

#[test]
fn wayland_takes_priority_over_xwayland_display() {
    assert_eq!(
        detect_backend(Some("wayland"), Some("wayland-0"), Some(":0")).unwrap(),
        CaptureBackend::Wayland
    );
    assert_eq!(
        detect_backend(None, Some("wayland-0"), Some(":0")).unwrap(),
        CaptureBackend::Wayland
    );
    assert_eq!(
        detect_backend(Some("wayland"), None, Some(":0")).unwrap(),
        CaptureBackend::Wayland
    );
    assert_eq!(
        detect_backend(Some("x11"), None, Some(":0")).unwrap(),
        CaptureBackend::X11
    );
    assert!(detect_backend(None, Some(""), Some("")).is_err());
}

#[test]
fn portal_video_uses_pipe_instead_of_x11() {
    let args = CaptureSource::Wayland.to_ffmpeg_args();
    assert!(args.windows(2).any(|p| p == ["-f", "yuv4mpegpipe"]));
    assert!(args.windows(2).any(|p| p == ["-i", "pipe:0"]));
    assert!(!args.iter().any(|a| a.contains("x11")));
}
