//! Host executable environment and child lifetime policy.
use tokio::process::Command;

/// Use host multimedia tools with host libraries, even from an AppImage which
/// prepends its own (potentially incompatible) GLib/GStreamer libraries/plugins.
pub(crate) fn host_command(program: &str) -> Command {
    let mut command = Command::new(program);
    command.kill_on_drop(true);
    if std::env::var_os("APPIMAGE").is_some() || std::env::var_os("APPDIR").is_some() {
        for key in [
            "LD_LIBRARY_PATH",
            "LD_PRELOAD",
            "GST_PLUGIN_PATH",
            "GST_PLUGIN_PATH_1_0",
            "GST_PLUGIN_SYSTEM_PATH",
            "GST_PLUGIN_SYSTEM_PATH_1_0",
            "GST_PLUGIN_SCANNER",
            "GST_PLUGIN_SCANNER_1_0",
        ] {
            command.env_remove(key);
        }
        command.env("PATH", "/usr/local/bin:/usr/bin:/bin");
    }
    command
}

/// Capture children must not survive an abrupt desktop-app exit. FFmpeg handles
/// SIGINT by flushing its encoder/trailer; PipeWire access dies with the helper.
pub(crate) fn capture_command(program: &str) -> Command {
    let mut command = host_command(program);
    let parent = std::process::id();
    // SAFETY: the post-fork callback only invokes async-signal-safe Linux calls.
    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGINT) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::getppid() as u32 != parent {
                libc::_exit(1);
            }
            Ok(())
        });
    }
    command
}
