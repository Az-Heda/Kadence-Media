//! Bridges the mpv IPC client to the Tauri app: spawns mpv on demand, forwards
//! playback state to the UI, and marks videos as watched when playback starts.

use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use vp_core::mpv::{MpvClient, MpvConfig, MpvEvent};

use crate::AppState;

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
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

#[derive(Debug, Serialize, Clone, PartialEq)]
pub(crate) struct Ended {
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

/// Something the event handler must send to the UI, in order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Out {
    State(PlayerState),
    Ended(Ended),
}

/// Side effects of one mpv event (the state itself is updated in place by `apply_event`).
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Effects {
    /// A duration worth keeping in the library: (video id, seconds).
    pub save_duration: Option<(i64, f64)>,
    /// Playback started for this video: mark it as watched.
    pub mark_watched: Option<i64>,
    /// Events for the UI, in emission order.
    pub out: Vec<Out>,
}

/// Fold one mpv event into the player state and say what has to happen next.
/// `position_due` tells whether a `time-pos` update may be emitted (they arrive far more often
/// than the UI needs); the position is stored either way.
pub(crate) fn apply_event(st: &mut PlayerState, ev: MpvEvent, position_due: bool) -> Effects {
    let mut fx = Effects::default();
    match ev {
        MpvEvent::Property { name, data } => {
            match name.as_str() {
                "time-pos" => {
                    st.position = data.as_f64().unwrap_or(0.0);
                    if !position_due {
                        return fx;
                    }
                }
                "duration" => {
                    st.duration = data.as_f64().unwrap_or(0.0);
                    if st.loaded && st.duration > 0.0 {
                        fx.save_duration = st.video_id.map(|id| (id, st.duration));
                    }
                }
                "pause" => st.paused = data.as_bool().unwrap_or(false),
                "volume" => st.volume = data.as_f64().unwrap_or(st.volume),
                "speed" => st.speed = data.as_f64().unwrap_or(1.0),
                "mute" => st.muted = data.as_bool().unwrap_or(false),
                "idle-active" => st.idle = data.as_bool().unwrap_or(false),
                _ => return fx,
            }
            fx.out.push(Out::State(st.clone()));
        }
        MpvEvent::FileLoaded => {
            st.loaded = true;
            fx.mark_watched = st.video_id;
            fx.out.push(Out::State(st.clone()));
        }
        MpvEvent::EndFile { reason } => {
            st.loaded = false;
            st.position = 0.0;
            fx.out.push(Out::Ended(Ended { video_id: st.video_id, reason }));
            fx.out.push(Out::State(st.clone()));
        }
        MpvEvent::Closed => {
            let volume = st.volume;
            *st = PlayerState { volume, idle: true, ..Default::default() };
            fx.out.push(Out::State(st.clone()));
            fx.out.push(Out::Ended(Ended { video_id: None, reason: "closed".into() }));
        }
    }
    fx
}

fn make_handler(app: AppHandle, state: Arc<Mutex<PlayerState>>) -> Arc<dyn Fn(MpvEvent) + Send + Sync> {
    let last_pos_emit = Mutex::new(Instant::now() - Duration::from_secs(1));
    Arc::new(move |ev| {
        let position_due = matches!(&ev, MpvEvent::Property { name, .. } if name == "time-pos") && {
            let mut last = last_pos_emit.lock().unwrap();
            let due = last.elapsed() >= Duration::from_millis(250);
            if due {
                *last = Instant::now();
            }
            due
        };
        let fx = apply_event(&mut state.lock().unwrap(), ev, position_due);

        // Playing a video reveals its duration for free: keep it so the indexer can skip it.
        if let Some((id, duration)) = fx.save_duration {
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
        if let Some(id) = fx.mark_watched {
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
        for out in fx.out {
            let _ = match out {
                Out::State(s) => app.emit("player-state", s),
                Out::Ended(e) => app.emit("player-ended", e),
            };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn prop(name: &str, data: Value) -> MpvEvent {
        MpvEvent::Property { name: name.into(), data }
    }
    fn playing(id: i64) -> PlayerState {
        PlayerState { video_id: Some(id), loaded: true, volume: 100.0, speed: 1.0, ..Default::default() }
    }

    #[test]
    fn position_is_always_stored_but_only_emitted_when_due() {
        let mut st = playing(1);
        let fx = apply_event(&mut st, prop("time-pos", json!(12.5)), false);
        assert_eq!(st.position, 12.5);
        assert_eq!(fx, Effects::default());
        let fx = apply_event(&mut st, prop("time-pos", json!(13.0)), true);
        assert_eq!(fx.out, vec![Out::State(st.clone())]);
        assert_eq!(st.position, 13.0);
        // A null position (between files) falls back to 0.
        apply_event(&mut st, prop("time-pos", Value::Null), true);
        assert_eq!(st.position, 0.0);
    }

    #[test]
    fn simple_properties_update_the_state_and_emit_it() {
        let mut st = playing(1);
        for (name, data) in [("pause", json!(true)), ("volume", json!(250.0)), ("speed", json!(1.5)), ("mute", json!(true))] {
            let fx = apply_event(&mut st, prop(name, data), false);
            assert_eq!(fx.out, vec![Out::State(st.clone())], "{name}");
        }
        assert!(st.paused && st.muted);
        assert_eq!((st.volume, st.speed), (250.0, 1.5));
        // `volume` without data keeps the old value; `speed` without data means normal speed.
        apply_event(&mut st, prop("volume", Value::Null), false);
        apply_event(&mut st, prop("speed", Value::Null), false);
        assert_eq!((st.volume, st.speed), (250.0, 1.0));
    }

    #[test]
    fn unknown_properties_are_ignored() {
        let mut st = playing(1);
        let before = st.clone();
        assert_eq!(apply_event(&mut st, prop("something-else", json!(1)), true), Effects::default());
        assert_eq!(st, before);
    }

    #[test]
    fn duration_is_kept_only_for_a_loaded_video() {
        // Before file-loaded the duration may belong to the previous file: don't save it.
        let mut st = PlayerState { video_id: Some(7), ..Default::default() };
        assert_eq!(apply_event(&mut st, prop("duration", json!(90.0)), false).save_duration, None);
        assert_eq!(st.duration, 90.0);

        let mut st = playing(7);
        assert_eq!(apply_event(&mut st, prop("duration", json!(90.0)), false).save_duration, Some((7, 90.0)));
        assert_eq!(apply_event(&mut st, prop("duration", json!(0.0)), false).save_duration, None);
        assert_eq!(apply_event(&mut st, prop("duration", Value::Null), false).save_duration, None);

        // Loaded but no video id (should not happen, must not panic or save).
        let mut st = PlayerState { loaded: true, ..Default::default() };
        assert_eq!(apply_event(&mut st, prop("duration", json!(5.0)), false).save_duration, None);
    }

    #[test]
    fn starting_playback_marks_the_video_watched() {
        let mut st = PlayerState { video_id: Some(3), ..Default::default() };
        let fx = apply_event(&mut st, MpvEvent::FileLoaded, false);
        assert!(st.loaded);
        assert_eq!(fx.mark_watched, Some(3));
        assert_eq!(fx.out, vec![Out::State(st.clone())]);

        let mut st = PlayerState::default();
        assert_eq!(apply_event(&mut st, MpvEvent::FileLoaded, false).mark_watched, None);
    }

    #[test]
    fn end_of_file_resets_playback_and_reports_why_before_the_new_state() {
        let mut st = PlayerState { position: 55.0, ..playing(4) };
        let fx = apply_event(&mut st, MpvEvent::EndFile { reason: "eof".into() }, false);
        assert!(!st.loaded);
        assert_eq!(st.position, 0.0);
        assert_eq!(st.video_id, Some(4)); // the UI still needs it to auto-advance the queue
        assert_eq!(
            fx.out,
            vec![Out::Ended(Ended { video_id: Some(4), reason: "eof".into() }), Out::State(st.clone())]
        );
        assert_eq!(fx.mark_watched, None);
    }

    #[test]
    fn mpv_going_away_resets_everything_but_the_volume() {
        let mut st = PlayerState { position: 9.0, duration: 60.0, paused: true, muted: true, speed: 2.0, ..playing(5) };
        st.volume = 180.0;
        let fx = apply_event(&mut st, MpvEvent::Closed, false);
        assert_eq!(st, PlayerState { volume: 180.0, idle: true, ..Default::default() });
        assert_eq!(
            fx.out,
            vec![Out::State(st.clone()), Out::Ended(Ended { video_id: None, reason: "closed".into() })]
        );
    }

    #[test]
    fn a_player_that_never_started_refuses_commands_and_stops_cleanly() {
        let player = Player::default();
        let err = player.with_client(|_| Ok(())).unwrap_err();
        assert!(err.contains("not running"), "{err}");

        *player.state.lock().unwrap() = PlayerState { position: 4.0, duration: 8.0, ..playing(2) };
        player.stop();
        let st = player.snapshot();
        assert_eq!((st.video_id, st.loaded, st.position, st.duration), (None, false, 0.0, 0.0));
        player.shutdown(); // no mpv: must not panic
    }

    #[test]
    fn tool_resolution_prefers_the_setting_then_falls_back_to_path() {
        assert_eq!(resolve_tool("mpv", "/opt/mpv/bin/mpv"), "/opt/mpv/bin/mpv");
        assert_eq!(resolve_tool("mpv", "  C:\\tools\\mpv.exe  "), "C:\\tools\\mpv.exe");
        // Nothing bundled next to the test executable: the bare name is looked up in PATH.
        assert_eq!(resolve_tool("kadence-no-such-tool", ""), "kadence-no-such-tool");
        assert_eq!(resolve_mpv("   "), resolve_tool("mpv", ""));
    }
}
