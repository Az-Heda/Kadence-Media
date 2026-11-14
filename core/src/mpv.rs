//! Minimal client for mpv's JSON IPC (`--input-ipc-server`).
//!
//! We drive the `mpv` binary as a child process instead of linking libmpv: the
//! video is still embedded in our window through `--wid`, but there is no
//! link-time dependency, which keeps Windows/Debian builds simple.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub type Reader = Box<dyn Read + Send>;
pub type Writer = Box<dyn Write + Send>;
pub type EventHandler = Arc<dyn Fn(MpvEvent) + Send + Sync>;

/// Properties we observe; mpv pushes `property-change` events for them.
pub const OBSERVED: &[&str] = &["time-pos", "duration", "pause", "volume", "mute", "idle-active", "speed"];

#[derive(Debug, Clone, PartialEq)]
pub enum MpvEvent {
    Property { name: String, data: Value },
    FileLoaded,
    EndFile { reason: String },
    /// The IPC connection closed (mpv exited or crashed).
    Closed,
}

#[derive(Debug, Clone)]
pub struct MpvConfig {
    pub binary: String,
    /// Native window id to embed the video in (None = mpv opens its own window).
    pub wid: Option<i64>,
    pub volume: f64,
    pub volume_max: f64,
    /// Headless mode (no video/audio output) used to read file metadata.
    pub probe: bool,
}

pub struct MpvClient {
    child: Option<Child>,
    writer: Arc<Mutex<Writer>>,
}

pub fn parse_event(line: &str) -> Option<MpvEvent> {
    let v: Value = serde_json::from_str(line).ok()?;
    match v.get("event")?.as_str()? {
        "property-change" => Some(MpvEvent::Property {
            name: v.get("name")?.as_str()?.to_string(),
            data: v.get("data").cloned().unwrap_or(Value::Null),
        }),
        "file-loaded" => Some(MpvEvent::FileLoaded),
        "end-file" => Some(MpvEvent::EndFile {
            reason: v.get("reason").and_then(|r| r.as_str()).unwrap_or("").to_string(),
        }),
        _ => None,
    }
}

impl MpvClient {
    /// Launch mpv (idle, embedded if `cfg.wid` is set) and connect to its IPC.
    pub fn spawn(cfg: &MpvConfig, on_event: EventHandler) -> Result<Self, String> {
        let ipc = ipc_path();
        let mut cmd = Command::new(&cfg.binary);
        if cfg.probe {
            cmd.args([
                "--idle=yes",
                "--no-terminal",
                "--no-config",
                "--load-scripts=no",
                "--vo=null",
                "--ao=null",
                "--pause=yes",
                "--cache=no",
                "--hwdec=no",
                "--sub-auto=no",
                "--audio-display=no",
                "--keep-open=no",
                "--msg-level=all=no",
            ]);
        } else {
            cmd.args([
                "--idle=yes",
                "--no-terminal",
                "--no-osc",
                "--no-osd-bar",
                "--osd-level=0",
                "--input-default-bindings=no",
                "--input-vo-keyboard=no",
                "--keep-open=no",
                "--sub-auto=fuzzy",
                "--audio-display=no",
                "--hwdec=auto-safe",
                "--title=Kadence Media",
            ]);
            cmd.arg(format!("--volume={}", cfg.volume));
            cmd.arg(format!("--volume-max={}", cfg.volume_max));
            if let Some(wid) = cfg.wid {
                cmd.arg(format!("--wid={wid}"));
                // `--wid` is an X11/Win32 handle, but on a Wayland desktop mpv probes its Wayland GPU
                // context first (it even finds the default `wayland-0` socket without WAYLAND_DISPLAY)
                // and then ignores `--wid`: the video opens in its own window. Pin the X11 context
                // so it embeds through XWayland (if it cannot start, mpv falls back to another vo).
                #[cfg(target_os = "linux")]
                cmd.arg("--gpu-context=x11egl");
            }
        }
        cmd.arg(format!("--input-ipc-server={ipc}"));
        cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("could not start mpv ('{}'): {e}. Is mpv installed?", cfg.binary))?;

        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            match connect(&ipc) {
                Ok((r, w)) => {
                    return Self::from_connection(r, w, Some(child), on_event);
                }
                Err(e) => {
                    if let Ok(Some(status)) = child.try_wait() {
                        return Err(format!("mpv exited immediately ({status}); check the mpv installation"));
                    }
                    if Instant::now() > deadline {
                        let _ = child.kill();
                        return Err(format!("could not connect to mpv IPC at {ipc}: {e}"));
                    }
                    thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }

    /// Build a client on top of an established connection (also used in tests).
    pub fn from_connection(
        reader: Reader,
        writer: Writer,
        child: Option<Child>,
        on_event: EventHandler,
    ) -> Result<Self, String> {
        let client = MpvClient { child, writer: Arc::new(Mutex::new(writer)) };
        for (i, prop) in OBSERVED.iter().enumerate() {
            client.command(&[json!("observe_property"), json!(i + 1), json!(prop)])?;
        }
        thread::spawn(move || {
            for line in BufReader::new(reader).lines() {
                let Ok(line) = line else { break };
                if let Some(ev) = parse_event(&line) {
                    on_event(ev);
                }
            }
            on_event(MpvEvent::Closed);
        });
        Ok(client)
    }

    pub fn command(&self, args: &[Value]) -> Result<(), String> {
        let mut line = serde_json::to_string(&json!({ "command": args })).map_err(|e| e.to_string())?;
        line.push('\n');
        let mut w = self.writer.lock().map_err(|_| "mpv writer poisoned".to_string())?;
        w.write_all(line.as_bytes())
            .and_then(|_| w.flush())
            .map_err(|e| format!("mpv IPC write failed: {e}"))
    }

    pub fn set_property(&self, name: &str, value: Value) -> Result<(), String> {
        self.command(&[json!("set_property"), json!(name), value])
    }

    pub fn load_file(&self, path: &str) -> Result<(), String> {
        self.command(&[json!("loadfile"), json!(path), json!("replace")])
    }

    pub fn stop(&self) -> Result<(), String> {
        self.command(&[json!("stop")])
    }

    pub fn seek_absolute(&self, seconds: f64) -> Result<(), String> {
        self.command(&[json!("seek"), json!(seconds), json!("absolute")])
    }

    pub fn seek_relative(&self, seconds: f64) -> Result<(), String> {
        self.command(&[json!("seek"), json!(seconds), json!("relative")])
    }

    /// False once the mpv process has exited.
    pub fn is_alive(&mut self) -> bool {
        match self.child.as_mut() {
            Some(c) => matches!(c.try_wait(), Ok(None)),
            None => true,
        }
    }

    pub fn kill(&mut self) {
        let _ = self.command(&[json!("quit")]);
        if let Some(mut c) = self.child.take() {
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(300));
                let _ = c.kill();
                let _ = c.wait();
            });
        }
    }
}

impl Drop for MpvClient {
    fn drop(&mut self) {
        self.kill();
    }
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

#[cfg(unix)]
fn ipc_path() -> String {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join(format!("vp-mpv-{}-{}.sock", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst)))
        .to_string_lossy()
        .into_owned()
}

#[cfg(windows)]
fn ipc_path() -> String {
    format!(r"\\.\pipe\vp-mpv-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::SeqCst))
}

#[cfg(unix)]
pub fn connect(path: &str) -> std::io::Result<(Reader, Writer)> {
    let s = std::os::unix::net::UnixStream::connect(path)?;
    let r = s.try_clone()?;
    Ok((Box::new(r), Box::new(s)))
}

#[cfg(windows)]
pub fn connect(path: &str) -> std::io::Result<(Reader, Writer)> {
    let f = std::fs::OpenOptions::new().read(true).write(true).open(path)?;
    let r = f.try_clone()?;
    Ok((Box::new(win::PollingPipeReader(r)), Box::new(f)))
}

#[cfg(windows)]
mod win {
    use std::io::{self, Read};
    use std::os::windows::io::AsRawHandle;
    use std::{fs::File, thread, time::Duration};
    use windows_sys::Win32::System::Pipes::PeekNamedPipe;

    /// A blocking `ReadFile` on a synchronous pipe handle would block `WriteFile`
    /// on the same (duplicated) handle. So only read when `PeekNamedPipe` says
    /// data is already available, and sleep briefly otherwise.
    pub struct PollingPipeReader(pub File);

    impl Read for PollingPipeReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }
            loop {
                let mut avail: u32 = 0;
                // SAFETY: valid pipe handle owned by `self.0`; only `avail` is written.
                let ok = unsafe {
                    PeekNamedPipe(
                        self.0.as_raw_handle() as _,
                        std::ptr::null_mut(),
                        0,
                        std::ptr::null_mut(),
                        &mut avail,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 {
                    // Broken pipe: mpv exited.
                    return Ok(0);
                }
                if avail > 0 {
                    let n = (avail as usize).min(buf.len());
                    return self.0.read(&mut buf[..n]);
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::sync::mpsc;

    #[test]
    fn parses_events() {
        assert_eq!(parse_event(r#"{"event":"file-loaded"}"#), Some(MpvEvent::FileLoaded));
        assert_eq!(
            parse_event(r#"{"event":"end-file","reason":"eof"}"#),
            Some(MpvEvent::EndFile { reason: "eof".into() })
        );
        assert_eq!(
            parse_event(r#"{"event":"property-change","id":1,"name":"pause","data":true}"#),
            Some(MpvEvent::Property { name: "pause".into(), data: json!(true) })
        );
        // property without data (unavailable) -> null
        assert_eq!(
            parse_event(r#"{"event":"property-change","id":2,"name":"duration"}"#),
            Some(MpvEvent::Property { name: "duration".into(), data: Value::Null })
        );
        assert_eq!(parse_event(r#"{"event":"tracks-changed"}"#), None);
        assert_eq!(parse_event(r#"{"request_id":0,"error":"success"}"#), None);
        assert_eq!(parse_event("garbage"), None);
    }

    fn read_line(r: &mut BufReader<UnixStream>) -> String {
        let mut s = String::new();
        r.read_line(&mut s).unwrap();
        s.trim().to_string()
    }

    #[test]
    fn talks_to_a_fake_mpv() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mpv.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let accept = thread::spawn(move || listener.accept().unwrap().0);

        let (tx, rx) = mpsc::channel();
        let (r, w) = connect(path.to_str().unwrap()).unwrap();
        let client = MpvClient::from_connection(
            r,
            w,
            None,
            Arc::new(move |e| {
                let _ = tx.send(e);
            }),
        )
        .unwrap();

        let mut server = accept.join().unwrap();
        let mut server_reader = BufReader::new(server.try_clone().unwrap());

        // The client must subscribe to every observed property first.
        for (i, prop) in OBSERVED.iter().enumerate() {
            let line = read_line(&mut server_reader);
            assert_eq!(line, format!(r#"{{"command":["observe_property",{},"{}"]}}"#, i + 1, prop));
        }

        // Commands are written as JSON lines.
        client.load_file("/videos/a.mkv").unwrap();
        assert_eq!(read_line(&mut server_reader), r#"{"command":["loadfile","/videos/a.mkv","replace"]}"#);
        client.set_property("volume", json!(250.0)).unwrap();
        assert_eq!(read_line(&mut server_reader), r#"{"command":["set_property","volume",250.0]}"#);
        client.seek_absolute(12.5).unwrap();
        assert_eq!(read_line(&mut server_reader), r#"{"command":["seek",12.5,"absolute"]}"#);

        // Events flow back to the handler (replies to commands are ignored).
        server
            .write_all(
                b"{\"request_id\":0,\"error\":\"success\"}\n{\"event\":\"file-loaded\"}\n\
                  {\"event\":\"property-change\",\"id\":3,\"name\":\"pause\",\"data\":false}\n\
                  {\"event\":\"end-file\",\"reason\":\"eof\"}\n",
            )
            .unwrap();
        let t = Duration::from_secs(2);
        assert_eq!(rx.recv_timeout(t).unwrap(), MpvEvent::FileLoaded);
        assert_eq!(
            rx.recv_timeout(t).unwrap(),
            MpvEvent::Property { name: "pause".into(), data: json!(false) }
        );
        assert_eq!(rx.recv_timeout(t).unwrap(), MpvEvent::EndFile { reason: "eof".into() });

        // Server going away yields Closed.
        drop(server_reader);
        drop(server);
        assert_eq!(rx.recv_timeout(t).unwrap(), MpvEvent::Closed);
        drop(client);
    }
}
