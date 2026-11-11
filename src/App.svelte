<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { api, type PlayerState } from './lib/api';
  import { app, loadSettings, refreshAll, playNext, queueIndex, guarded, toast, type Page } from './lib/store.svelte';
  import Library from './pages/Library.svelte';
  import Folders from './pages/Folders.svelte';
  import Collections from './pages/Collections.svelte';
  import Settings from './pages/Settings.svelte';
  import Player from './pages/Player.svelte';
  import MiniPlayer from './components/MiniPlayer.svelte';

  const surface = new URLSearchParams(location.search).get('surface') ?? 'overlay';

  const pages: { id: Page; label: string; icon: string }[] = [
    { id: 'library', label: 'Library', icon: '▦' },
    { id: 'folders', label: 'Folders', icon: '🗀' },
    { id: 'collections', label: 'Tags & Playlists', icon: '☰' },
    { id: 'settings', label: 'Settings', icon: '⚙' },
  ];

  $effect(() => {
    document.body.classList.toggle('host', surface === 'host');
    document.body.classList.toggle('transparent', surface === 'overlay' && app.playerView);
  });

  onMount(() => {
    if (surface !== 'overlay') return;
    guarded(async () => {
      await loadSettings();
      await refreshAll();
      app.player = await api.playerState();
      // Pick up files added/removed while the app was closed (errors only, to avoid noise).
      const [, errors] = await api.rescanAll();
      if (errors.length) toast(errors.join('\n'), true);
      await refreshAll();
    });
    const unlisten = [
      listen<PlayerState>('player-state', (e) => (app.player = e.payload)),
      listen<{ id: number; watched: boolean }>('watched-changed', (e) => {
        const v = app.videos.find((x) => x.id === e.payload.id);
        if (v) v.watched = e.payload.watched;
        if (app.filter.watched !== null) refreshAll();
      }),
      listen<{ video_id: number | null; reason: string }>('player-ended', (e) => {
        // Auto-advance through the queue (playlist playback) when a video finishes.
        if (e.payload.reason === 'eof' && e.payload.video_id != null) {
          const i = app.queue.indexOf(e.payload.video_id);
          if (i >= 0 && i + 1 < app.queue.length) playNext(1);
        }
      }),
    ];
    return () => unlisten.forEach((p) => p.then((f) => f()));
  });
</script>

{#if surface === 'overlay'}
  {#if app.playerView}
    <Player />
  {:else}
    <div class="shell">
      <nav>
        <div class="brand"><span class="logo">▶</span> Vibing</div>
        {#each pages as p}
          <button class="nav ghost" class:active={app.page === p.id} onclick={() => (app.page = p.id)}>
            <span class="icon">{p.icon}</span>{p.label}
          </button>
        {/each}
      </nav>
      <div class="content">
        <main>
          {#if app.page === 'library'}<Library />
          {:else if app.page === 'folders'}<Folders />
          {:else if app.page === 'collections'}<Collections />
          {:else}<Settings />{/if}
        </main>
        {#if app.player.video_id != null}<MiniPlayer />{/if}
      </div>
    </div>
  {/if}
  {#if app.toast}
    <div class="toast" class:error={app.toast.error}>{app.toast.text}</div>
  {/if}
{/if}

<style>
  .shell { display: flex; height: 100vh; background: var(--bg); }
  nav { width: 210px; background: var(--surface); border-right: 1px solid var(--border); padding: 14px 10px; display: flex; flex-direction: column; gap: 4px; }
  .brand { font-size: 17px; font-weight: 700; padding: 6px 10px 16px; letter-spacing: .02em; }
  .logo { color: var(--primary); margin-right: 4px; }
  .nav { text-align: left; display: flex; align-items: center; gap: 10px; padding: 10px 12px; }
  .nav.active { background: var(--primary-soft); color: var(--primary); border-color: transparent; }
  .icon { width: 18px; text-align: center; }
  .content { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  main { flex: 1; overflow-y: auto; padding: 22px 26px; }
  .toast {
    position: fixed; right: 20px; top: 20px; z-index: 50; max-width: 420px; padding: 11px 16px;
    background: var(--surface-2); border: 1px solid var(--primary); border-radius: 10px; box-shadow: 0 8px 30px #0008;
  }
  .toast.error { border-color: var(--danger); }
</style>
