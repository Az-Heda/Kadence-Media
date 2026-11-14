mod commands;
mod host;
mod indexer;
mod player;
mod screenshot;
mod thumbs;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, PhysicalPosition, PhysicalSize, Position, RunEvent, Size, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent};
use vp_core::Library;

pub struct AppState {
    pub lib: Mutex<Library>,
    pub player: player::Player,
    pub indexer: indexer::Indexer,
    /// Native window id mpv draws into (None => mpv opens its own window).
    pub wid: Mutex<Option<i64>>,
    /// Where the SQLite database actually lives (shown in Settings).
    pub db_path: PathBuf,
    /// Window geometry to restore when leaving picture-in-picture (Some = PiP is active).
    pub pip: Mutex<Option<PipSaved>>,
}

/// Main-window state saved when entering picture-in-picture.
pub struct PipSaved {
    pub size: PhysicalSize<u32>,
    pub position: PhysicalPosition<i32>,
    pub maximized: bool,
}

const DB_FILE: &str = "KadenceMedia.sqlite3";

/// Folder of the executable (for AppImage: the folder of the .AppImage file itself,
/// since the real binary runs from a read-only mount).
fn executable_dir() -> Option<PathBuf> {
    if let Some(img) = std::env::var_os("APPIMAGE") {
        if let Some(dir) = PathBuf::from(img).parent() {
            return Some(dir.to_path_buf());
        }
    }
    std::env::current_exe().ok()?.parent().map(|p| p.to_path_buf())
}

/// Open the library next to the executable; if that folder isn't writable (e.g. a
/// system-wide install in /usr/bin or Program Files) fall back to the app-data dir.
fn open_library(app: &tauri::App) -> Result<(Library, PathBuf), Box<dyn std::error::Error>> {
    if let Some(dir) = executable_dir() {
        let path = dir.join(DB_FILE);
        if let Ok(lib) = Library::open(&path) {
            return Ok((lib, path));
        }
        eprintln!("cannot write to {}, falling back to the app data directory", dir.display());
    }
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let path = data_dir.join(DB_FILE);
    Ok((Library::open(&path)?, path))
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
        // thumb://localhost/<video id> (http://thumb.localhost/<id> on Windows): JPEG from the database.
        .register_uri_scheme_protocol("thumb", |ctx, request| {
            let id = request.uri().path().trim_matches('/').parse::<i64>().ok();
            let bytes = id.and_then(|id| {
                let state = ctx.app_handle().state::<AppState>();
                let lib = state.lib.lock().ok()?;
                lib.get_thumb(id).ok().flatten()
            });
            let builder = tauri::http::Response::builder();
            match bytes {
                Some(b) => builder
                    .header("Content-Type", "image/jpeg")
                    .header("Cache-Control", "max-age=31536000")
                    .body(b),
                None => builder.status(404).body(Vec::new()),
            }
            .unwrap()
        })
        .setup(|app| {
            let (lib, db_path) = open_library(app)?;
            app.manage(AppState {
                lib: Mutex::new(lib),
                player: player::Player::default(),
                indexer: indexer::Indexer::default(),
                wid: Mutex::new(None),
                db_path,
                pip: Mutex::new(None),
            });

            let main = app.get_webview_window("main").ok_or("main window missing")?;
            *app.state::<AppState>().wid.lock().unwrap() = host::video_wid(&main);

            // Transparent overlay window hosting the Svelte UI above the video.
            let size = main.inner_size()?;
            let overlay = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("index.html?surface=overlay".into()))
                .title("Kadence Media UI")
                .decorations(false)
                .transparent(true)
                .shadow(false)
                .skip_taskbar(true)
                .resizable(false)
                // Needed for HTML5 drag-and-drop (playlist reordering) to work on Windows.
                .disable_drag_drop_handler()
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
            commands::set_default_folder,
            commands::database_path,
            commands::list_rules,
            commands::create_rule,
            commands::update_rule,
            commands::delete_rule,
            commands::apply_rules_to_existing,
            commands::count_pattern_matches,
            commands::list_videos,
            commands::list_videos_page,
            commands::toggle_watched_matching,
            commands::player_set_speed,
            commands::set_watched,
            commands::set_hidden,
            commands::set_rating,
            commands::indexing_status,
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
            commands::set_pip,
            commands::is_pip,
            commands::player_screenshot,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the application");

    app.run(|handle, event| {
        if let RunEvent::Exit = event {
            if let Some(state) = handle.try_state::<AppState>() {
                state.indexer.shutdown();
                state.player.shutdown();
            }
        }
    });
}
