<script lang="ts">
  import { api, type Video } from '../lib/api';
  import { app, defaultFolderId, guarded, loadVideos, moveInPlaylist, playQueue, refreshAll, removeFromPlaylist, toast } from '../lib/store.svelte';
  import type { PlaylistCtx } from '../lib/dnd';
  import type { SortBy } from '../lib/api';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import VideoTable from '../components/VideoTable.svelte';
  import Play from '@lucide/svelte/icons/play';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import X from '@lucide/svelte/icons/x';
  import VideoCard from '../components/VideoCard.svelte';
  import ManageVideo from '../components/ManageVideo.svelte';

  let managingId = $state<number | null>(null);
  const managing = $derived(app.videos.find((v) => v.id === managingId));
  let scanning = $state(false);

  // Reload whenever any filter changes (debounced for typing in the search box).
  let timer: ReturnType<typeof setTimeout>;
  $effect(() => {
    void [app.filter.search, app.filter.watched, app.filter.playlist_id, app.filter.folder_id, [...app.filter.tag_ids], app.filter.sort_by, app.filter.sort_desc];
    clearTimeout(timer);
    timer = setTimeout(loadVideos, 150);
    return () => clearTimeout(timer);
  });

  function toggleTagFilter(id: number) {
    const t = app.filter.tag_ids;
    app.filter.tag_ids = t.includes(id) ? t.filter((x) => x !== id) : [...t, id];
  }

  function clearFilters() {
    app.filter.search = '';
    app.filter.watched = null;
    app.filter.tag_ids = [];
    app.filter.playlist_id = null;
    app.filter.folder_id = defaultFolderId(); // the default folder (if any) stays applied
  }

  const filtered = $derived(
    app.filter.search !== '' || app.filter.watched !== null || app.filter.tag_ids.length > 0 ||
      app.filter.playlist_id !== null || app.filter.folder_id !== defaultFolderId(),
  );

  async function rescan() {
    scanning = true;
    const res = await guarded(() => api.rescanAll());
    scanning = false;
    if (res) {
      const [r, errors] = res;
      if (errors.length) toast(errors.join('\n'), true);
      else toast(`Scan done: ${r.added} added, ${r.removed} removed`);
      await refreshAll();
    }
  }

  // A queue started from a playlist view gets the playlist side panel in the player.
  const playlistTitle = () => app.playlists.find((p) => p.id === app.filter.playlist_id)?.name ?? null;
  const play = (v: Video) => playQueue(app.videos, app.videos.findIndex((x) => x.id === v.id), playlistTitle());
  // Playlist tools (drag to reorder, remove) appear only while a playlist is being viewed.
  const playlistCtx = $derived<PlaylistCtx | null>(
    app.filter.playlist_id === null
      ? null
      : {
          canReorder: app.filter.sort_by === 'default' && !app.filter.sort_desc,
          onmove: (d, t, after) => moveInPlaylist(app.filter.playlist_id!, d, t, after),
          onremove: (id) => removeFromPlaylist(app.filter.playlist_id!, id),
        },
  );
  const sortOptions: [SortBy, string][] = [
    ['path', 'Sort: Full path'], ['title', 'Sort: Title'], ['created', 'Sort: Date created'],
    ['size', 'Sort: Size'], ['duration', 'Sort: Duration'], ['rating', 'Sort: Rating'],
  ];

  // Inside a playlist the natural order is the playlist's own; leaving it goes back to full path.
  let prevPlaylist = app.filter.playlist_id;
  $effect(() => {
    const pl = app.filter.playlist_id;
    if (pl === prevPlaylist) return;
    prevPlaylist = pl;
    if (pl !== null) app.filter.sort_by = 'default';
    else if (app.filter.sort_by === 'default') app.filter.sort_by = 'path';
  });
  const watchedOptions: [boolean | null, string][] = [[null, 'All'], [false, 'Unwatched'], [true, 'Watched']];
</script>

<div class="head">
  <div class="grow">
    <h1>Library</h1>
    <div class="muted">{app.videos.length} video{app.videos.length === 1 ? '' : 's'}{filtered ? ' (filtered)' : ''}</div>
  </div>
  <button onclick={rescan} disabled={scanning}><RefreshCw size={15} /> {scanning ? 'Scanning…' : 'Rescan'}</button>
  <button class="primary" disabled={app.videos.length === 0} onclick={() => playQueue(app.videos, 0, playlistTitle())}><Play size={15} fill="currentColor" /> Play all</button>
</div>

<div class="toolbar">
  <input type="search" class="grow" placeholder="Search videos…" bind:value={app.filter.search} />
  <div class="seg">
    {#each watchedOptions as [val, label]}
      <button class:on={app.filter.watched === val} onclick={() => (app.filter.watched = val)}>{label}</button>
    {/each}
  </div>
  <select bind:value={app.filter.sort_by} title="Sort videos by">
    {#if app.filter.playlist_id !== null}<option value="default">Sort: Playlist order</option>{/if}
    {#each sortOptions as [val, label]}<option value={val}>{label}</option>{/each}
  </select>
  <button class="ghost" onclick={() => (app.filter.sort_desc = !app.filter.sort_desc)}
    title={app.filter.sort_desc ? 'Descending' : 'Ascending'}>
    {#if app.filter.sort_desc}<ArrowDown size={16} />{:else}<ArrowUp size={16} />{/if}
  </button>
  <select bind:value={app.filter.playlist_id}>
    <option value={null}>All playlists</option>
    {#each app.playlists as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
  </select>
  <select bind:value={app.filter.folder_id}>
    <option value={null}>All folders</option>
    {#each app.folders as f (f.id)}<option value={f.id}>{f.path}</option>{/each}
  </select>
  {#if filtered}<button class="ghost" onclick={clearFilters}><X size={15} /> Clear</button>{/if}
</div>

{#if playlistCtx?.canReorder && app.videos.length > 1}
  <div class="muted hint">Drag videos to reorder this playlist.</div>
{/if}

{#if app.tags.length}
  <div class="tagbar">
    {#each app.tags as t (t.id)}
      <button class="chip" class:active={app.filter.tag_ids.includes(t.id)} onclick={() => toggleTagFilter(t.id)}>
        <span class="dot" style:background={t.color}></span>{t.name}
      </button>
    {/each}
  </div>
{/if}

{#if app.videos.length === 0}
  <div class="empty card">
    {#if filtered}
      No video matches these filters.
    {:else if app.folders.length === 0}
      Your library is empty. Add a folder from the <b>Folders</b> page to get started.
    {:else}
      No videos found in your folders. Try <b>Rescan</b>.
    {/if}
  </div>
{:else if app.settings.library_view === 'table'}
  <VideoTable videos={app.videos} onplay={play} onmanage={(v) => (managingId = v.id)} playlist={playlistCtx} />
{:else}
  <div class="grid">
    {#each app.videos as v (v.id)}
      <VideoCard video={v} onplay={() => play(v)} onmanage={() => (managingId = v.id)} playlist={playlistCtx} />
    {/each}
  </div>
{/if}

{#if managing}<ManageVideo video={managing} onclose={() => (managingId = null)} />{/if}

<style>
  .head { display: flex; align-items: center; gap: 10px; margin-bottom: 14px; }
  .toolbar { display: flex; gap: 10px; flex-wrap: wrap; align-items: center; margin-bottom: 12px; }
  .toolbar select { max-width: 200px; }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 8px; overflow: hidden; }
  .seg button { border: 0; border-radius: 0; background: var(--surface); }
  .seg button.on { background: var(--primary-soft); color: var(--primary); }
  .hint { font-size: 12px; margin: -4px 0 12px; }
  .tagbar { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 16px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 16px; }
  .empty { text-align: center; padding: 40px; color: var(--muted); }
</style>
