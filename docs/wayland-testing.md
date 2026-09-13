# Wayland capture validation

Automated regression checks:

```sh
cargo test --workspace
# Requires ffmpeg, gst-launch-1.0 and GStreamer base/good plugins, no desktop:
cargo test --workspace -- --ignored
```

The transport tests replace the portal source with GStreamer's animated test
pattern, then exercise the production conversion, framerate negotiation, Y4M
pipe, FFmpeg encoding, decoding, graceful stop and helper cleanup on failure.
The suite also checks FFmpeg device input, failed encoder cleanup, and finite
export completion. These tests do not prove that a compositor supplies real
screen frames.

For the live check, run from a Wayland terminal and approve the desktop dialog:

```sh
cargo run -p clipforge-core --example capture_smoke -- /tmp/clipforge-screen
cargo run -p clipforge-core --example capture_smoke -- /tmp/clipforge-window --window
cargo run -p clipforge-core --example capture_smoke -- /tmp/clipforge-replay --replay
```

Each run captures eight seconds after encoding starts, using the best detected
encoder and desktop audio. Use `--silent` to disable audio. Use a new output
directory per run. Move something visible and play audio during the capture.
Check the resulting video is nonblack, motion plays at the correct speed, audio
is synchronized, and replay segments decode. For RPM, select its game window or
the monitor displaying it. Also test cancellation followed by retry, stopping
sharing from KDE, closing a shared window, and record/stop/record in the AppImage.
The sharing indicator should disappear on stop and no `gst-launch-1.0` or FFmpeg
capture child should remain after stopping or quitting.

X11 regression: run the desktop or CLI in an actual X11 session, record fullscreen
and a window, then stop and decode the output. Setting `DISPLAY` on a Wayland
session is not an X11 test.
