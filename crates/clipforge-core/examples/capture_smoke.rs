//! Interactive field check: select a screen/window in the desktop portal.
use clipforge_core::{
    config::{CaptureMode, Config},
    encode::hw_probe::{probe_encoders, select_best_encoder},
    recording::{
        linux::LinuxRecordingBackend, RecordingDestination, RecordingRequest, RecordingService,
    },
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let output = std::path::PathBuf::from(args.get(1).ok_or_else(|| {
        anyhow::anyhow!("Usage: capture_smoke OUTPUT_DIR [--window] [--replay] [--silent]")
    })?);
    std::fs::create_dir_all(&output)?;
    let mut config = Config::default();
    config.recording.fps = 30;
    config.recording.audio_enabled = !args.iter().any(|a| a == "--silent");
    if args.iter().any(|a| a == "--window") {
        config.recording.capture_mode = CaptureMode::Window { id: None };
    }
    let encoders = probe_encoders().await;
    let encoder = select_best_encoder(&encoders)?.clone();
    println!(
        "Select a screen/window in the sharing dialog. Encoder: {}",
        encoder.name
    );
    let destination = if args.iter().any(|a| a == "--replay") {
        config.paths.replay_cache_dir = output.clone();
        config.replay.segment_secs = 2;
        RecordingDestination::replay(&config)
    } else {
        RecordingDestination::File(output.join("capture.mkv"))
    };
    let service = RecordingService::new(LinuxRecordingBackend);
    service
        .start(RecordingRequest {
            config,
            encoder,
            destination,
        })
        .await?;
    println!("Recording for eight seconds...");
    tokio::time::sleep(std::time::Duration::from_secs(8)).await;
    service.stop().await?;
    println!("Saved capture in {}", output.display());
    Ok(())
}
