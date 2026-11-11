//! Background job that fills in video durations after scans, one file at a time,
//! through a single headless mpv process. Scans themselves never wait for it.

use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use vp_core::probe::DurationProber;

use crate::player::resolve_mpv;
use crate::AppState;

#[derive(Default)]
pub struct Indexer {
    running: AtomicBool,
    cancel: AtomicBool,
    done: AtomicUsize,
    total: AtomicUsize,
}

#[derive(Serialize, Clone)]
pub struct Progress {
    pub running: bool,
    pub done: usize,
    pub total: usize,
}

#[derive(Serialize, Clone)]
pub struct DurationFound {
    pub id: i64,
    pub duration: f64,
}

impl Indexer {
    pub fn progress(&self) -> Progress {
        Progress {
            running: self.running.load(Ordering::SeqCst),
            done: self.done.load(Ordering::SeqCst),
            total: self.total.load(Ordering::SeqCst),
        }
    }

    /// Start indexing in the background (no-op if a run is already in progress).
    pub fn start(app: &AppHandle) {
        let state = app.state::<AppState>();
        if state.indexer.running.swap(true, Ordering::SeqCst) {
            return;
        }
        state.indexer.cancel.store(false, Ordering::SeqCst);
        let app = app.clone();
        std::thread::spawn(move || {
            run(&app);
            let ix = &app.state::<AppState>().indexer;
            ix.running.store(false, Ordering::SeqCst);
            let _ = app.emit("indexing-progress", ix.progress());
        });
    }

    /// Stop at the next file and wait (briefly) so the mpv helper is not orphaned on exit.
    pub fn shutdown(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        let start = Instant::now();
        while self.running.load(Ordering::SeqCst) && start.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

fn run(app: &AppHandle) {
    let state = app.state::<AppState>();
    let ix = &state.indexer;
    let (total, binary) = {
        let Ok(lib) = state.lib.lock() else { return };
        (
            lib.count_missing_duration().unwrap_or(0),
            resolve_mpv(&lib.get_setting("mpv_path").ok().flatten().unwrap_or_default()),
        )
    };
    if total == 0 {
        return;
    }
    ix.done.store(0, Ordering::SeqCst);
    ix.total.store(total, Ordering::SeqCst);
    let _ = app.emit("indexing-progress", ix.progress());

    let mut prober: Option<DurationProber> = None;
    let mut consecutive_errors = 0;
    'outer: loop {
        if ix.cancel.load(Ordering::SeqCst) {
            break;
        }
        let batch = match state.lib.lock().map(|l| l.videos_missing_duration(25)) {
            Ok(Ok(b)) if !b.is_empty() => b,
            _ => break,
        };
        for (id, path) in batch {
            if ix.cancel.load(Ordering::SeqCst) {
                break 'outer;
            }
            let seconds = if !Path::new(&path).is_file() {
                0.0
            } else {
                if prober.is_none() {
                    match DurationProber::spawn(&binary) {
                        Ok(p) => prober = Some(p),
                        Err(msg) => {
                            // Most likely mpv is not installed: leave durations unset so we retry later.
                            let _ = app.emit("indexing-error", msg);
                            break 'outer;
                        }
                    }
                }
                match prober.as_mut().expect("spawned above").duration_of(&path) {
                    Ok(d) => {
                        consecutive_errors = 0;
                        d.unwrap_or(0.0)
                    }
                    Err(_) => {
                        // The helper died: restart it (and give up if that keeps happening).
                        prober = None;
                        consecutive_errors += 1;
                        if consecutive_errors >= 3 {
                            break 'outer;
                        }
                        continue 'outer;
                    }
                }
            };
            if state.lib.lock().map(|l| l.set_duration(id, seconds).is_ok()).unwrap_or(false) {
                let _ = app.emit("video-duration", DurationFound { id, duration: seconds });
            }
            let done = ix.done.fetch_add(1, Ordering::SeqCst) + 1;
            if done > ix.total.load(Ordering::SeqCst) {
                ix.total.store(done, Ordering::SeqCst);
            }
            let _ = app.emit("indexing-progress", ix.progress());
        }
    }
}
