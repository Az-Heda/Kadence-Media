use std::collections::HashMap;
use tauri::{AppHandle, Manager, State};
use vp_core::mpv::MpvConfig;
use vp_core::{scan_folder_files_pub, Folder, Playlist, Rule, RuleInput, ScanReport, Tag, Video, VideoFilter, VideoPage};

use crate::indexer::{Indexer, Progress};
use crate::player::{json_f64, resolve_mpv, PlayerState};
use crate::AppState;

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
    state.db_path.to_string_lossy().into_owned()
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
    main.set_fullscreen(on).map_err(e)
}

#[tauri::command]
pub fn is_fullscreen(app: AppHandle) -> bool {
    app.get_webview_window("main").and_then(|w| w.is_fullscreen().ok()).unwrap_or(false)
}
