//! Bridges the mpv IPC client to the Tauri app: spawns mpv on demand, forwards
//! playback state to the UI, and marks videos as watched when playback starts.

use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use vp_core::mpv::{MpvClient, MpvConfig, MpvEvent};

use crate::AppState;

#[derive(Debug, Clone, Default, Serialize)]
pub struct PlayerState {
    pub video_id: Option<i64>,
    pub loaded: bool,
    pub idle: bool,
    pub paused: bool,
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub muted: bool,
    pub speed: f64,
}

#[derive(Serialize, Clone)]
struct Ended {
    video_id: Option<i64>,
    reason: String,
}

#[derive(Serialize, Clone)]
struct WatchedChanged {
    id: i64,
    watched: bool,
}

#[derive(Default)]
pub struct Player {
    client: Mutex<Option<MpvClient>>,
    pub state: Arc<Mutex<PlayerState>>,
}

impl Player {
    pub fn snapshot(&self) -> PlayerState {
        self.state.lock().unwrap().clone()
    }

    /// Start playing `path` (spawning mpv first if needed).
    pub fn play(&self, app: &AppHandle, video_id: i64, path: &str, cfg: &MpvConfig) -> Result<(), String> {
        let mut guard = self.client.lock().map_err(|_| "player lock poisoned".to_string())?;
        let alive = guard.as_mut().map(|c| c.is_alive()).unwrap_or(false);
        if !alive {
            *guard = None;
            let handler = make_handler(app.clone(), self.state.clone());
            *guard = Some(MpvClient::spawn(cfg, handler)?);
        }
        let client = guard.as_ref().unwrap();
        {
            let mut st = self.state.lock().unwrap();
            st.video_id = Some(video_id);
            st.position = 0.0;
            st.duration = 0.0;
        }
        client.set_property("pause", json!(false))?;
        client.load_file(path)
    }

    /// Run `f` against the running mpv, or fail if nothing is playing yet.
    pub fn with_client<T>(&self, f: impl FnOnce(&MpvClient) -> Result<T, String>) -> Result<T, String> {
        let guard = self.client.lock().map_err(|_| "player lock poisoned".to_string())?;
        match guard.as_ref() {
            Some(c) => f(c),
            None => Err("the player is not running".into()),
        }
    }

    pub fn stop(&self) {
        let _ = self.with_client(|c| c.stop());
        let mut st = self.state.lock().unwrap();
        st.video_id = None;
        st.loaded = false;
        st.position = 0.0;
        st.duration = 0.0;
    }

    pub fn shutdown(&self) {
        if let Ok(mut g) = self.client.lock() {
            *g = None; // Drop kills mpv
        }
    }
}

fn make_handler(app: AppHandle, state: Arc<Mutex<PlayerState>>) -> Arc<dyn Fn(MpvEvent) + Send + Sync> {
    let last_pos_emit = Mutex::new(Instant::now() - Duration::from_secs(1));
    Arc::new(move |ev| match ev {
        MpvEvent::Property { name, data } => {
            let mut found: Option<(i64, f64)> = None;
            let snapshot = {
                let mut st = state.lock().unwrap();
                match name.as_str() {
                    "time-pos" => {
                        st.position = data.as_f64().unwrap_or(0.0);
                        let mut last = last_pos_emit.lock().unwrap();
                        if last.elapsed() < Duration::from_millis(250) {
                            return;
                        }
                        *last = Instant::now();
                    }
                    "duration" => {
                        st.duration = data.as_f64().unwrap_or(0.0);
                        if st.loaded && st.duration > 0.0 {
                            found = st.video_id.map(|id| (id, st.duration));
                        }
                    }
                    "pause" => st.paused = data.as_bool().unwrap_or(false),
                    "volume" => st.volume = data.as_f64().unwrap_or(st.volume),
                    "speed" => st.speed = data.as_f64().unwrap_or(1.0),
                    "mute" => st.muted = data.as_bool().unwrap_or(false),
                    "idle-active" => st.idle = data.as_bool().unwrap_or(false),
                    _ => return,
                }
                st.clone()
            };
            // Playing a video reveals its duration for free: keep it so the indexer can skip it.
            if let Some((id, duration)) = found {
                let saved = app
                    .state::<AppState>()
                    .lib
                    .lock()
                    .map(|lib| lib.set_duration(id, duration).is_ok())
                    .unwrap_or(false);
                if saved {
                    let _ = app.emit("video-duration", crate::indexer::DurationFound { id, duration });
                }
            }
            let _ = app.emit("player-state", snapshot);
        }
        MpvEvent::FileLoaded => {
            let (snapshot, id) = {
                let mut st = state.lock().unwrap();
                st.loaded = true;
                (st.clone(), st.video_id)
            };
            // Playback started: mark the video as watched.
            if let Some(id) = id {
                let marked = app
                    .state::<AppState>()
                    .lib
                    .lock()
                    .map(|lib| lib.set_watched(id, true).is_ok())
                    .unwrap_or(false);
                if marked {
                    let _ = app.emit("watched-changed", WatchedChanged { id, watched: true });
                }
            }
            let _ = app.emit("player-state", snapshot);
        }
        MpvEvent::EndFile { reason } => {
            let (snapshot, id) = {
                let mut st = state.lock().unwrap();
                st.loaded = false;
                st.position = 0.0;
                (st.clone(), st.video_id)
            };
            let _ = app.emit("player-ended", Ended { video_id: id, reason });
            let _ = app.emit("player-state", snapshot);
        }
        MpvEvent::Closed => {
            let snapshot = {
                let mut st = state.lock().unwrap();
                let volume = st.volume;
                *st = PlayerState { volume, idle: true, ..Default::default() };
                st.clone()
            };
            let _ = app.emit("player-state", snapshot);
            let _ = app.emit("player-ended", Ended { video_id: None, reason: "closed".into() });
        }
    })
}

/// Pick the mpv executable (see `resolve_tool`).
pub fn resolve_mpv(setting: &str) -> String {
    resolve_tool("mpv", setting)
}

/// Pick an executable: explicit setting, then one bundled next to our
/// executable (handy on Windows), then the bare name from PATH.
pub fn resolve_tool(tool: &str, setting: &str) -> String {
    if !setting.trim().is_empty() {
        return setting.trim().to_string();
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let name = if cfg!(windows) { format!("{tool}.exe") } else { tool.to_string() };
            for cand in [dir.join(&name), dir.join(tool).join(&name)] {
                if cand.is_file() {
                    return cand.to_string_lossy().into_owned();
                }
            }
        }
    }
    tool.to_string()
}

pub fn json_f64(v: f64) -> Value {
    json!(v)
}
