<script lang="ts">
  import { api, type Video } from '../lib/api';
  import { app, defaultFolderId, guarded, libraryFilter, loadVideos, moveInPlaylist, pageSize, playQueue, refreshAll, removeFromPlaylist, setHidden, setPage, toast, toggleWatched, toggleWatchedAll } from '../lib/store.svelte';
  import type { PlaylistCtx } from '../lib/dnd';
  import type { SortBy } from '../lib/api';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import VideoTable from '../components/VideoTable.svelte';
  import Play from '@lucide/svelte/icons/play';
  import CheckCheck from '@lucide/svelte/icons/check-check';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import X from '@lucide/svelte/icons/x';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import ChevronsLeft from '@lucide/svelte/icons/chevrons-left';
  import ChevronsRight from '@lucide/svelte/icons/chevrons-right';
  import EyeOff from '@lucide/svelte/icons/eye-off';
  import Eye from '@lucide/svelte/icons/eye';
  import Tag from '@lucide/svelte/icons/tag';
  import VideoCard from '../components/VideoCard.svelte';
  import ManageVideo from '../components/ManageVideo.svelte';

  let managingId = $state<number | null>(null);
  const managing = $derived(app.videos.find((v) => v.id === managingId));
  let scanning = $state(false);

  // Right-click menu (shared by grid and table).
  let menu = $state<{ x: number; y: number; video: Video } | null>(null);
  function openMenu(e: MouseEvent, video: Video) {
    e.preventDefault();
    // Keep the menu inside the window.
    menu = { x: Math.min(e.clientX, window.innerWidth - 220), y: Math.min(e.clientY, window.innerHeight - 190), video };
  }
  const closeMenu = () => (menu = null);
  // Run the action before closing: the items read `menu.video`, which is gone once the menu closes.
  function menuDo(fn: () => unknown) { try { fn(); } finally { closeMenu(); } }

  // Reload whenever any filter changes (debounced for typing in the search box).
  let timer: ReturnType<typeof setTimeout>;
  let lastKey: string | null = null;
  $effect(() => {
    const key = JSON.stringify([app.filter, app.settings.library_view, app.settings.grid_page_size, app.settings.table_page_size]);
    // Any filter/sort/page-size change starts again from the first page, but merely
    // re-opening the Library (e.g. after stopping a video) keeps the current page.
    if (lastKey !== null && key !== lastKey) app.paging.page = 0;
    lastKey = key;
    clearTimeout(timer);
    timer = setTimeout(loadVideos, 150);
    return () => clearTimeout(timer);
  });

  function onkey(e: KeyboardEvent) {
    if (e.key === 'Escape' && menu) { closeMenu(); return; }
    if (app.playerView || managing || e.ctrlKey || e.altKey || e.metaKey || e.shiftKey) return;
    const el = e.target as HTMLElement | null;
    // Esc = the "Clear" button (also from the search box; a select just closes its dropdown).
    if (e.key === 'Escape' && el?.tagName !== 'SELECT') {
      if (filtered) { e.preventDefault(); clearFilters(); }
      if (el?.tagName === 'INPUT') el.blur();
      return;
    }
    if (el && (el.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName))) return;
    if (e.key === 'ArrowLeft' && app.paging.page > 0) { e.preventDefault(); setPage(app.paging.page - 1); }
    else if (e.key === 'ArrowRight' && app.paging.page < pageCount - 1) { e.preventDefault(); setPage(app.paging.page + 1); }
  }

  function toggleTagFilter(id: number) {
    const t = app.filter.tag_ids;
    app.filter.tag_ids = t.includes(id) ? t.filter((x) => x !== id) : [...t, id];
  }

  function clearFilters() {
    app.filter.search = '';
    app.filter.watched = null;
    app.filter.hidden_only = false;
    app.filter.tag_ids = [];
    app.filter.playlist_id = null;
    app.filter.folder_id = defaultFolderId(); // the default folder (if any) stays applied
  }

  const filtered = $derived(
    app.filter.search !== '' || app.filter.watched !== null || app.filter.hidden_only || app.filter.tag_ids.length > 0 ||
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
  // The queue is the whole filtered list (all pages), fetched from the backend.
  async function playFiltered(startId?: number) {
    const all = await guarded(() => api.listVideos(libraryFilter()));
    if (!all || all.length === 0) return;
    const i = startId === undefined ? 0 : all.findIndex((x) => x.id === startId);
    await playQueue(all, Math.max(0, i), playlistTitle());
  }
  const play = (v: Video) => playFiltered(v.id);
  const pageCount = $derived(Math.max(1, Math.ceil(app.paging.total / pageSize())));
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

<svelte:window onkeydown={onkey} />

<div class="head">
  <div class="grow">
    <h1>Library</h1>
    <div class="muted">{app.paging.total} video{app.paging.total === 1 ? '' : 's'}{filtered ? ' (filtered)' : ''}</div>
  </div>
  {#if app.filter.tag_ids.length > 0 || app.filter.playlist_id !== null}
    <button onclick={toggleWatchedAll} disabled={app.paging.total === 0}
      title="Mark every video matching this filter as watched; press again to mark them all unwatched">
      <CheckCheck size={15} /> Toggle watched
    </button>
  {/if}
  <button onclick={rescan} disabled={scanning}><RefreshCw size={15} /> {scanning ? 'Scanning…' : 'Rescan'}</button>
  <button class="primary" disabled={app.paging.total === 0} onclick={() => playFiltered()}><Play size={15} fill="currentColor" /> Play all</button>
</div>

<div class="toolbar">
  <input type="search" class="grow" placeholder="Search videos…" bind:value={app.filter.search} />
  <div class="seg">
    {#each watchedOptions as [val, label]}
      <button class:on={!app.filter.hidden_only && app.filter.watched === val}
        onclick={() => { app.filter.hidden_only = false; app.filter.watched = val; }}>{label}</button>
    {/each}
    {#if app.settings.show_hidden === 'true'}
      <button class:on={app.filter.hidden_only} onclick={() => (app.filter.hidden_only = true)}>Hidden</button>
    {/if}
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

{#if playlistCtx?.canReorder && app.paging.total > 1}
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
  <VideoTable videos={app.videos} onplay={play} onmanage={(v) => (managingId = v.id)} oncontext={openMenu} playlist={playlistCtx} />
{:else}
  <div class="grid">
    {#each app.videos as v (v.id)}
      <VideoCard video={v} onplay={() => play(v)} onmanage={() => (managingId = v.id)} oncontext={(e) => openMenu(e, v)} playlist={playlistCtx} />
    {/each}
  </div>
{/if}

{#if pageCount > 1}
  <div class="pager">
    <button class="ghost" disabled={app.paging.page === 0} onclick={() => setPage(0)} title="First page"><ChevronsLeft size={16} /></button>
    <button class="ghost" disabled={app.paging.page === 0} onclick={() => setPage(app.paging.page - 1)} title="Previous page"><ChevronLeft size={16} /></button>
    <span class="muted">Page {app.paging.page + 1} of {pageCount}</span>
    <button class="ghost" disabled={app.paging.page >= pageCount - 1} onclick={() => setPage(app.paging.page + 1)} title="Next page"><ChevronRight size={16} /></button>
    <button class="ghost" disabled={app.paging.page >= pageCount - 1} onclick={() => setPage(pageCount - 1)} title="Last page"><ChevronsRight size={16} /></button>
  </div>
{/if}

{#if menu}
  {@const v = menu.video}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="menu-backdrop" onclick={closeMenu} oncontextmenu={(e) => { e.preventDefault(); closeMenu(); }}>
    <div class="menu card" style:left="{menu.x}px" style:top="{menu.y}px" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <button class="ghost" role="menuitem" onclick={() => menuDo(() => play(v))}><Play size={14} /> Play</button>
      <button class="ghost" role="menuitem" onclick={() => menuDo(() => toggleWatched(v))}>
        {#if v.watched}<EyeOff size={14} /> Mark as unwatched{:else}<Eye size={14} /> Mark as watched{/if}
      </button>
      <button class="ghost" role="menuitem" onclick={() => menuDo(() => (managingId = v.id))}><Tag size={14} /> Tags & playlists</button>
      <hr />
      <button class="ghost" role="menuitem" onclick={() => menuDo(() => setHidden(v, !v.hidden))}>
        {#if v.hidden}<Eye size={14} /> Unhide video{:else}<EyeOff size={14} /> Hide video{/if}
      </button>
    </div>
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
  .menu-backdrop { position: fixed; inset: 0; z-index: 40; }
  .menu { position: fixed; min-width: 200px; padding: 5px; display: flex; flex-direction: column; gap: 2px; box-shadow: 0 8px 30px #0008; }
  .menu button { justify-content: flex-start; display: flex; align-items: center; gap: 9px; text-align: left; padding: 7px 10px; }
  .menu hr { border: 0; border-top: 1px solid var(--border); margin: 3px 0; width: 100%; }
  .pager { display: flex; justify-content: center; align-items: center; gap: 8px; margin: 16px 0 4px; }
  .empty { text-align: center; padding: 40px; color: var(--muted); }
</style>
