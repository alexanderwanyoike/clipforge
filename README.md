# ClipForge

Linux-first game recording with instant replay, export presets, and a library.

## Features

- **Screen recording** — X11 capture with hardware-accelerated encoding (VA-API, NVENC, QSV)
- **Instant replay** — ring-buffer replay saves via `/dev/shm` segments
- **Library** — FTS5-indexed recording library with thumbnails and metadata
- **Export presets** — Shorts (9:16), YouTube, Trailer, High Quality with loudnorm
- **Tray + hotkeys** — system tray and global hotkeys for start/stop/save
- **CLI** — headless recording, replay, export, and system doctor

## Requirements

- Linux / X11
- FFmpeg (with `x11grab`, `pulse` input support)
- PipeWire or PulseAudio
- Rust 1.75+
- Node 18+ and Yarn

## Quick Start

```bash
# System dependencies (Ubuntu/Debian)
sudo apt install -y ffmpeg libwebkit2gtk-4.1-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev libglib2.0-dev libgtk-3-dev \
  libappindicator3-dev librsvg2-dev patchelf

# Clone and build
git clone https://github.com/your-user/clipforge.git
cd clipforge

# Install UI dependencies
yarn --cwd ui install

# Development
cargo tauri dev

# Production build
cargo tauri build
```

## Project Structure

```
crates/
  clipforge-core/   # Recording, replay, encoding, library, export logic
  clipforge-cli/    # Headless CLI binary
src-tauri/          # Tauri desktop app shell + commands
ui/                 # React + TypeScript frontend
```

## CLI Usage

```bash
clipforge-cli record              # Start fullscreen recording
clipforge-cli replay start        # Start replay buffer
clipforge-cli replay save         # Save last N seconds from buffer
clipforge-cli export <file>       # Export with a preset
clipforge-cli doctor              # Check FFmpeg, encoders, audio
clipforge-cli devices             # List audio/video devices
```

## Configuration

Config file: `~/.config/ClipForge/config.json`

| Path | Default |
|------|---------|
| Recordings | `~/Videos/ClipForge/recordings/` |
| Replays | `~/Videos/ClipForge/replays/` |
| Exports | `~/Videos/ClipForge/exports/` |
| Replay cache | `/dev/shm/clipforge-replay/` |
| Thumbnails | `~/.cache/clipforge/thumbnails/` |

## Running Tests

```bash
cargo test --workspace            # All crates
cargo test -p clipforge-core      # Core library only
```

## License

MIT
