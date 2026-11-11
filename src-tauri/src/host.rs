//! Native video surface for mpv's `--wid`.
//!
//! * Linux (X11): mpv embeds directly into the main window's XID.
//! * Windows: we create a child HWND covering the client area (above the
//!   WebView2 control) and give that to mpv.
//! * Wayland has no `--wid` support: the app forces XWayland (see `run`), and
//!   if no wid is available mpv falls back to its own window.

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use tauri::WebviewWindow;

/// Create/return the native id mpv should draw into.
pub fn video_wid(main: &WebviewWindow) -> Option<i64> {
    let handle = main.window_handle().ok()?;
    match handle.as_raw() {
        #[cfg(windows)]
        RawWindowHandle::Win32(h) => {
            let size = main.inner_size().ok()?;
            win::create_child(h.hwnd.get(), size.width as i32, size.height as i32).map(|c| c as i64)
        }
        #[cfg(not(windows))]
        RawWindowHandle::Xlib(h) if h.window > 0 => Some(h.window as i64),
        #[cfg(not(windows))]
        RawWindowHandle::Xcb(h) => Some(h.window.get() as i64),
        _ => None,
    }
}

/// Keep the native video surface sized to the window (no-op where mpv resizes itself).
pub fn resize(wid: Option<i64>, width: u32, height: u32) {
    #[cfg(windows)]
    if let Some(w) = wid {
        win::resize_child(w as isize, width as i32, height as i32);
    }
    #[cfg(not(windows))]
    let _ = (wid, width, height);
}

#[cfg(windows)]
mod win {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub fn create_child(parent: isize, w: i32, h: i32) -> Option<isize> {
        let class = wide("STATIC");
        let title = wide("");
        // SAFETY: plain Win32 call with a valid parent HWND from the main window.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WS_CLIPCHILDREN,
                0,
                0,
                w,
                h,
                parent as _,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if hwnd.is_null() {
            None
        } else {
            // SAFETY: hwnd was just created.
            unsafe { SetWindowPos(hwnd, HWND_TOP, 0, 0, w, h, SWP_NOACTIVATE) };
            Some(hwnd as isize)
        }
    }

    pub fn resize_child(child: isize, w: i32, h: i32) {
        // SAFETY: child is the HWND created in `create_child`.
        unsafe { SetWindowPos(child as _, HWND_TOP, 0, 0, w, h, SWP_NOACTIVATE) };
    }
}
