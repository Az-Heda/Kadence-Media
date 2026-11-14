//! Screenshot of the current video frame, copied to the system clipboard.
//!
//! mpv writes the frame to a temporary PNG (`screenshot-to-file`, which the IPC protocol
//! runs asynchronously, so we poll until the file decodes); the pixels are then handed
//! to a dedicated clipboard thread. On Linux/X11 the clipboard content only lives while
//! the process that set it keeps the `Clipboard` object alive, hence the long-lived thread.

use arboard::{Clipboard, ImageData};
use serde_json::json;
use std::borrow::Cow;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::{mpsc, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::player::Player;

type Job = (ImageData<'static>, mpsc::Sender<Result<(), String>>);

fn set_image(cb: &mut Option<Clipboard>, img: ImageData<'static>) -> Result<(), String> {
    if cb.is_none() {
        *cb = Some(Clipboard::new().map_err(|e| format!("clipboard unavailable: {e}"))?);
    }
    cb.as_mut().unwrap().set_image(img).map_err(|e| format!("could not copy to the clipboard: {e}"))
}

fn clipboard_thread() -> &'static mpsc::Sender<Job> {
    static TX: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::spawn(move || {
            let mut cb: Option<Clipboard> = None;
            for (img, reply) in rx {
                let _ = reply.send(set_image(&mut cb, img));
            }
        });
        tx
    })
}

/// Decode a PNG into RGBA8 pixels.
fn decode_png(path: &Path) -> Result<ImageData<'static>, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("image too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let px = &buf[..info.buffer_size()];
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => px.to_vec(),
        png::ColorType::Rgb => px.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => px.chunks_exact(2).flat_map(|p| [p[0], p[0], p[0], p[1]]).collect(),
        png::ColorType::Grayscale => px.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err("unexpected indexed screenshot".into()),
    };
    Ok(ImageData { width: info.width as usize, height: info.height as usize, bytes: Cow::Owned(rgba) })
}

/// Grab the frame mpv is showing right now and put it on the clipboard.
pub fn copy_frame_to_clipboard(player: &Player) -> Result<(), String> {
    if !player.snapshot().loaded {
        return Err("nothing is playing".into());
    }
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let path = std::env::temp_dir().join(format!("kadence-media-shot-{}-{nanos}.png", std::process::id()));
    let path_str = path.to_string_lossy().into_owned();
    player.with_client(|c| c.command(&[json!("screenshot-to-file"), json!(path_str), json!("video")]))?;

    let deadline = Instant::now() + Duration::from_secs(5);
    let image = loop {
        match decode_png(&path) {
            Ok(img) => break img,
            Err(err) if Instant::now() > deadline => {
                let _ = std::fs::remove_file(&path);
                return Err(format!("mpv did not produce a screenshot ({err})"));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(60)),
        }
    };
    let _ = std::fs::remove_file(&path);

    let (reply_tx, reply_rx) = mpsc::channel();
    clipboard_thread().send((image, reply_tx)).map_err(|_| "clipboard thread stopped".to_string())?;
    reply_rx.recv_timeout(Duration::from_secs(5)).map_err(|_| "clipboard did not answer".to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_png(path: &Path, color: png::ColorType, data: &[u8]) {
        let mut enc = png::Encoder::new(File::create(path).unwrap(), 2, 1);
        enc.set_color(color);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(data).unwrap();
    }

    #[test]
    fn decodes_rgb_and_rgba_screenshots_to_rgba() {
        let dir = std::env::temp_dir().join(format!("kadence-shot-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let rgb = dir.join("rgb.png");
        write_png(&rgb, png::ColorType::Rgb, &[1, 2, 3, 4, 5, 6]);
        let img = decode_png(&rgb).unwrap();
        assert_eq!((img.width, img.height), (2, 1));
        assert_eq!(&*img.bytes, &[1, 2, 3, 255, 4, 5, 6, 255]);

        let rgba = dir.join("rgba.png");
        write_png(&rgba, png::ColorType::Rgba, &[1, 2, 3, 9, 4, 5, 6, 8]);
        assert_eq!(&*decode_png(&rgba).unwrap().bytes, &[1, 2, 3, 9, 4, 5, 6, 8]);

        // A missing / half-written file is an error (the caller keeps polling).
        assert!(decode_png(&dir.join("missing.png")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
