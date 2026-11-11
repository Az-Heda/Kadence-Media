mod commands;
mod host;
mod player;

use std::sync::Mutex;
use tauri::{Manager, PhysicalPosition, PhysicalSize, Position, RunEvent, Size, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent};
use vp_core::Library;

pub struct AppState {
    pub lib: Mutex<Library>,
    pub player: player::Player,
    /// Native window id mpv draws into (None => mpv opens its own window).
    pub wid: Mutex<Option<i64>>,
}

/// Keep the transparent UI overlay glued to the client area of the main window.
fn sync_overlay(main: &WebviewWindow, overlay: &WebviewWindow) {
    if main.is_minimized().unwrap_or(false) {
        let _ = overlay.hide();
        return;
    }
    let pos = main.inner_position().unwrap_or(PhysicalPosition::new(0, 0));
    let size = main.inner_size().unwrap_or(PhysicalSize::new(1280, 720));
    let _ = overlay.set_position(Position::Physical(pos));
    let _ = overlay.set_size(Size::Physical(size));
    if !overlay.is_visible().unwrap_or(true) {
        let _ = overlay.show();
    }
}

pub fn run() {
    // mpv's --wid embedding needs X11; under Wayland run through XWayland.
    #[cfg(target_os = "linux")]
    if std::env::var_os("VP_ALLOW_WAYLAND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let lib = Library::open(&data_dir.join("library.db"))?;
            app.manage(AppState {
                lib: Mutex::new(lib),
                player: player::Player::default(),
                wid: Mutex::new(None),
            });

            let main = app.get_webview_window("main").ok_or("main window missing")?;
            *app.state::<AppState>().wid.lock().unwrap() = host::video_wid(&main);

            // Transparent overlay window hosting the Svelte UI above the video.
            let size = main.inner_size()?;
            let overlay = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html?surface=overlay".into()))
                .title("Vibing Player UI")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .skip_taskbar(true)
                .resizable(false)
                .visible(false)
                .inner_size(size.width as f64, size.height as f64)
                .parent(&main)?
                .build()?;
            let _ = overlay.set_background_color(Some(tauri::window::Color(0, 0, 0, 0)));
            sync_overlay(&main, &overlay);
            let _ = overlay.show();

            let handle = app.handle().clone();
            let main2 = main.clone();
            main.on_window_event(move |event| match event {
                WindowEvent::Moved(_)
                | WindowEvent::Resized(_)
                | WindowEvent::ScaleFactorChanged { .. }
                | WindowEvent::Focused(true) => {
                    if let WindowEvent::Resized(size) = event {
                        host::resize(*handle.state::<AppState>().wid.lock().unwrap(), size.width, size.height);
                    }
                    if let Some(overlay) = handle.get_webview_window("overlay") {
                        sync_overlay(&main2, &overlay);
                    }
                }
                WindowEvent::Destroyed => handle.exit(0),
                _ => {}
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_folders,
            commands::add_folder,
            commands::remove_folder,
            commands::rescan_all,
            commands::list_videos,
            commands::set_watched,
            commands::list_tags,
            commands::create_tag,
            commands::update_tag,
            commands::delete_tag,
            commands::set_video_tag,
            commands::list_playlists,
            commands::create_playlist,
            commands::rename_playlist,
            commands::delete_playlist,
            commands::add_to_playlist,
            commands::remove_from_playlist,
            commands::reorder_playlist,
            commands::get_settings,
            commands::set_setting,
            commands::play_video,
            commands::player_state,
            commands::player_set_pause,
            commands::player_seek,
            commands::player_set_volume,
            commands::player_set_mute,
            commands::player_stop,
            commands::set_fullscreen,
            commands::is_fullscreen,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the application");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            if let Some(state) = handle.try_state::<AppState>() {
                state.player.shutdown();
            }
        }
    });
}
