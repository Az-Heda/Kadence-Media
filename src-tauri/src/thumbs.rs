//! Thumbnail generation with ffmpeg, run by the indexer thread after durations are read.
//! ffmpeg writes one small JPEG to its stdout and it is stored in the database; the UI
//! loads it through the `thumb://` protocol (see `lib.rs`). Nothing is written to disk.

use serde::Serialize;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

const WIDTH: u32 = 480;
const FILE_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Serialize, Clone)]
pub struct ThumbReady {
    pub id: i64,
}

/// Grab one frame at `seconds` as JPEG bytes. `Ok(None)` = ffmpeg ran but produced nothing;
/// `Err` = ffmpeg could not be started (most likely not installed).
fn grab(path: &str, seconds: f64) -> Result<Option<Vec<u8>>, String> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-ss", &format!("{seconds:.2}"), "-i", path])
        .args(["-frames:v", "1", "-an", "-sn", "-vf", &format!("scale={WIDTH}:-2"), "-q:v", "4"])
        .args(["-f", "image2pipe", "-c:v", "mjpeg", "pipe:1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("cannot run ffmpeg: {e}"))?;
    let mut stdout = child.stdout.take().expect("piped");
    // Drain on a thread so a full pipe can never stall ffmpeg while we poll for the timeout.
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let start = Instant::now();
    let success = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) if start.elapsed() < FILE_TIMEOUT => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    let bytes = reader.join().unwrap_or_default();
    Ok((success && !bytes.is_empty()).then_some(bytes))
}

/// Generate thumbnails for every video that has none, until none are left or `cancel` is set.
pub fn run(app: &AppHandle, cancel: &AtomicBool) {
    let state = app.state::<AppState>();
    loop {
        if cancel.load(Ordering::SeqCst) {
            return;
        }
        let batch = match state.lib.lock().map(|l| l.videos_missing_thumb(25)) {
            Ok(Ok(b)) if !b.is_empty() => b,
            _ => return,
        };
        for (id, path, duration) in batch {
            if cancel.load(Ordering::SeqCst) {
                return;
            }
            let jpeg = if !Path::new(&path).is_file() {
                None
            } else {
                // ~10% in avoids black intros; unknown duration falls back to 5 s.
                let at = match duration {
                    Some(d) if d > 0.0 => d * 0.1,
                    _ => 5.0,
                };
                let result = match grab(&path, at) {
                    // Very short or odd files: retry from the very start.
                    Ok(None) if at > 0.0 => grab(&path, 0.0),
                    other => other,
                };
                match result {
                    Ok(bytes) => bytes,
                    Err(msg) => {
                        // ffmpeg missing: leave everything unset so it is retried next time.
                        let _ = app.emit("indexing-error", msg);
                        return;
                    }
                }
            };
            let ok = jpeg.is_some();
            if state.lib.lock().map(|l| l.set_thumb(id, jpeg.as_deref()).is_ok()).unwrap_or(false) && ok {
                let _ = app.emit("video-thumb", ThumbReady { id });
            }
        }
    }
}
