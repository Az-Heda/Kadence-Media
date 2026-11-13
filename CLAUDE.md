# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Kadence Media: a dark/gold video library + player for Windows 11 and Debian Linux. Tauri 2 (Rust) + Svelte 5 + TypeScript, playback via the external `mpv` binary. `README.md` has user-facing docs (prerequisites, release workflow); `requests/*.md` hold the original feature briefs.

## Commands

```bash
npm install
npm run tauri dev          # run the app (vite on :1420, strict port)
npm run check              # svelte-check / TypeScript
cargo test -p vp-core      # all core tests
cargo test -p vp-core <name>   # single test, e.g. old_databases_are_migrated
make test                  # cargo test -p vp-core + npm run check
make linux | make windows  # installers (see Makefile comments for per-OS tooling)
```

All tests live in `core/` (db, rules, mpv, probe); `src-tauri` and the frontend have none, so UI/shell changes are verified with `npm run check` plus running the app. Releases are built by `.github/workflows/release.yml` on `vN.N.N` tags pushed from `main`.

## Architecture

Workspace: `core` (`vp-core`, Tauri-free logic) · `src-tauri` (shell) · `src` (Svelte UI).

- **`core`**: `db.rs` is the `Library` (SQLite via rusqlite, bundled): folders, videos, tags, playlists, rules, settings, with schema migrations (see `old_databases_are_migrated`). `scan.rs` walks folders, `rules.rs` applies regex path rules (tags/playlists, additive, only to newly found videos or on explicit "apply"), `probe.rs` reads durations, `mpv.rs` is a JSON-IPC client (unix socket / Windows named pipe) for a child `mpv` process.
- **`src-tauri`**: `commands.rs` holds the `#[tauri::command]`s registered in `lib.rs`'s `generate_handler!`; `AppState` (in `lib.rs`) bundles the `Mutex<Library>`, `Player`, `Indexer` and the native `wid`. `player.rs` spawns/drives mpv, emits playback state events to the UI and marks videos watched when playback starts. `indexer.rs` is a background job (single headless mpv) that fills durations, then `thumbs.rs` makes thumbnails (ffmpeg); scans never wait on it. `host.rs` provides the native video surface.
- **Video embedding trick**: mpv is embedded in the *main* window through `--wid` (X11 XID on Linux; a child HWND on Windows). The Svelte UI runs in a second transparent, undecorated **"overlay" window** (`index.html?surface=overlay`) that `sync_overlay` keeps glued over the main window's client area; the Player page is transparent so video shows through. Consequently the app forces XWayland (`GDK_BACKEND=x11`; `VP_ALLOW_WAYLAND=1` opts out) and falls back to mpv's own window when no wid exists or "Embed video" is off.
- **Database location**: `KadenceMedia.sqlite3` next to the executable (AppImage: next to the `.AppImage`), falling back to the app-data dir if not writable. In dev that is `target/debug/`, so `cargo clean` wipes the dev library.
- **Frontend**: `src/lib/api.ts` is the single typed wrapper over `invoke` (types mirror `core/src/models.rs` — keep them in sync when changing commands/models); `src/lib/store.svelte.ts` is the global Svelte 5 runes store (settings, filters, player state, event listeners); pages are in `src/pages/`, simple routing/shell in `App.svelte`. Settings are stringly-typed key/value pairs (the `Settings` type in `api.ts`).
