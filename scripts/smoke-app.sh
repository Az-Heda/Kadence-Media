#!/usr/bin/env bash
# Smoke test of the real app window setup: launches the built binary and checks that
#   * both windows exist ("Kadence Media" = video host, "Kadence Media UI" = the transparent overlay),
#   * the overlay is glued exactly over the main window's client area (what `sync_overlay` guarantees),
#   * the app is still alive a few seconds later (no crash while setting everything up).
# The video embedding itself (mpv --wid) is covered by core/tests/mpv_smoke.rs.
#
# Needs an X display (on a bare machine: `xvfb-run -a scripts/smoke-app.sh`) and `xwininfo` (x11-utils).
# Usage: scripts/smoke-app.sh [path/to/kadence-media]   (default: target/debug/kadence-media)
set -euo pipefail

BIN="${1:-target/debug/kadence-media}"
MAIN_TITLE="Kadence Media"
OVERLAY_TITLE="Kadence Media UI"
TOLERANCE=2   # px; window managers may round differently

[ -n "${DISPLAY:-}" ] || { echo "SKIPPED: no X display (use xvfb-run -a $0)"; exit 0; }
command -v xwininfo >/dev/null || { echo "SKIPPED: xwininfo not found (apt install x11-utils)"; exit 0; }
[ -x "$BIN" ] || { echo "FAIL: $BIN not found; build it first (cargo build -p kadence-media)"; exit 1; }

TMP="$(mktemp -d)"
APP_PID=""
cleanup() {
  [ -n "$APP_PID" ] && kill "$APP_PID" 2>/dev/null && wait "$APP_PID" 2>/dev/null || true
  rm -rf "$TMP"
}
trap cleanup EXIT

# A throwaway library: the smoke test must never touch a real one.
KADENCE_MEDIA_DB="$TMP/smoke.sqlite3" "$BIN" >"$TMP/app.log" 2>&1 &
APP_PID=$!

window_id() { xwininfo -root -tree | grep -F "\"$1\": (" | head -1 | awk '{print $1}' || true; }
# "x y w h" of a window's client area in root coordinates
geometry() {
  xwininfo -id "$1" | awk '/Absolute upper-left X:/{x=$4} /Absolute upper-left Y:/{y=$4} /Width:/{w=$2} /Height:/{h=$2} END{print x, y, w, h}'
}

main_id=""; overlay_id=""
for _ in $(seq 1 60); do
  kill -0 "$APP_PID" 2>/dev/null || { echo "FAIL: the app exited during startup"; cat "$TMP/app.log"; exit 1; }
  main_id="$(window_id "$MAIN_TITLE")"; overlay_id="$(window_id "$OVERLAY_TITLE")"
  [ -n "$main_id" ] && [ -n "$overlay_id" ] && break
  sleep 0.5
done
[ -n "$main_id" ]    || { echo "FAIL: window '$MAIN_TITLE' never appeared"; cat "$TMP/app.log"; exit 1; }
[ -n "$overlay_id" ] || { echo "FAIL: overlay window '$OVERLAY_TITLE' never appeared"; cat "$TMP/app.log"; exit 1; }
echo "windows: main=$main_id overlay=$overlay_id"

sleep 3   # let the overlay settle (it syncs on move/resize/focus events)
kill -0 "$APP_PID" 2>/dev/null || { echo "FAIL: the app crashed after startup"; cat "$TMP/app.log"; exit 1; }

read -r mx my mw mh <<<"$(geometry "$main_id")"
read -r ox oy ow oh <<<"$(geometry "$overlay_id")"
echo "main    : ${mw}x${mh} at ${mx},${my}"
echo "overlay : ${ow}x${oh} at ${ox},${oy}"

abs() { local v=$(( $1 )); echo $(( v < 0 ? -v : v )); }
for pair in "x:$mx:$ox" "y:$my:$oy" "width:$mw:$ow" "height:$mh:$oh"; do
  IFS=: read -r name a b <<<"$pair"
  if [ "$(abs "$a - $b")" -gt "$TOLERANCE" ]; then
    echo "FAIL: overlay $name ($b) does not match the main window ($a): the UI is not glued over the video"
    exit 1
  fi
done
echo "OK: overlay covers the main window's client area"
