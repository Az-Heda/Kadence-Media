# Kadence Media

A dark, gold-accented video library + player for Windows 11 and Linux (Debian).
Tauri 2 (Rust) · Svelte 5 + TypeScript · mpv for playback.

## Features

**Library**
- **Folders page**: add folders; every video in them and their subfolders appears in the library (rescan drops files that disappeared; an unplugged drive never wipes its entries). Star a folder to make it the **default**: the library opens filtered to it. Removing a folder asks for confirmation, since its watched/rating/tag/playlist data is lost.
- **Grid or table view** (Settings). The table has resizable columns (widths are remembered) and columns you can show/hide and reorder by drag and drop.
- **Search, filters and sorting**: search by text; filter by watched/unwatched, tags (AND), playlist and folder; sort by full path, title, date created, size, duration or rating (or the playlist's own order). Filtering, sorting and paging all run in the backend.
- **Pagination** with a configurable page size per view (default 60 in grid, 20 in table); `←`/`→` change page.
- **Thumbnails and durations**: a background job reads durations (mpv) and generates thumbnails (ffmpeg) without blocking scans; progress shows in the sidebar.
- **Ratings**: 0.5 to 5 stars (half steps) on cards and in the table.
- **Watched tracking**: set automatically when playback starts, or toggled manually; "Toggle watched" marks everything matching the current tag/playlist filter at once.
- **Hide videos** from the right-click menu; an optional "Hidden" tab (Settings) shows them again.
- **Right-click menu** on any video: play, watched, tags & playlists, hide.

**Organization**
- **Tags** (many per video, with colors) and **playlists** (ordered, reorderable by drag and drop, played in sequence) are separate concepts, each with its own page (Playlists, then Tags): a table with name search, client-side pagination, rename, delete, "show in library" and a dialog to create new ones.
- **Automatic rules**: a regex on the video's full path applies tags and playlists to newly found videos (and on demand to existing ones), with a live match count. The Rules page has a name search, and editing a rule replaces its card in place.

**Player** (mpv embedded in the window)
- Seek bar, previous/next, stop, close, fullscreen, mute, and **volume boost** past 100% up to a configurable limit (default 300%).
- **Playback speed** menu (0.25x to 3x).
- **Playlist side panel** with thumbnails, dates and durations when playing from a playlist; the queue auto-advances.
- **Screenshot to clipboard**: pauses the video and copies the current frame; the video stays paused.
- **Picture-in-picture**: shrinks the window to a small always-on-top player in the corner of the screen, with compact controls.
- Shortcuts: `Space` pause · `←/→` seek 5s (`Shift` 30s) · `0`-`9` jump to 0%-90% · `↑/↓` volume · `M` mute · `F`/double-click fullscreen · `N`/`P` next/previous · `S` screenshot to clipboard · `I` picture-in-picture · `Esc` leaves picture-in-picture, then does the action chosen in Settings (stop, back to the library, or nothing).

**Customization**
- **Themes page**: primary color (default gold) plus background, cards, buttons, borders and text colors; save and apply named themes.
- **Languages**: English and Italian (default follows the system), plus date order, separator, 12/24-hour time, seconds and date/time order for every date shown (Settings → Language & formats).
- **Profiles**: a switcher at the bottom of the sidebar. Each profile has its own library (folders, tags, playlists, rules, settings) in a separate file; an optional 4–8 digit PIN (not available on the Default profile) is asked when switching to it, with a delay after repeated wrong attempts. The PIN only locks the profile inside the app, it does not encrypt the files. The app always starts on the Default profile.
- **Settings**: language and formats, maximum volume, library view, page sizes, hidden videos tab, table columns, Esc behavior, embed toggle, mpv and ffmpeg paths, database location.

## Architecture
```
core/        vp-core: SQLite library, profile registry, scanner, tags/playlists/settings, mpv JSON-IPC client (no Tauri; unit-tested)
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
**Both**: Rust ≥ 1.90 (`rustup update`), Node ≥ 20, `mpv`, and `ffmpeg` (thumbnails; without it the library works but shows placeholder tiles).

**Debian**
```bash
sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
  libayatana-appindicator3-dev librsvg2-dev mpv ffmpeg
```
The app forces X11/XWayland (`GDK_BACKEND=x11`) because mpv's `--wid` doesn't work on Wayland.
Set `VP_ALLOW_WAYLAND=1` to opt out (video will then open in its own mpv window). When embedding, mpv is also pinned to its X11 GPU context, since on a Wayland desktop it would otherwise ignore the embed handle.

**Windows 11**: MSVC Build Tools (C++ workload), WebView2 (preinstalled on 11), an mpv build
(e.g. https://github.com/shinchiro/mpv-winbuild-cmake/releases or `scoop install mpv`) and ffmpeg (`scoop install ffmpeg`).
For each, either put the `.exe` on `PATH`, next to `kadence-media.exe` (or in an `mpv/` / `ffmpeg/` subfolder), or set its path in Settings (Playback engine).

## Run / build
`make linux` and `make windows` each build one standalone executable, no installer (see the comments in the `Makefile` for per-OS requirements).

```bash
npm install
npm run tauri dev      # development
npm run tauri build -- --no-bundle   # executable in target/release
cargo test -p vp-core  # core tests (db, rules, profiles, mpv client, probe)
cargo test -p kadence-media  # shell tests (player state, screenshot decoding, PiP placement)
```
Data lives in SQLite files next to the executable: one library per profile plus the profile registry (see "Database" below).

## Database
Each profile has its own library file. The Default profile uses `KadenceMedia.sqlite3`; every other profile uses `profile-<id>.sqlite3`. The list of profiles (with salted PIN hashes) is in `profiles.sqlite3`. All of them are created **next to the executable** (`target/debug/` in dev, so
`make clean` / `cargo clean` deletes the dev library; AppImage: next to the `.AppImage` file).
If that folder is not writable (e.g. a `.deb` in `/usr/bin`, or an MSI under Program Files) the app falls
back to its per-user data folder. The path of the current profile's library is shown in Settings → Database.
`-wal`/`-shm` files may appear beside it while the app runs.

## Automatic rules
Rules (regex on the video's full path → tags + playlists) are applied to **newly found** videos during scans.
Use "Apply to existing videos" on the Rules page to run them on the current library (additive only).
Use `[\\/]` to match both Windows and Linux path separators.

## Releases (GitHub Actions)

`.github/workflows/release.yml` runs when a tag like `v1.0.0` (`v` + number.number.number) is pushed. It first checks that the tagged commit is on `main` (otherwise it fails and releases nothing), runs the tests, then builds two standalone executables (no installers) with `tauri build --no-bundle`:

- `kadence-media-linux-x86_64` (built on Ubuntu 22.04; run `chmod +x` after download; needs webkit2gtk 4.1, mpv and, for thumbnails, ffmpeg)
- `kadence-media-windows-x86_64.exe` (needs WebView2, mpv.exe and, for thumbnails, ffmpeg.exe)

Both are attached to a GitHub Release named after the tag. To release: `git tag v0.1.0 && git push origin v0.1.0` (from `main`).

## Known limitations
- No resume position yet: a video always starts from the beginning.
- The screenshot goes to the clipboard only (nothing is saved to disk) and captures the plain video frame, without subtitles.
- If the video doesn't show embedded on your setup, turn off "Embed video" in Settings → mpv uses its own window.
