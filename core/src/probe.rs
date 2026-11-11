//! Reads video durations through one long-lived, headless mpv process.
//!
//! mpv is already required for playback, so this adds no dependency. Reusing a
//! single process avoids paying mpv's start-up cost for every file.

use crate::mpv::{MpvClient, MpvConfig, MpvEvent};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub struct DurationProber {
    client: MpvClient,
    rx: Receiver<MpvEvent>,
    /// Give up on a single file after this long.
    pub file_timeout: Duration,
    /// After `file-loaded`, how long to wait for the duration property.
    pub grace: Duration,
}

impl DurationProber {
    pub fn spawn(binary: &str) -> Result<Self, String> {
        let (tx, rx) = channel();
        let tx = Mutex::new(tx);
        let cfg = MpvConfig { binary: binary.to_string(), wid: None, volume: 0.0, volume_max: 100.0, probe: true };
        let client = MpvClient::spawn(
            &cfg,
            Arc::new(move |e| {
                if let Ok(tx) = tx.lock() {
                    let _ = tx.send(e);
                }
            }),
        )?;
        Ok(Self::from_parts(client, rx))
    }

    pub fn from_parts(client: MpvClient, rx: Receiver<MpvEvent>) -> Self {
        DurationProber { client, rx, file_timeout: Duration::from_secs(15), grace: Duration::from_millis(1500) }
    }

    /// `Ok(Some(secs))` on success, `Ok(None)` if the file has no readable duration
    /// (corrupt/unsupported/timeout), `Err` if the mpv process is gone.
    pub fn duration_of(&mut self, path: &str) -> Result<Option<f64>, String> {
        while self.rx.try_recv().is_ok() {} // drop stale events
        self.client.load_file(path)?;
        let mut deadline = Instant::now() + self.file_timeout;
        let result = loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.rx.recv_timeout(remaining) {
                Ok(MpvEvent::Property { name, data }) if name == "duration" => {
                    if let Some(d) = data.as_f64().filter(|d| *d > 0.0) {
                        break Ok(Some(d));
                    }
                }
                Ok(MpvEvent::FileLoaded) => deadline = deadline.min(Instant::now() + self.grace),
                Ok(MpvEvent::EndFile { reason }) if reason == "error" || reason == "eof" => break Ok(None),
                Ok(MpvEvent::Closed) => break Err("mpv exited".to_string()),
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => break Ok(None),
                Err(RecvTimeoutError::Disconnected) => break Err("mpv connection lost".to_string()),
            }
        };
        if result.is_ok() {
            let _ = self.client.stop();
        }
        result
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::mpv::{connect, OBSERVED};
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::thread;

    /// Fake mpv: answers every `loadfile` with the canned reply lines for that path.
    fn prober_with_fake(replies: Vec<(&'static str, &'static str)>) -> (tempfile::TempDir, DurationProber) {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("m.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut out = stream.try_clone().unwrap();
            for line in BufReader::new(stream).lines() {
                let line = line.unwrap();
                if !line.contains("\"loadfile\"") {
                    continue;
                }
                for (needle, reply) in &replies {
                    if line.contains(needle) {
                        let _ = out.write_all(reply.as_bytes());
                    }
                }
            }
        });
        let (tx, rx) = channel();
        let tx = Mutex::new(tx);
        let (r, w) = connect(sock.to_str().unwrap()).unwrap();
        let client = MpvClient::from_connection(r, w, None, Arc::new(move |e| {
            let _ = tx.lock().unwrap().send(e);
        }))
        .unwrap();
        let _ = OBSERVED; // observed properties are subscribed by from_connection
        let mut p = DurationProber::from_parts(client, rx);
        p.file_timeout = Duration::from_millis(800);
        p.grace = Duration::from_millis(150);
        (dir, p)
    }

    #[test]
    fn reads_durations_handles_errors_and_timeouts() {
        let (_d, mut p) = prober_with_fake(vec![
            ("good.mkv", "{\"event\":\"file-loaded\"}\n{\"event\":\"property-change\",\"id\":2,\"name\":\"duration\",\"data\":123.4}\n"),
            ("late.mkv", "{\"event\":\"file-loaded\"}\n"), // loaded but never reports a duration
            ("bad.avi", "{\"event\":\"end-file\",\"reason\":\"error\"}\n"),
            // "silent.mp4": no reply at all
        ]);
        assert_eq!(p.duration_of("/v/good.mkv").unwrap(), Some(123.4));
        assert_eq!(p.duration_of("/v/bad.avi").unwrap(), None);
        assert_eq!(p.duration_of("/v/late.mkv").unwrap(), None); // ends after the grace period
        let t = Instant::now();
        assert_eq!(p.duration_of("/v/silent.mp4").unwrap(), None);
        assert!(t.elapsed() >= Duration::from_millis(700), "waited for the file timeout");
        // Still usable afterwards.
        assert_eq!(p.duration_of("/v/good.mkv").unwrap(), Some(123.4));
    }

    #[test]
    fn reports_a_dead_process() {
        let dir = tempfile::tempdir().unwrap();
        let sock = dir.path().join("m.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        thread::spawn(move || drop(listener.accept().unwrap())); // accept, then hang up
        let (tx, rx) = channel();
        let tx = Mutex::new(tx);
        let (r, w) = connect(sock.to_str().unwrap()).unwrap();
        let client = MpvClient::from_connection(r, w, None, Arc::new(move |e| {
            let _ = tx.lock().unwrap().send(e);
        }));
        // The observe_property writes may already fail once the peer is gone.
        if let Ok(client) = client {
            let mut p = DurationProber::from_parts(client, rx);
            thread::sleep(Duration::from_millis(100));
            assert!(p.duration_of("/v/a.mkv").is_err());
        }
    }
}
