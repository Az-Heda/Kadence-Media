<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { api, type IndexingProgress, type PlayerState } from './lib/api';
  import { app, defaultFolderId, loadSettings, loadProfiles, refreshAll, playNext, queueIndex, guarded, setPip, toast, type Page } from './lib/store.svelte';
  import { t, type Key } from './lib/i18n.svelte';
  import Library from './pages/Library.svelte';
  import Folders from './pages/Folders.svelte';
  import Collection from './pages/Collection.svelte';
  import Themes from './pages/Themes.svelte';
  import SettingsPage from './pages/Settings.svelte';
  import Player from './pages/Player.svelte';
  import Rules from './pages/Rules.svelte';
  import Stats from './pages/Stats.svelte';
  import ChartColumn from '@lucide/svelte/icons/chart-column';
  import MiniPlayer from './components/MiniPlayer.svelte';
  import ProfilesDialog from './components/ProfilesDialog.svelte';
  import UserRound from '@lucide/svelte/icons/user-round';
  import LayoutGrid from '@lucide/svelte/icons/layout-grid';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import Tags from '@lucide/svelte/icons/tags';
  import ListVideo from '@lucide/svelte/icons/list-video';
  import WandSparkles from '@lucide/svelte/icons/wand-sparkles';
  import Settings2 from '@lucide/svelte/icons/settings';
  import Palette from '@lucide/svelte/icons/palette';
  import Clapperboard from '@lucide/svelte/icons/clapperboard';
  import LoaderCircle from '@lucide/svelte/icons/loader-circle';

  const surface = new URLSearchParams(location.search).get('surface') ?? 'overlay';

  const allPages: { id: Page; label: Key; icon: typeof LayoutGrid }[] = [
    { id: 'library', label: 'nav.library', icon: LayoutGrid },
    { id: 'folders', label: 'nav.folders', icon: FolderOpen },
    { id: 'playlists', label: 'nav.playlists', icon: ListVideo },
    { id: 'tags', label: 'nav.tags', icon: Tags },
    { id: 'rules', label: 'nav.rules', icon: WandSparkles },
    { id: 'stats', label: 'nav.stats', icon: ChartColumn },
  ];
  // Without folders there is nothing to show: hide the Library and land on Folders.
  const pages = $derived(allPages.filter((p) => p.id !== 'library' || app.folders.length > 0));
  let loaded = $state(false);
  let showProfiles = $state(false);
  const currentProfile = $derived(app.profiles.profiles.find((p) => p.id === app.profiles.current));
  $effect(() => {
    if (loaded && app.folders.length === 0 && app.page === 'library') app.page = 'folders';
  });
  // Pinned to the bottom of the sidebar.
  const bottomPages: { id: Page; label: Key; icon: typeof LayoutGrid }[] = [
    { id: 'themes', label: 'nav.themes', icon: Palette },
    { id: 'settings', label: 'nav.settings', icon: Settings2 },
  ];

  $effect(() => {
    document.body.classList.toggle('host', surface === 'host');
    document.body.classList.toggle('transparent', surface === 'overlay' && app.playerView);
  });

  onMount(() => {
    if (surface !== 'overlay') return;
    guarded(async () => {
      await loadSettings();
      await loadProfiles();
      app.folders = await api.listFolders(); // needed to tell whether the default folder is empty
      app.filter.folder_id = defaultFolderId(); // default folder is applied before the first load
      await refreshAll();
      loaded = true;
      app.player = await api.playerState();
      app.indexing = await api.indexingStatus();
      // Pick up files added/removed while the app was closed (errors only, to avoid noise).
      const [, errors] = await api.rescanAll();
      if (errors.length) toast(errors.join('\n'), true);
      await refreshAll();
    });
    const unlisten = [
      listen<PlayerState>('player-state', (e) => {
        app.player = e.payload;
        // mpv went away while in picture-in-picture: go back to the normal window.
        if (app.pip && e.payload.video_id == null) setPip(false);
      }),
      listen<{ id: number; watched: boolean }>('watched-changed', (e) => {
        const v = app.videos.find((x) => x.id === e.payload.id);
        if (v) v.watched = e.payload.watched;
        if (app.filter.watched !== null) refreshAll();
      }),
      listen<IndexingProgress>('indexing-progress', (e) => (app.indexing = e.payload)),
      listen<{ id: number; duration: number }>('video-duration', (e) => {
        for (const list of [app.videos, app.queueVideos]) {
          const v = list.find((x) => x.id === e.payload.id);
          if (v) v.duration = e.payload.duration;
        }
      }),
      listen<{ id: number }>('video-thumb', (e) => {
        for (const list of [app.videos, app.queueVideos]) {
          const v = list.find((x) => x.id === e.payload.id);
          if (v) v.thumb = true;
        }
      }),
      listen<string>('indexing-error', (e) => toast(t('app.indexing_error', { msg: e.payload }), true)),
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
        <div class="brand"><span class="logo"><Clapperboard size={20} /></span> Kadence Media</div>
        {#each pages as p}
          <button class="nav ghost" class:active={app.page === p.id} onclick={() => (app.page = p.id)}>
            <p.icon size={18} />{t(p.label)}
          </button>
        {/each}
        <span class="spacer"></span>
        {#if app.indexing.running}
          <div class="indexing" title={t('nav.indexing_title')}>
            <span class="spin"><LoaderCircle size={15} /></span>
            {t('nav.indexing', { done: app.indexing.done, total: app.indexing.total })}
          </div>
        {/if}
        {#each bottomPages as p}
          <button class="nav ghost" class:active={app.page === p.id} onclick={() => (app.page = p.id)}>
            <p.icon size={18} />{t(p.label)}
          </button>
        {/each}
        <button class="nav ghost profile" title={t('profiles.switch_title')} onclick={() => (showProfiles = true)}>
          <UserRound size={18} /><span class="pname">{currentProfile?.name ?? ''}</span>
        </button>
      </nav>
      <div class="content">
        <main>
          {#if app.page === 'library'}<Library />
          {:else if app.page === 'folders'}<Folders />
          {:else if app.page === 'playlists'}<Collection kind="playlist" />
          {:else if app.page === 'tags'}<Collection kind="tag" />
          {:else if app.page === 'rules'}<Rules />
          {:else if app.page === 'stats'}<Stats />
          {:else if app.page === 'themes'}<Themes />
          {:else}<SettingsPage />{/if}
        </main>
        {#if app.player.video_id != null}<MiniPlayer />{/if}
      </div>
    </div>
  {/if}
  {#if showProfiles}<ProfilesDialog onclose={() => (showProfiles = false)} />{/if}
  {#if app.toast}
    <div class="toast" class:error={app.toast.error}>{app.toast.text}</div>
  {/if}
{/if}

<style>
  .shell { display: flex; height: 100vh; background: var(--bg); }
  nav { width: 210px; background: var(--surface); border-right: 1px solid var(--border); padding: 14px 10px; display: flex; flex-direction: column; gap: 4px; }
  .brand { font-size: 17px; font-weight: 700; padding: 6px 10px 16px; letter-spacing: .02em; }
  .brand { display: flex; align-items: center; gap: 8px; }
  .logo { color: var(--primary); display: inline-flex; }
  .nav { justify-content: flex-start; text-align: left; display: flex; align-items: center; gap: 10px; padding: 10px 12px; }
  .nav.active { background: var(--primary-soft); color: var(--primary); border-color: transparent; }
    .spacer { flex: 1; }
  .profile { margin-top: 6px; border-top: 1px solid var(--border); border-radius: 0 0 8px 8px; padding-top: 12px; }
  .pname { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .indexing { display: flex; align-items: center; gap: 8px; padding: 10px 12px; font-size: 12px; color: var(--muted); }
  .spin { display: inline-flex; animation: spin 1s linear infinite; color: var(--primary); }
  @keyframes spin { to { transform: rotate(360deg); } }
  .content { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  main { flex: 1; overflow-y: auto; padding: 22px 26px; }
  .toast {
    position: fixed; right: 20px; top: 20px; z-index: 50; max-width: 420px; padding: 11px 16px;
    background: var(--surface-2); border: 1px solid var(--primary); border-radius: 10px; box-shadow: 0 8px 30px #0008;
  }
  .toast.error { border-color: var(--danger); }
  .toast { white-space: pre-line; }
</style>
