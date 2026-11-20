use std::collections::HashMap;
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, Position, Size, State};
use vp_core::mpv::MpvConfig;
use vp_core::profiles::ProfileInfo;
use vp_core::{scan_folder_files_pub, Folder, Library, Playlist, Rule, RuleInput, ScanReport, Tag, Video, VideoFilter, VideoPage};

use crate::indexer::{Indexer, Progress};
use crate::player::{json_f64, resolve_mpv, PlayerState};
use crate::{AppState, PipSaved};

type Res<T> = Result<T, String>;

fn e<E: std::fmt::Display>(err: E) -> String {
    err.to_string()
}

macro_rules! lib {
    ($state:expr) => {
        $state.lib.lock().map_err(|_| "database lock poisoned".to_string())?
    };
}

// ---------------------------------------------------------------- folders

#[tauri::command]
pub fn list_folders(state: State<'_, AppState>) -> Res<Vec<Folder>> {
    lib!(state).list_folders().map_err(e)
}

fn scan_one(state: &AppState, id: i64) -> Res<ScanReport> {
    // Walk the filesystem without holding the DB lock.
    let path = lib!(state).folder_path(id).map_err(e)?;
    let files = scan_folder_files_pub(&path).map_err(e)?;
    lib!(state).apply_scan(id, &files).map_err(e)
}

#[tauri::command]
pub async fn add_folder(app: AppHandle, state: State<'_, AppState>, path: String) -> Res<Folder> {
    let folder = lib!(state).add_folder(&path).map_err(e)?;
    scan_one(&state, folder.id)?;
    Indexer::start(&app); // read durations in the background
    lib!(state)
        .list_folders()
        .map_err(e)?
        .into_iter()
        .find(|f| f.id == folder.id)
        .ok_or_else(|| "folder vanished".to_string())
}

#[tauri::command]
pub fn remove_folder(state: State<'_, AppState>, id: i64) -> Res<()> {
    lib!(state).remove_folder(id).map_err(e)
}

/// Rescan every folder. Folders that are not accessible are reported in `errors`
/// but never wipe their videos.
#[tauri::command]
pub async fn rescan_all(app: AppHandle, state: State<'_, AppState>) -> Res<(ScanReport, Vec<String>)> {
    let ids = lib!(state).folder_ids().map_err(e)?;
    let mut total = ScanReport::default();
    let mut errors = Vec::new();
    for id in ids {
        match scan_one(&state, id) {
            Ok(r) => {
                total.added += r.added;
                total.removed += r.removed;
                total.total += r.total;
                total.rule_matches += r.rule_matches;
            }
            Err(msg) => errors.push(msg),
        }
    }
    Indexer::start(&app); // read durations of new/changed videos in the background
    Ok((total, errors))
}

#[tauri::command]
pub fn indexing_status(state: State<'_, AppState>) -> Progress {
    state.indexer.progress()
}

#[tauri::command]
pub fn set_default_folder(state: State<'_, AppState>, id: Option<i64>) -> Res<()> {
    lib!(state).set_default_folder(id).map_err(e)
}

#[tauri::command]
pub fn database_path(state: State<'_, AppState>) -> String {
    state.db_path.lock().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
}

// --------------------------------------------------------------- profiles

#[derive(serde::Serialize)]
pub struct ProfilesState {
    pub current: i64,
    pub profiles: Vec<ProfileInfo>,
}

fn profiles_state(state: &AppState) -> Res<ProfilesState> {
    let profiles = state.profiles.lock().map_err(|_| "profiles lock poisoned".to_string())?.list().map_err(e)?;
    let current = *state.current_profile.lock().map_err(|_| "profiles lock poisoned".to_string())?;
    Ok(ProfilesState { current, profiles })
}

macro_rules! profiles {
    ($state:expr) => {
        $state.profiles.lock().map_err(|_| "profiles lock poisoned".to_string())?
    };
}

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Res<ProfilesState> {
    profiles_state(&state)
}

#[tauri::command]
pub fn create_profile(state: State<'_, AppState>, name: String, pin: Option<String>) -> Res<ProfilesState> {
    profiles!(state).create(&name, pin.as_deref()).map_err(e)?;
    profiles_state(&state)
}

#[tauri::command]
pub fn set_profile_pin(state: State<'_, AppState>, id: i64, current_pin: Option<String>, new_pin: Option<String>) -> Res<ProfilesState> {
    profiles!(state).set_pin(id, current_pin.as_deref(), new_pin.as_deref()).map_err(e)?;
    profiles_state(&state)
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, id: i64, pin: Option<String>) -> Res<ProfilesState> {
    let _switching = state.switching.lock().map_err(|_| "profiles lock poisoned".to_string())?;
    if *state.current_profile.lock().map_err(|_| "profiles lock poisoned".to_string())? == id {
        return Err("switch to another profile before deleting this one".into());
    }
    profiles!(state).delete(id, pin.as_deref()).map_err(e)?;
    profiles_state(&state)
}

/// Load another profile's library. Playback and the background indexer belong to the old
/// profile, so both are stopped first and the indexer is restarted on the new library.
#[tauri::command]
pub async fn switch_profile(app: AppHandle, id: i64, pin: Option<String>) -> Res<ProfilesState> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let _switching = state.switching.lock().map_err(|_| "profiles lock poisoned".to_string())?;
        if *state.current_profile.lock().map_err(|_| "profiles lock poisoned".to_string())? == id {
            return profiles_state(&state);
        }
        // Check the PIN and open the new library before touching anything running.
        let path = {
            let mut profiles = profiles!(state);
            profiles.verify(id, pin.as_deref()).map_err(e)?;
            profiles.db_path(id).map_err(e)?
        };
        let new_lib = Library::open(&path).map_err(e)?;

        state.indexer.shutdown();
        state.player.stop();
        state.player.shutdown();
        let _ = app.emit("player-state", state.player.snapshot());
        *lib!(state) = new_lib;
        *state.db_path.lock().map_err(|_| "profiles lock poisoned".to_string())? = path;
        *state.current_profile.lock().map_err(|_| "profiles lock poisoned".to_string())? = id;
        let _ = app.emit("indexing-progress", state.indexer.progress());
        Indexer::start(&app); // durations/thumbnails of the new library
        profiles_state(&state)
    })
    .await
    .map_err(e)?
}

// ------------------------------------------------------------------ rules

#[tauri::command]
pub fn list_rules(state: State<'_, AppState>) -> Res<Vec<Rule>> {
    lib!(state).list_rules().map_err(e)
}

#[tauri::command]
pub fn create_rule(state: State<'_, AppState>, input: RuleInput) -> Res<Rule> {
    lib!(state).create_rule(&input).map_err(e)
}

#[tauri::command]
pub fn update_rule(state: State<'_, AppState>, id: i64, input: RuleInput) -> Res<()> {
    lib!(state).update_rule(id, &input).map_err(e)
}

#[tauri::command]
pub fn delete_rule(state: State<'_, AppState>, id: i64) -> Res<()> {
    lib!(state).delete_rule(id).map_err(e)
}

#[tauri::command]
pub async fn apply_rules_to_existing(state: State<'_, AppState>) -> Res<usize> {
    lib!(state).apply_rules_to_existing().map_err(e)
}

#[tauri::command]
pub fn count_pattern_matches(state: State<'_, AppState>, pattern: String, case_insensitive: bool) -> Res<usize> {
    lib!(state).count_pattern_matches(&pattern, case_insensitive).map_err(e)
}

// ----------------------------------------------------------------- videos

#[tauri::command]
pub fn list_videos(state: State<'_, AppState>, filter: Option<VideoFilter>) -> Res<Vec<Video>> {
    lib!(state).list_videos(&filter.unwrap_or_default()).map_err(e)
}

#[tauri::command]
pub fn list_videos_page(state: State<'_, AppState>, filter: Option<VideoFilter>, page: usize, page_size: usize) -> Res<VideoPage> {
    lib!(state).list_videos_page(&filter.unwrap_or_default(), page, page_size).map_err(e)
}

#[tauri::command]
pub fn set_rating(state: State<'_, AppState>, id: i64, rating: f64) -> Res<()> {
    lib!(state).set_rating(id, rating).map_err(e)
}

/// Watch/unwatch everything matching the filter (all pages). Returns the new state.
#[tauri::command]
pub fn toggle_watched_matching(state: State<'_, AppState>, filter: Option<VideoFilter>) -> Res<bool> {
    lib!(state).toggle_watched_matching(&filter.unwrap_or_default()).map_err(e)
}

#[tauri::command]
pub fn set_hidden(state: State<'_, AppState>, id: i64, hidden: bool) -> Res<()> {
    lib!(state).set_hidden(id, hidden).map_err(e)
}

#[tauri::command]
pub fn set_watched(state: State<'_, AppState>, id: i64, watched: bool) -> Res<()> {
    lib!(state).set_watched(id, watched).map_err(e)
}

// ------------------------------------------------------------------- tags

#[tauri::command]
pub fn list_tags(state: State<'_, AppState>) -> Res<Vec<Tag>> {
    lib!(state).list_tags().map_err(e)
}

#[tauri::command]
pub fn create_tag(state: State<'_, AppState>, name: String, color: String) -> Res<Tag> {
    lib!(state).create_tag(&name, &color).map_err(e)
}

#[tauri::command]
pub fn update_tag(state: State<'_, AppState>, id: i64, name: String, color: String) -> Res<()> {
    lib!(state).update_tag(id, &name, &color).map_err(e)
}

#[tauri::command]
pub fn delete_tag(state: State<'_, AppState>, id: i64) -> Res<()> {
    lib!(state).delete_tag(id).map_err(e)
}

#[tauri::command]
pub fn set_video_tag(state: State<'_, AppState>, video_id: i64, tag_id: i64, on: bool) -> Res<()> {
    lib!(state).set_video_tag(video_id, tag_id, on).map_err(e)
}

// -------------------------------------------------------------- playlists

#[tauri::command]
pub fn list_playlists(state: State<'_, AppState>) -> Res<Vec<Playlist>> {
    lib!(state).list_playlists().map_err(e)
}

#[tauri::command]
pub fn create_playlist(state: State<'_, AppState>, name: String) -> Res<Playlist> {
    lib!(state).create_playlist(&name).map_err(e)
}

#[tauri::command]
pub fn rename_playlist(state: State<'_, AppState>, id: i64, name: String) -> Res<()> {
    lib!(state).rename_playlist(id, &name).map_err(e)
}

#[tauri::command]
pub fn delete_playlist(state: State<'_, AppState>, id: i64) -> Res<()> {
    lib!(state).delete_playlist(id).map_err(e)
}

#[tauri::command]
pub fn add_to_playlist(state: State<'_, AppState>, playlist_id: i64, video_id: i64) -> Res<()> {
    lib!(state).add_to_playlist(playlist_id, video_id).map_err(e)
}

#[tauri::command]
pub fn remove_from_playlist(state: State<'_, AppState>, playlist_id: i64, video_id: i64) -> Res<()> {
    lib!(state).remove_from_playlist(playlist_id, video_id).map_err(e)
}

#[tauri::command]
pub fn reorder_playlist(state: State<'_, AppState>, playlist_id: i64, ordered_ids: Vec<i64>) -> Res<()> {
    lib!(state).reorder_playlist(playlist_id, &ordered_ids).map_err(e)
}

// --------------------------------------------------------------- settings

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Res<HashMap<String, String>> {
    lib!(state).all_settings().map_err(e)
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> Res<()> {
    lib!(state).set_setting(&key, &value).map_err(e)?;
    // Apply live where possible (ignore "player not running").
    if key == "volume_max" {
        if let Ok(v) = value.parse::<f64>() {
            let _ = state.player.with_client(|c| c.set_property("volume-max", json_f64(v.clamp(100.0, 1000.0))));
        }
    }
    Ok(())
}

// ----------------------------------------------------------------- player

fn setting_f64(map: &HashMap<String, String>, key: &str, default: f64) -> f64 {
    map.get(key).and_then(|v| v.parse().ok()).unwrap_or(default)
}

#[tauri::command]
pub async fn play_video(app: AppHandle, state: State<'_, AppState>, id: i64) -> Res<()> {
    let (video, settings) = {
        let lib = lib!(state);
        (lib.get_video(id).map_err(e)?, lib.all_settings().map_err(e)?)
    };
    if !std::path::Path::new(&video.path).is_file() {
        return Err(format!("file not found: {}", video.path));
    }
    let embed = settings.get("embed_video").map(|v| v == "true").unwrap_or(true);
    let volume_max = setting_f64(&settings, "volume_max", 300.0).clamp(100.0, 1000.0);
    let cfg = MpvConfig {
        binary: resolve_mpv(settings.get("mpv_path").map(String::as_str).unwrap_or("")),
        wid: if embed { *state.wid.lock().unwrap() } else { None },
        volume: setting_f64(&settings, "volume", 100.0).clamp(0.0, volume_max),
        volume_max,
        probe: false,
    };
    state.player.play(&app, id, &video.path, &cfg)
}

#[tauri::command]
pub fn player_state(state: State<'_, AppState>) -> PlayerState {
    state.player.snapshot()
}

#[tauri::command]
pub fn player_set_pause(state: State<'_, AppState>, paused: bool) -> Res<()> {
    state.player.with_client(|c| c.set_property("pause", serde_json::json!(paused)))
}

#[tauri::command]
pub fn player_seek(state: State<'_, AppState>, seconds: f64, relative: bool) -> Res<()> {
    state
        .player
        .with_client(|c| if relative { c.seek_relative(seconds) } else { c.seek_absolute(seconds) })
}

#[tauri::command]
pub fn player_set_volume(state: State<'_, AppState>, volume: f64) -> Res<()> {
    let max = lib!(state)
        .get_setting("volume_max")
        .map_err(e)?
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(300.0);
    let v = volume.clamp(0.0, max.clamp(100.0, 1000.0));
    lib!(state).set_setting("volume", &v.to_string()).map_err(e)?;
    state.player.state.lock().unwrap().volume = v;
    // The player may not be running yet; the saved value is used at spawn.
    let _ = state.player.with_client(|c| c.set_property("volume", json_f64(v)));
    Ok(())
}

#[tauri::command]
pub fn player_set_speed(state: State<'_, AppState>, speed: f64) -> Res<()> {
    let v = speed.clamp(0.25, 4.0);
    state.player.state.lock().unwrap().speed = v;
    state.player.with_client(|c| c.set_property("speed", json_f64(v)))
}

#[tauri::command]
pub fn player_set_mute(state: State<'_, AppState>, muted: bool) -> Res<()> {
    state.player.with_client(|c| c.set_property("mute", serde_json::json!(muted)))
}

#[tauri::command]
pub fn player_stop(state: State<'_, AppState>) {
    state.player.stop();
}

// ----------------------------------------------------------------- window

#[tauri::command]
pub fn set_fullscreen(app: AppHandle, on: bool) -> Res<()> {
    let main = app.get_webview_window("main").ok_or("main window missing")?;
    main.set_fullscreen(on).map_err(e)?;
    // Keybinds are handled by the overlay webview; make sure it keeps keyboard focus.
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.set_focus();
    }
    Ok(())
}

#[tauri::command]
pub fn is_fullscreen(app: AppHandle) -> bool {
    app.get_webview_window("main").and_then(|w| w.is_fullscreen().ok()).unwrap_or(false)
}

// ------------------------------------------------------- picture-in-picture

/// Logical size of the picture-in-picture window (video area), and the main window's normal minimum.
const PIP_SIZE: (f64, f64) = (480.0, 270.0);
const PIP_MIN: (f64, f64) = (240.0, 135.0);
const NORMAL_MIN: (f64, f64) = (960.0, 600.0); // keep in sync with tauri.conf.json

/// Top-left (physical px) of the picture-in-picture window: the bottom-right corner of the monitor,
/// with a margin, leaving extra room below for the title bar / taskbar.
fn pip_corner(monitor_pos: (i32, i32), monitor_size: (u32, u32), scale: f64) -> (i32, i32) {
    let (w, h, margin) = ((PIP_SIZE.0 * scale) as i32, (PIP_SIZE.1 * scale) as i32, (24.0 * scale) as i32);
    (monitor_pos.0 + monitor_size.0 as i32 - w - margin, monitor_pos.1 + monitor_size.1 as i32 - h - margin * 3)
}

/// Shrink the window into a small always-on-top player in the bottom-right corner of its monitor
/// (`on`), or restore the previous size and position. The overlay UI follows the main window.
#[tauri::command]
pub fn set_pip(app: AppHandle, state: State<'_, AppState>, on: bool) -> Res<()> {
    let main = app.get_webview_window("main").ok_or("main window missing")?;
    let overlay = app.get_webview_window("overlay");
    let mut saved = state.pip.lock().map_err(|_| "pip lock poisoned".to_string())?;
    let top = |v: bool| {
        let _ = main.set_always_on_top(v);
        if let Some(o) = &overlay {
            let _ = o.set_always_on_top(v);
        }
    };
    if on {
        if saved.is_some() {
            return Ok(());
        }
        let _ = main.set_fullscreen(false);
        let maximized = main.is_maximized().unwrap_or(false);
        if maximized {
            let _ = main.unmaximize();
        }
        *saved = Some(PipSaved {
            size: main.inner_size().map_err(e)?,
            position: main.outer_position().map_err(e)?,
            maximized,
        });
        main.set_min_size(Some(Size::Logical(LogicalSize::new(PIP_MIN.0, PIP_MIN.1)))).map_err(e)?;
        main.set_size(Size::Logical(LogicalSize::new(PIP_SIZE.0, PIP_SIZE.1))).map_err(e)?;
        if let Ok(Some(mon)) = main.current_monitor() {
            let scale = main.scale_factor().unwrap_or(1.0);
            let (mp, ms) = (mon.position(), mon.size());
            let (x, y) = pip_corner((mp.x, mp.y), (ms.width, ms.height), scale);
            let _ = main.set_position(Position::Physical(PhysicalPosition::new(x, y)));
        }
        top(true);
    } else if let Some(s) = saved.take() {
        top(false);
        main.set_min_size(Some(Size::Logical(LogicalSize::new(NORMAL_MIN.0, NORMAL_MIN.1)))).map_err(e)?;
        main.set_size(Size::Physical(PhysicalSize::new(s.size.width, s.size.height))).map_err(e)?;
        let _ = main.set_position(Position::Physical(s.position));
        if s.maximized {
            let _ = main.maximize();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn is_pip(state: State<'_, AppState>) -> bool {
    state.pip.lock().map(|p| p.is_some()).unwrap_or(false)
}

/// Copy the frame currently shown by mpv to the clipboard.
#[tauri::command]
pub async fn player_screenshot(app: AppHandle) -> Res<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        crate::screenshot::copy_frame_to_clipboard(&state.player)
    })
    .await
    .map_err(e)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pip_sits_in_the_bottom_right_corner_of_its_monitor() {
        // 1920x1080 at 100%: 480x270 window, 24px margin, 72px reserved below.
        assert_eq!(pip_corner((0, 0), (1920, 1080), 1.0), (1920 - 480 - 24, 1080 - 270 - 72));
        // A second monitor to the right keeps its own origin.
        assert_eq!(pip_corner((1920, 0), (1920, 1080), 1.0), (1920 + 1920 - 480 - 24, 1080 - 270 - 72));
        // HiDPI: everything scales with the monitor.
        assert_eq!(pip_corner((0, 0), (3840, 2160), 2.0), (3840 - 960 - 48, 2160 - 540 - 144));
        // A monitor left of / above the origin (negative coordinates) still works.
        assert_eq!(pip_corner((-1920, -200), (1920, 1080), 1.0), (-480 - 24, -200 + 1080 - 270 - 72));
    }
}

// ------------------------------------------------------------- statistics

#[tauri::command]
pub fn library_stats(state: State<'_, AppState>) -> Res<vp_core::Stats> {
    lib!(state).stats().map_err(e)
}

// ------------------------------------------------------- export / import

/// Write tag / playlist / rule definitions (never videos) to a JSON file.
#[tauri::command]
pub fn export_data(state: State<'_, AppState>, path: String, tags: bool, playlists: bool, rules: bool) -> Res<()> {
    if !(tags || playlists || rules) {
        return Err("nothing selected to export".into());
    }
    let json = lib!(state).export_data(tags, playlists, rules).map_err(e)?.to_json().map_err(e)?;
    std::fs::write(&path, json).map_err(|err| format!("cannot write '{path}': {err}"))
}

/// Merge an export file into the current library (existing items are skipped).
#[tauri::command]
pub fn import_data(state: State<'_, AppState>, path: String) -> Res<vp_core::ImportReport> {
    // An export is a few KB; refuse anything absurd instead of reading it into memory.
    let size = std::fs::metadata(&path).map_err(|err| format!("cannot read '{path}': {err}"))?.len();
    if size > 10 * 1024 * 1024 {
        return Err("that file is too large to be an export".into());
    }
    let json = std::fs::read_to_string(&path).map_err(|err| format!("cannot read '{path}': {err}"))?;
    let file = vp_core::ExportFile::parse(&json).map_err(e)?;
    lib!(state).import_data(&file).map_err(e)
}

// ------------------------------------------------------- rename / move

/// Windows locks a playing file, and elsewhere mpv would keep the old path: stop it first.
fn ensure_not_playing(state: &AppState, id: i64) -> Res<()> {
    let p = state.player.snapshot();
    if p.video_id == Some(id) && (p.loaded || !p.idle) {
        return Err("this video is playing: stop it first".into());
    }
    Ok(())
}

/// Run a planned rename/move. Same-drive changes are instant and happen under the library lock
/// (so a rescan cannot see the file half-way); copying across drives happens without it.
async fn relocate(app: &AppHandle, id: i64, plan: impl FnOnce(&Library) -> vp_core::Result<vp_core::fileops::MovePlan>) -> Res<Video> {
    let state = app.state::<AppState>();
    ensure_not_playing(&state, id)?;
    let copy_plan = {
        let lib = lib!(state);
        let plan = plan(&lib).map_err(e)?;
        if lib.apply_plan(&plan).map_err(e)? {
            None
        } else {
            Some(plan)
        }
    };
    if let Some(plan) = copy_plan {
        let p = plan.clone();
        tauri::async_runtime::spawn_blocking(move || vp_core::fileops::copy_across(&p))
            .await
            .map_err(e)?
            .map_err(e)?;
        if let Err(err) = lib!(state).commit_plan(&plan) {
            // The row could not follow: put the file back rather than orphan it.
            let _ = std::fs::copy(&plan.to, &plan.from).and_then(|_| std::fs::remove_file(&plan.to));
            return Err(e(err));
        }
    }
    let video = lib!(state).get_video(id).map_err(e);
    video
}

/// Rename a video's file (`name` is the new name without its extension).
#[tauri::command]
pub async fn rename_video(app: AppHandle, id: i64, name: String) -> Res<Video> {
    relocate(&app, id, |lib| lib.plan_rename(id, &name)).await
}

/// Move a video's file into a library folder (or a folder inside one).
#[tauri::command]
pub async fn move_video(app: AppHandle, id: i64, dir: String) -> Res<Video> {
    relocate(&app, id, |lib| lib.plan_move(id, &dir)).await
}
