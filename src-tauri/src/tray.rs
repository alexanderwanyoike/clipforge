use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Show ClipForge", true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", "Start Recording", true, None::<&str>)?;
    let replay_toggle = MenuItem::with_id(
        app,
        "replay_toggle",
        "Enable Replay Buffer",
        true,
        None::<&str>,
    )?;
    let replay_save = MenuItem::with_id(app, "replay_save", "Save Replay", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &record, &replay_toggle, &replay_save, &quit])?;

    let _tray = TrayIconBuilder::new()
        .menu(&menu)
        .tooltip("ClipForge")
        .on_menu_event(move |app, event| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "record" => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        // Toggle recording via the command system
                        let _ = app.emit("tray-toggle-recording", ());
                    });
                }
                "replay_toggle" => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        let _ = app.emit("tray-toggle-replay", ());
                    });
                }
                "replay_save" => {
                    let app = app.clone();
                    tokio::spawn(async move {
                        let _ = app.emit("tray-save-replay", ());
                    });
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    Ok(())
}
