<script lang="ts">
  import { onMount } from 'svelte';
  import { api, fmtTime } from '../lib/api';
  import { app, currentVideo, guarded, playNext, queueIndex, stopPlayback, togglePause, volumeMax } from '../lib/store.svelte';

  const video = $derived(currentVideo());
  const hasPrev = $derived(queueIndex() > 0);
  const hasNext = $derived(queueIndex() >= 0 && queueIndex() < app.queue.length - 1);

  let visible = $state(true);
  let hideTimer: ReturnType<typeof setTimeout>;
  let fullscreen = $state(false);
  let seeking = $state(false);
  let seekValue = $state(0);
  const shownPos = $derived(seeking ? seekValue : app.player.position);

  function poke() {
    visible = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => (visible = false), 2800);
  }

  async function setFullscreen(on: boolean) {
    await guarded(() => api.setFullscreen(on));
    fullscreen = on;
  }

  function back() {
    if (fullscreen) setFullscreen(false);
    app.playerView = false;
  }

  async function setVolume(v: number) {
    await guarded(() => api.setVolume(v));
  }

  async function commitSeek() {
    seeking = false;
    await guarded(() => api.seek(seekValue, false));
  }

  function onKey(e: KeyboardEvent) {
    if (e.target instanceof HTMLInputElement && e.target.type !== 'range') return;
    poke();
    const vol = app.player.volume;
    switch (e.key) {
      case ' ': e.preventDefault(); togglePause(); break;
      case 'ArrowLeft': guarded(() => api.seek(e.shiftKey ? -30 : -5, true)); break;
      case 'ArrowRight': guarded(() => api.seek(e.shiftKey ? 30 : 5, true)); break;
      case 'ArrowUp': e.preventDefault(); setVolume(Math.min(volumeMax(), vol + 5)); break;
      case 'ArrowDown': e.preventDefault(); setVolume(Math.max(0, vol - 5)); break;
      case 'f': case 'F': setFullscreen(!fullscreen); break;
      case 'm': case 'M': guarded(() => api.setMute(!app.player.muted)); break;
      case 'n': case 'N': playNext(1); break;
      case 'p': case 'P': playNext(-1); break;
      case 'Escape': back(); break;
    }
  }

  onMount(() => {
    poke();
    guarded(async () => (fullscreen = await api.isFullscreen()));
    return () => clearTimeout(hideTimer);
  });
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="stage" class:idle={!visible} onmousemove={poke} onclick={togglePause} ondblclick={() => setFullscreen(!fullscreen)}>
  {#if app.player.idle || (!app.player.loaded && app.player.video_id == null)}
    <div class="msg muted">Nothing is playing</div>
  {/if}

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="top bar" onclick={(e) => e.stopPropagation()} ondblclick={(e) => e.stopPropagation()}>
    <button onclick={back}>← Library</button>
    <div class="title grow">{video?.title ?? ''}</div>
  </div>

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="bottom bar" onclick={(e) => e.stopPropagation()} ondblclick={(e) => e.stopPropagation()}>
    <input class="seek" type="range" min="0" max={app.player.duration || 1} step="0.1" value={shownPos}
      oninput={(e) => { seeking = true; seekValue = Number(e.currentTarget.value); }}
      onchange={commitSeek} />
    <div class="row controls">
      <button class="ghost" disabled={!hasPrev} onclick={() => playNext(-1)} title="Previous (P)">⏮</button>
      <button class="primary round" onclick={togglePause} title="Play/Pause (Space)">{app.player.paused ? '▶' : '❚❚'}</button>
      <button class="ghost" disabled={!hasNext} onclick={() => playNext(1)} title="Next (N)">⏭</button>
      <span class="time">{fmtTime(shownPos)} / {fmtTime(app.player.duration)}</span>
      <span class="grow"></span>
      <button class="ghost" onclick={() => guarded(() => api.setMute(!app.player.muted))} title="Mute (M)">
        {app.player.muted || app.player.volume === 0 ? '🔇' : app.player.volume > 100 ? '🔊' : '🔉'}
      </button>
      <input class="vol" type="range" min="0" max={volumeMax()} step="1" value={app.player.volume}
        oninput={(e) => setVolume(Number(e.currentTarget.value))} />
      <span class="volnum" class:boost={app.player.volume > 100}>{Math.round(app.player.volume)}%</span>
      <button class="ghost" onclick={() => setFullscreen(!fullscreen)} title="Fullscreen (F)">{fullscreen ? '🗗' : '⛶'}</button>
    </div>
  </div>
</div>

<style>
  .stage { position: fixed; inset: 0; background: rgba(0, 0, 0, 0.004); } /* invisible but hit-testable */
  .stage.idle { cursor: none; }
  .msg { position: absolute; inset: 0; display: grid; place-items: center; pointer-events: none; }
  .bar { position: absolute; left: 0; right: 0; padding: 14px 20px; transition: opacity .25s; }
  .idle .bar { opacity: 0; pointer-events: none; }
  .top { top: 0; display: flex; align-items: center; gap: 14px; background: linear-gradient(#000c, transparent); padding-bottom: 36px; }
  .bottom { bottom: 0; background: linear-gradient(transparent, #000d); padding-top: 40px; }
  .title { font-size: 16px; font-weight: 600; text-shadow: 0 1px 4px #000; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .controls { gap: 8px; }
  .time { font-variant-numeric: tabular-nums; margin-left: 8px; text-shadow: 0 1px 4px #000; }
  .seek { width: 100%; margin: 0 0 8px; }
  .vol { width: 150px; }
  .volnum { width: 48px; text-align: right; font-variant-numeric: tabular-nums; }
  .volnum.boost { color: var(--primary); font-weight: 600; }
  .round { border-radius: 50%; width: 42px; height: 42px; padding: 0; }
  .bar button { background: #0009; }
  .bar button.ghost { background: transparent; }
  .bar button.primary { background: var(--primary); }
</style>
