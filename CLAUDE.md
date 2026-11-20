# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Kadence Media: a dark/gold video library + player for Windows 11 and Debian Linux. Tauri 2 (Rust) + Svelte 5 + TypeScript, playback via the external `mpv` binary. `README.md` has user-facing docs (prerequisites, release workflow); `requests/*.md` hold the original feature briefs.

## Commands

```bash
pnpm install
pnpm tauri dev          # run the app (vite on :1420, strict port)
pnpm check              # svelte-check / TypeScript
cargo test -p vp-core      # all core tests
cargo test -p vp-core <name>   # single test, e.g. old_databases_are_migrated
make test                  # cargo test -p vp-core + pnpm check
make linux | make windows  # installers (see Makefile comments for per-OS tooling)
```

Most tests live in `core/` (db, rules, profiles, mpv, probe); `src-tauri` has a few (player state reducer, screenshot decoding, PiP placement; `cargo test -p kadence-media`) and the frontend has none, so UI changes are verified with `pnpm check` plus running the app. `scripts/smoke-app.sh` launches the built app under X11 and checks the overlay sits over the main window. Releases are built by `.github/workflows/release.yml` on `vN.N.N` tags pushed from `main`.

## Architecture

Workspace: `core` (`vp-core`, Tauri-free logic) · `src-tauri` (shell) · `src` (Svelte UI).

- **`core`**: `db.rs` is the `Library` (SQLite via rusqlite, bundled): folders, videos, tags, playlists, rules, settings, with schema migrations (see `old_databases_are_migrated`). `scan.rs` walks folders, `rules.rs` applies regex path rules (tags/playlists, additive, only to newly found videos or on explicit "apply"), `probe.rs` reads durations, `mpv.rs` is a JSON-IPC client (unix socket / Windows named pipe) for a child `mpv` process.
- **`src-tauri`**: `commands.rs` holds the `#[tauri::command]`s registered in `lib.rs`'s `generate_handler!`; `AppState` (in `lib.rs`) bundles the `Mutex<Library>`, `Player`, `Indexer` and the native `wid`. `player.rs` spawns/drives mpv, emits playback state events to the UI and marks videos watched when playback starts. `indexer.rs` is a background job (single headless mpv) that fills durations, then `thumbs.rs` makes thumbnails (ffmpeg); scans never wait on it. `host.rs` provides the native video surface.
- **Video embedding trick**: mpv is embedded in the *main* window through `--wid` (X11 XID on Linux; a child HWND on Windows). The Svelte UI runs in a second transparent, undecorated **"overlay" window** (`index.html?surface=overlay`) that `sync_overlay` keeps glued over the main window's client area; the Player page is transparent so video shows through. Consequently the app forces XWayland (`GDK_BACKEND=x11`; `VP_ALLOW_WAYLAND=1` opts out) and falls back to mpv's own window when no wid exists or "Embed video" is off.
- **Profiles**: `core/src/profiles.rs` is the registry (`profiles.sqlite3`, same folder as the default library): names, library file per profile, optional PIN (argon2 hash, 4–8 digits, rate-limited; a UI gate, not encryption; the UI offers no PIN on Default). The "Default" profile is the original `KadenceMedia.sqlite3`; others are `profile-<id>.sqlite3`. `switch_profile` (`commands.rs`) verifies the PIN, stops the indexer and mpv, swaps the `Library` inside `AppState.lib` and restarts the indexer; the frontend (`switchProfile` in the store, `ProfilesDialog.svelte`) then resets filters/queue and reloads. The app always starts on Default.
- **Database location**: `KadenceMedia.sqlite3` next to the executable (AppImage: next to the `.AppImage`), falling back to the app-data dir if not writable. In dev that is `target/debug/`, so `cargo clean` wipes the dev library.
- **Frontend**: `src/lib/api.ts` is the single typed wrapper over `invoke` (types mirror `core/src/models.rs` — keep them in sync when changing commands/models); `src/lib/store.svelte.ts` is the global Svelte 5 runes store (settings, filters, player state, event listeners); pages are in `src/pages/` (`Collection.svelte` serves both the Tags and Playlists pages via a `kind` prop), simple routing/shell in `App.svelte`. Settings are stringly-typed key/value pairs (the `Settings` type in `api.ts`).
- **i18n**: all UI text goes through `t('key', { params })` from `src/lib/i18n.svelte.ts` (use `th` for `{@html}` strings: it escapes params). Strings live in `src/lib/i18n/en.ts` (source of truth) and `it.ts` (typed `Dict`, so a missing key fails `pnpm check`); `_one`/`_other` suffixes are plurals chosen by `{ n }`. Date/time display (`fmtDateTime`) follows the `language`/`date_*`/`time_*` settings. Rust-side error messages are still English. To add a language: new dict file, register it in `dicts` and `LANGUAGES`.
