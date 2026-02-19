use anyhow::Result;
use clap::{Parser, Subcommand};
use clipforge_core::audio::list_audio_sources;
use clipforge_core::capture::x11::create_capture_source;
use clipforge_core::config::Config;
use clipforge_core::doctor::run_diagnostics;
use clipforge_core::encode::ffmpeg::{build_recording_command, build_replay_command};
use clipforge_core::encode::hw_probe::{probe_encoders, select_best_encoder};
use clipforge_core::export::pipeline::{ExportJob, ExportPipeline};
use clipforge_core::export::presets::ExportPreset;
use clipforge_core::process::FfmpegProcess;
use clipforge_core::replay::ring::ReplayRing;
use clipforge_core::replay::save::save_replay;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "clipforge",
    version,
    about = "Linux game recording with instant replay"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Record screen
    Record {
        /// Capture mode: screen, window
        #[arg(long, default_value = "screen")]
        mode: String,

        /// Framerate
        #[arg(long, default_value = "60")]
        fps: u32,

        /// Encoder: auto, h264_vaapi, h264_nvenc, libx264
        #[arg(long, default_value = "auto")]
        encoder: String,

        /// Output file path
        #[arg(short, long)]
        out: Option<PathBuf>,
    },

    /// Start replay buffer service
    Replay {
        /// Replay buffer duration in seconds
        #[arg(long, default_value = "30")]
        seconds: u32,
    },

    /// Save last N seconds from running replay buffer
    SaveReplay {
        /// Seconds to save
        #[arg(long, default_value = "30")]
        seconds: u32,

        /// Output file
        #[arg(short, long)]
        out: Option<PathBuf>,
    },

    /// Export a recording with a preset
    Export {
        /// Input file
        #[arg(long)]
        input: PathBuf,

        /// Export preset: shorts, youtube, trailer, high_quality
        #[arg(long, default_value = "high_quality")]
        preset: String,

        /// Output file
        #[arg(short, long)]
        out: Option<PathBuf>,

        /// Trim start (seconds)
        #[arg(long)]
        trim_start: Option<f64>,

        /// Trim end (seconds)
        #[arg(long)]
        trim_end: Option<f64>,
    },

    /// List available devices (encoders, audio sources)
    Devices,

    /// Run system diagnostics
    Doctor,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "clipforge=info".into()),
        )
        .init();

    let cli = Cli::parse();
    let mut config = Config::load().unwrap_or_default();
    config.ensure_dirs()?;

    match cli.command {
        Commands::Record {
            mode: _,
            fps,
            encoder,
            out,
        } => {
            config.recording.fps = fps;

            let encoders = probe_encoders().await;
            let enc = if encoder == "auto" {
                select_best_encoder(&encoders)
            } else {
                encoders
                    .iter()
                    .find(|e| e.name == encoder)
                    .unwrap_or_else(|| select_best_encoder(&encoders))
            };

            let source = create_capture_source(&config).await?;
            let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
            let output = out.unwrap_or_else(|| PathBuf::from(format!("recording_{timestamp}.mkv")));

            let args = build_recording_command(&config, enc, &source, &output).await;
            println!("Recording to: {}", output.display());
            println!(
                "Encoder: {} | FPS: {} | Press Ctrl+C to stop",
                enc.name, fps
            );

            let mut process = FfmpegProcess::spawn(args).await?;

            // Wait for Ctrl+C
            tokio::signal::ctrl_c().await?;

            println!("\nStopping recording...");
            process.stop_graceful().await?;
            println!("Saved: {}", output.display());
        }

        Commands::Replay { seconds } => {
            config.replay.duration_secs = seconds;

            let encoders = probe_encoders().await;
            let enc = select_best_encoder(&encoders);
            let source = create_capture_source(&config).await?;

            let ring = ReplayRing::new(
                &config.paths.replay_cache_dir,
                config.replay.segment_secs,
                config.replay.max_segments,
            );
            ring.cleanup()?;

            let args = build_replay_command(&config, enc, &source).await;
            println!("Replay buffer active ({seconds} seconds)");
            println!("Press Ctrl+C to stop");

            let mut process = FfmpegProcess::spawn(args).await?;
            tokio::signal::ctrl_c().await?;

            println!("\nStopping replay buffer...");
            process.stop_graceful().await?;
            ring.cleanup()?;
        }

        Commands::SaveReplay { seconds, out } => {
            let ring = ReplayRing::new(
                &config.paths.replay_cache_dir,
                config.replay.segment_secs,
                config.replay.max_segments,
            );

            let timestamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S");
            let output = out.unwrap_or_else(|| {
                config
                    .paths
                    .replays_dir
                    .join(format!("replay_{timestamp}.mkv"))
            });

            println!("Saving last {seconds} seconds...");
            let path = save_replay(&ring, seconds, &output).await?;
            println!("Saved: {}", path.display());
        }

        Commands::Export {
            input,
            preset,
            out,
            trim_start,
            trim_end,
        } => {
            let preset_obj = match preset.as_str() {
                "shorts" => ExportPreset::shorts(),
                "youtube" => ExportPreset::youtube(),
                "trailer" => ExportPreset::trailer(),
                "high_quality" => ExportPreset::high_quality(),
                _ => anyhow::bail!(
                    "Unknown preset: {preset}. Use: shorts, youtube, trailer, high_quality"
                ),
            };

            let output = out.unwrap_or_else(|| {
                let stem = input.file_stem().unwrap_or_default().to_string_lossy();
                PathBuf::from(format!("{}_{}.{}", stem, preset, preset_obj.container))
            });

            let job = ExportJob {
                input,
                output: output.clone(),
                preset: preset_obj,
                trim_start,
                trim_end,
            };

            println!("Exporting with '{preset}' preset...");
            ExportPipeline::run(&job).await?;
            println!("Exported: {}", output.display());
        }

        Commands::Devices => {
            println!("=== Hardware Encoders ===");
            let encoders = probe_encoders().await;
            for enc in &encoders {
                let hw = if enc.is_hardware() { "HW" } else { "SW" };
                let device = enc.device.as_deref().unwrap_or("-");
                println!("  [{}] {} (device: {})", hw, enc.name, device);
            }

            println!("\n=== Audio Sources ===");
            match list_audio_sources().await {
                Ok(sources) => {
                    for source in &sources {
                        let kind = match source.source_type {
                            clipforge_core::audio::AudioSourceType::Monitor => "Desktop",
                            clipforge_core::audio::AudioSourceType::Input => "Mic",
                        };
                        println!("  [{}] {}", kind, source.name);
                    }
                }
                Err(e) => println!("  Error: {e}"),
            }
        }

        Commands::Doctor => {
            let report = run_diagnostics().await;
            println!("=== ClipForge System Diagnostics ===\n");
            for check in &report.checks {
                let icon = match check.status {
                    clipforge_core::doctor::CheckStatus::Pass => "[PASS]",
                    clipforge_core::doctor::CheckStatus::Warn => "[WARN]",
                    clipforge_core::doctor::CheckStatus::Fail => "[FAIL]",
                };
                println!("{} {}: {}", icon, check.name, check.detail);
                if let Some(ref rec) = check.recommendation {
                    println!("       -> {rec}");
                }
            }
        }
    }

    Ok(())
}
