# Kadence Media

A dark, gold-accented video library + player for Windows 11 and Linux (Debian).
Tauri 2 (Rust) · Svelte 5 + TypeScript · mpv for playback.

## Features
- **Folders page**: add folders; every video in them and their subfolders appears in the library (rescan drops files that disappeared; an unplugged drive never wipes its entries).
- **Library**: search, filter by watched/unwatched, tags (AND), playlist, folder.
- **Tags** (many per video) and **playlists** (ordered, play-in-sequence, reorder) — separate concepts.
- **Watched tracking**: set automatically when playback starts, or toggled manually on any card.
- **Volume boost**: slider goes past 100% up to a configurable limit (default 300%, Settings page).
- **Settings**: max volume, primary color (default gold), grid/table library view, embed toggle, mpv path.
- **Default folder**: star a folder on the Folders page and the library opens filtered to it.
- Player shortcuts: `Space` pause · `←/→` seek 5s (`Shift` 30s) · `↑/↓` volume · `M` mute · `F`/double-click fullscreen · `N`/`P` next/previous · `Esc` back.

## Architecture
```
core/        vp-core: SQLite library, scanner, tags/playlists/settings, mpv JSON-IPC client (no Tauri; unit-tested)
src-tauri/   Tauri shell: commands, player bridge, native video surface
src/         Svelte UI
```
Video is drawn by **mpv embedded in the main window via `--wid`**. A second, transparent, undecorated
"overlay" window (the Svelte UI) is kept glued over the main window's client area; the player page is
transparent so the video shows through. (Same approach as OpenPlayer.)

> **Deviation from "libmpv"**: instead of linking libmpv, the app launches the `mpv` *binary* and talks to it
> over mpv's JSON IPC (unix socket / Windows named pipe). Same embedding, same features (incl. `volume-max`),
> but no link-time dependency. You therefore need an `mpv` executable at runtime.

## Prerequisites
**Both**: Rust ≥ 1.90 (`rustup update`), Node ≥ 20, `mpv`.

**Debian**
```bash
sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev mpv
```
The app forces X11/XWayland (`GDK_BACKEND=x11`) because mpv's `--wid` doesn't work on Wayland.
Set `VP_ALLOW_WAYLAND=1` to opt out (video will then open in its own mpv window).

**Windows 11**: MSVC Build Tools (C++ workload), WebView2 (preinstalled on 11), and an mpv build
(e.g. https://github.com/shinchiro/mpv-winbuild-cmake/releases or `scoop install mpv`). Either put `mpv.exe` on
`PATH`, next to `kadence-media.exe` (or in an `mpv/` subfolder), or set its path in Settings.

## Run / build
`make linux` and `make windows` build the installers (see the comments in the `Makefile` for per-OS requirements).

```bash
npm install
npm run tauri dev      # development
npm run tauri build    # installers in target/release/bundle
cargo test -p vp-core  # core tests
```
Data lives in a single SQLite file, `KadenceMedia.sqlite3`, next to the executable (see "Database" below).

## Database
One SQLite file, `KadenceMedia.sqlite3`, created **next to the executable** (`target/debug/` in dev, so
`make clean` / `cargo clean` deletes the dev library; AppImage: next to the `.AppImage` file).
If that folder is not writable (e.g. a `.deb` in `/usr/bin`, or an MSI under Program Files) the app falls
back to its per-user data folder. The path in use is shown in Settings → Database.
`-wal`/`-shm` files may appear beside it while the app runs.

## Automatic rules
Rules (regex on the video's full path → tags + playlists) are applied to **newly found** videos during scans.
Use "Apply to existing videos" on the Rules page to run them on the current library (additive only).
Use `[\\/]` to match both Windows and Linux path separators.

## Known limitations
- No thumbnails yet (placeholder tiles); no resume-position.
- If the video doesn't show embedded on your setup, turn off "Embed video" in Settings → mpv uses its own window.
