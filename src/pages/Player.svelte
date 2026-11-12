<script lang="ts">
  import { onMount } from 'svelte';
  import { api, fmtDateTime, fmtTime } from '../lib/api';
  import { app, currentVideo, guarded, playNext, playQueue, queueIndex, stopPlayback, togglePause, volumeMax } from '../lib/store.svelte';
  import CircleX from '@lucide/svelte/icons/circle-x';
  import ArrowLeft from '@lucide/svelte/icons/arrow-left';
  import Play from '@lucide/svelte/icons/play';
  import Pause from '@lucide/svelte/icons/pause';
  import Square from '@lucide/svelte/icons/square';
  import SkipBack from '@lucide/svelte/icons/skip-back';
  import SkipForward from '@lucide/svelte/icons/skip-forward';
  import Volume1 from '@lucide/svelte/icons/volume-1';
  import Volume2 from '@lucide/svelte/icons/volume-2';
  import VolumeX from '@lucide/svelte/icons/volume-x';
  import Maximize from '@lucide/svelte/icons/maximize';
  import Minimize from '@lucide/svelte/icons/minimize';
  import ListVideo from '@lucide/svelte/icons/list-video';
  import X from '@lucide/svelte/icons/x';
  import Gauge from '@lucide/svelte/icons/gauge';
  import Thumb from '../components/Thumb.svelte';
  import Check from '@lucide/svelte/icons/check';

  const video = $derived(currentVideo());
  const hasPrev = $derived(queueIndex() > 0);
  const hasNext = $derived(queueIndex() >= 0 && queueIndex() < app.queue.length - 1);

  let visible = $state(true);
  let hideTimer: ReturnType<typeof setTimeout>;
  let fullscreen = $state(false);
  let seeking = $state(false);
  let seekValue = $state(0);
  const shownPos = $derived(seeking ? seekValue : app.player.position);

  // Playlist side panel (only when the queue comes from a playlist).
  let panelOpen = $state(false);
  let listEl = $state<HTMLElement>();
  const showPanel = $derived(panelOpen && app.queueTitle !== null);

  $effect(() => {
    // Keep the playing item in view when the panel opens or the video changes.
    void [showPanel, app.player.video_id];
    queueMicrotask(() => listEl?.querySelector('.current')?.scrollIntoView({ block: 'nearest' }));
  });

  // Playback speed menu (in the top bar).
  const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3];
  let speedOpen = $state(false);
  const speed = $derived(app.player.speed || 1);
  const fmtSpeed = (s: number) => `${s}x`;
  async function setSpeed(s: number) {
    speedOpen = false;
    await guarded(() => api.setSpeed(s));
  }

  function poke() {
    visible = true;
    clearTimeout(hideTimer);
    hideTimer = setTimeout(() => (visible = false), 2800);
  }

  async function setFullscreen(on: boolean) {
    await guarded(() => api.setFullscreen(on));
    fullscreen = on;
  }

  /** Hide the player but keep playing in the background. */
  async function back() {
    if (fullscreen) await setFullscreen(false);
    app.playerView = false;
  }

  /** Stop playback and return to the library. */
  async function stop() {
    if (fullscreen) await setFullscreen(false);
    await stopPlayback();
  }

  /** Esc: close the panel first; then leave fullscreen and do the configured action. */
  async function escape() {
    if (speedOpen) { speedOpen = false; return; }
    if (panelOpen) { panelOpen = false; return; }
    const action = app.settings.escape_action;
    if (action === 'stop') await stop();
    else if (action === 'back') await back();
    else if (fullscreen) await setFullscreen(false);
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
    // 0-9: jump to 0%, 10%, ... 90% of the video.
    if (/^[0-9]$/.test(e.key) && !e.ctrlKey && !e.metaKey && !e.altKey) {
      if (app.player.duration > 0) {
        guarded(() => api.seek((app.player.duration * Number(e.key)) / 10, false));
      }
      return;
    }
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
      case 'Escape': escape(); break;
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
<div class="stage" class:idle={!visible && !showPanel} onmousemove={poke} onclick={togglePause} ondblclick={() => setFullscreen(!fullscreen)}>
  {#if app.player.idle || (!app.player.loaded && app.player.video_id == null)}
    <div class="msg muted">Nothing is playing</div>
  {/if}

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="top bar" onclick={(e) => e.stopPropagation()} ondblclick={(e) => e.stopPropagation()}>
    <button class="danger" onclick={stop} title="Close the video (stops playback)"><CircleX size={16} /> Close</button>
    <button onclick={back} title="Back to the library (keeps playing)"><ArrowLeft size={16} /> Library</button>
    <div class="title grow">{video?.title ?? ''}</div>
    <div class="speed">
      <button class:on={speed !== 1} onclick={() => (speedOpen = !speedOpen)} title="Playback speed"><Gauge size={16} /> {fmtSpeed(speed)}</button>
      {#if speedOpen}
        <div class="menu" role="menu">
          {#each SPEEDS as s}
            <button class="opt" class:current={s === speed} role="menuitem" onclick={() => setSpeed(s)}>
              <span class="chk">{#if s === speed}<Check size={14} />{/if}</span>{fmtSpeed(s)}{#if s === 1}<span class="muted"> (normal)</span>{/if}
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>

  {#if showPanel}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="panel" role="presentation" onclick={(e) => e.stopPropagation()} ondblclick={(e) => e.stopPropagation()}>
      <header>
        <div class="grow">
          <div class="ptitle">{app.queueTitle}</div>
          <div class="muted small">{queueIndex() + 1} / {app.queueVideos.length}</div>
        </div>
        <button class="ghost" onclick={() => (panelOpen = false)} title="Close"><X size={16} /></button>
      </header>
      <div class="items" bind:this={listEl}>
        {#each app.queueVideos as v, i (v.id)}
          {@const current = v.id === app.player.video_id}
          <button class="item" class:current onclick={() => playQueue(app.queueVideos, i, app.queueTitle)}>
            <span class="n">{#if current}<Play size={13} fill="currentColor" />{:else}{i + 1}{/if}</span>
            <span class="pthumb"><Thumb video={v} size={20} /></span>
            <span class="meta">
              <span class="t">{v.title}</span>
              <span class="muted small">{fmtDateTime(v.modified)}{#if v.duration && v.duration > 0} · {fmtTime(v.duration)}{/if}</span>
            </span>
          </button>
        {/each}
      </div>
    </div>
  {/if}

  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="bottom bar" onclick={(e) => e.stopPropagation()} ondblclick={(e) => e.stopPropagation()}>
    <input class="seek" type="range" min="0" max={app.player.duration || 1} step="0.1" value={shownPos}
      oninput={(e) => { seeking = true; seekValue = Number(e.currentTarget.value); }}
      onchange={commitSeek} />
    <div class="row controls">
      <button class="ghost" disabled={!hasPrev} onclick={() => playNext(-1)} title="Previous (P)"><SkipBack size={18} /></button>
      <button class="primary round" onclick={togglePause} title="Play/Pause (Space)">{#if app.player.paused}<Play size={18} fill="currentColor" />{:else}<Pause size={18} fill="currentColor" />{/if}</button>
      <button class="ghost" onclick={stop} title="Stop and close the video"><Square size={17} fill="currentColor" /></button>
      <button class="ghost" disabled={!hasNext} onclick={() => playNext(1)} title="Next (N)"><SkipForward size={18} /></button>
      <span class="time">{fmtTime(shownPos)} / {fmtTime(app.player.duration)}</span>
      <span class="grow"></span>
      <button class="ghost" onclick={() => guarded(() => api.setMute(!app.player.muted))} title="Mute (M)">
        {#if app.player.muted || app.player.volume === 0}<VolumeX size={18} />{:else if app.player.volume > 100}<Volume2 size={18} />{:else}<Volume1 size={18} />{/if}
      </button>
      <input class="vol" type="range" min="0" max={volumeMax()} step="1" value={app.player.volume}
        oninput={(e) => setVolume(Number(e.currentTarget.value))} />
      <span class="volnum" class:boost={app.player.volume > 100}>{Math.round(app.player.volume)}%</span>
      {#if app.queueTitle !== null}
        <button class="ghost" class:on={panelOpen} onclick={() => (panelOpen = !panelOpen)} title="Playlist: {app.queueTitle}">
          <ListVideo size={18} />
        </button>
      {/if}
      <button class="ghost" onclick={() => setFullscreen(!fullscreen)} title="Fullscreen (F)">{#if fullscreen}<Minimize size={18} />{:else}<Maximize size={18} />{/if}</button>
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
  .bar button.ghost.on { color: var(--primary); }
  /* Icon buttons: no hover border/background, only the icon turns gold. */
  .bar button.ghost:hover:not(:disabled) { background: transparent; border-color: transparent; color: var(--primary); }
  .bar button.primary { background: var(--primary); }

  .speed { position: relative; }
  .top button.on { color: var(--primary); border-color: var(--primary); }
  .menu {
    position: absolute; right: 0; top: calc(100% + 6px); min-width: 140px; display: flex; flex-direction: column; padding: 4px;
    background: #101014f5; border: 1px solid var(--border); border-radius: 10px; box-shadow: 0 10px 30px #000a; z-index: 5;
  }
  .opt { justify-content: flex-start; background: transparent !important; border-color: transparent; }
  .opt:hover { background: var(--surface-2) !important; }
  .opt.current { color: var(--primary); }
  .chk { width: 16px; display: inline-flex; }

  .panel {
    position: absolute; right: 16px; top: 70px; bottom: 120px; width: 520px; max-width: calc(100vw - 32px); display: flex; flex-direction: column;
    background: #101014f0; border: 1px solid var(--border); border-radius: 12px; overflow: hidden; box-shadow: 0 10px 40px #000a;
  }
  .panel header { display: flex; align-items: center; gap: 8px; padding: 12px 14px; border-bottom: 1px solid var(--border); }
  .ptitle { font-weight: 700; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .small { font-size: 12px; }
  .items { overflow-y: auto; flex: 1; padding: 6px; }
  .item { display: flex; width: 100%; justify-content: flex-start; text-align: left; gap: 10px; background: transparent; border: 1px solid transparent; padding: 8px 10px; }
  .item:hover { background: var(--surface-2); }
  .item.current { background: var(--primary-soft); border-color: var(--primary); color: var(--primary); }
  .n { width: 22px; text-align: center; color: var(--muted); font-variant-numeric: tabular-nums; display: inline-flex; justify-content: center; }
  .item.current .n { color: var(--primary); }
  .pthumb { width: 150px; flex-shrink: 0; border-radius: 6px; overflow: hidden; }
  .meta { display: flex; flex-direction: column; gap: 3px; min-width: 0; flex: 1; }
  .item { align-items: center; }
  .t { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
