<script lang="ts">
  import { api } from '../lib/api';
  import { app, guarded, refreshAll } from '../lib/store.svelte';
  import { t, th } from '../lib/i18n.svelte';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import Plus from '@lucide/svelte/icons/plus';
  import X from '@lucide/svelte/icons/x';
  import Check from '@lucide/svelte/icons/check';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';
  import ChevronLeft from '@lucide/svelte/icons/chevron-left';
  import ChevronsLeft from '@lucide/svelte/icons/chevrons-left';
  import ChevronsRight from '@lucide/svelte/icons/chevrons-right';

  // One page component serves both "Tags" and "Playlists"; they share search, paging and dialogs.
  let { kind }: { kind: 'tag' | 'playlist' } = $props();
  type Row = { id: number; name: string; color?: string; video_count: number };

  const PAGE_SIZE = 15;
  let search = $state('');
  let page = $state(0);

  const rows = $derived<Row[]>(kind === 'tag' ? app.tags : app.playlists);
  const filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return q ? rows.filter((r) => r.name.toLowerCase().includes(q)) : rows;
  });
  const pageCount = $derived(Math.max(1, Math.ceil(filtered.length / PAGE_SIZE)));
  const shown = $derived(filtered.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE));
  // Searching or deleting rows can leave the current page out of range.
  $effect(() => { if (page > pageCount - 1) page = pageCount - 1; });

  // ---- create dialog
  let creating = $state(false);
  let newName = $state('');
  let newColor = $state('#d4a017');
  function openCreate() { newName = ''; creating = true; }
  async function create() {
    if (!newName.trim()) return;
    const made = await guarded(() => kind === 'tag' ? api.createTag(newName, newColor) : api.createPlaylist(newName));
    if (!made) return;
    creating = false;
    search = '';
    await refreshAll();
    // Jump to the page holding the new row.
    const i = rows.findIndex((r) => r.id === made.id);
    if (i >= 0) page = Math.floor(i / PAGE_SIZE);
  }

  // ---- inline edit (name for both, colour for tags)
  let editingId = $state<number | null>(null);
  let editValue = $state('');
  function startEdit(r: Row) { editingId = r.id; editValue = r.name; }
  async function saveEdit(r: Row) {
    const ok = await guarded(async () => {
      if (kind === 'tag') await api.updateTag(r.id, editValue, r.color!);
      else await api.renamePlaylist(r.id, editValue);
      return true;
    });
    if (ok) { editingId = null; await refreshAll(); }
  }
  async function setColor(r: Row, color: string) {
    await guarded(() => api.updateTag(r.id, r.name, color));
    await refreshAll();
  }

  // ---- delete
  let pending = $state<Row | null>(null);
  async function confirmDelete() {
    const p = pending;
    pending = null;
    if (!p) return;
    if (kind === 'tag') {
      app.filter.tag_ids = app.filter.tag_ids.filter((x) => x !== p.id);
      await guarded(() => api.deleteTag(p.id));
    } else {
      if (app.filter.playlist_id === p.id) app.filter.playlist_id = null;
      await guarded(() => api.deletePlaylist(p.id));
    }
    await refreshAll();
  }

  function showInLibrary(id: number) {
    if (kind === 'tag') {
      app.filter.tag_ids = [id];
      app.filter.playlist_id = null;
    } else {
      app.filter.playlist_id = id;
      app.filter.sort_by = 'default'; // playlist order
      app.filter.tag_ids = [];
    }
    app.page = 'library';
  }
  const focusOnMount = (el: HTMLInputElement) => { el.focus(); el.select(); };
</script>

<div class="row head">
  <h1 class="grow">{t(kind === 'tag' ? 'common.tags' : 'common.playlists')}</h1>
  <button class="primary" onclick={openCreate}><Plus size={15} /> {t(kind === 'tag' ? 'collections.create_tag' : 'collections.create_playlist')}</button>
</div>
<div class="muted intro">{t(kind === 'tag' ? 'collections.intro_tags' : 'collections.intro_playlists')}</div>

<input type="text" class="search" placeholder={t('collections.search')} bind:value={search} oninput={() => (page = 0)} />

<div class="card tablewrap">
  <table>
    <thead>
      <tr>
        {#if kind === 'tag'}<th class="colorcol">{t('collections.color')}</th>{/if}
        <th>{t('collections.name')}</th>
        <th class="num">{t('collections.videos')}</th>
        <th class="actions"></th>
      </tr>
    </thead>
    <tbody>
      {#each shown as r (r.id)}
        <tr>
          {#if kind === 'tag'}
            <td class="colorcol"><input type="color" value={r.color} onchange={(e) => setColor(r, e.currentTarget.value)} /></td>
          {/if}
          <td>
            {#if editingId === r.id}
              <input type="text" class="full" bind:value={editValue} use:focusOnMount
                onkeydown={(e) => { if (e.key === 'Enter') saveEdit(r); else if (e.key === 'Escape') editingId = null; }} />
            {:else}{r.name}{/if}
          </td>
          <td class="num">{r.video_count}</td>
          <td class="actions">
            {#if editingId === r.id}
              <button class="ghost" title={t('common.cancel')} onclick={() => (editingId = null)}><X size={15} /></button>
              <button class="ghost save" title={t('common.save')} onclick={() => saveEdit(r)}><Check size={15} /></button>
            {:else}
              <button class="ghost" title={t('common.show_in_library')} onclick={() => showInLibrary(r.id)}><ChevronRight size={15} /></button>
              <button class="ghost" title={t('common.rename')} onclick={() => startEdit(r)}><Pencil size={15} /></button>
              <button class="danger ghost" title={t('common.delete')} onclick={() => (pending = r)}><Trash2 size={15} /></button>
            {/if}
          </td>
        </tr>
      {:else}
        <tr><td colspan="4" class="muted empty">{t(search.trim() ? 'collections.no_results' : kind === 'tag' ? 'common.no_tags' : 'common.no_playlists')}</td></tr>
      {/each}
    </tbody>
  </table>
</div>

{#if pageCount > 1}
  <div class="row pager">
    <button class="ghost" title={t('library.first_page')} disabled={page === 0} onclick={() => (page = 0)}><ChevronsLeft size={16} /></button>
    <button class="ghost" title={t('library.prev_page')} disabled={page === 0} onclick={() => page--}><ChevronLeft size={16} /></button>
    <span class="muted">{t('library.page_of', { page: page + 1, total: pageCount })}</span>
    <button class="ghost" title={t('library.next_page')} disabled={page >= pageCount - 1} onclick={() => page++}><ChevronRight size={16} /></button>
    <button class="ghost" title={t('library.last_page')} disabled={page >= pageCount - 1} onclick={() => (page = pageCount - 1)}><ChevronsRight size={16} /></button>
  </div>
{/if}

<svelte:window onkeydown={(e) => e.key === 'Escape' && (creating = false)} />

{#if creating}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions, a11y_no_noninteractive_element_interactions -->
  <div class="backdrop" onclick={() => (creating = false)}>
    <form class="dialog card" aria-label={t(kind === 'tag' ? 'collections.create_tag' : 'collections.create_playlist')} onclick={(e) => e.stopPropagation()}
      onsubmit={(e) => { e.preventDefault(); create(); }}>
      <h1>{t(kind === 'tag' ? 'collections.create_tag' : 'collections.create_playlist')}</h1>
      <div class="row fields">
        {#if kind === 'tag'}<input type="color" bind:value={newColor} />{/if}
        <input type="text" class="grow" placeholder={t(kind === 'tag' ? 'common.new_tag' : 'common.new_playlist')}
          bind:value={newName} use:focusOnMount />
      </div>
      <div class="row buttons">
        <span class="grow"></span>
        <button type="button" onclick={() => (creating = false)}>{t('common.cancel')}</button>
        <button class="primary" type="submit" disabled={!newName.trim()}>{t('common.create')}</button>
      </div>
    </form>
  </div>
{/if}

{#if pending}
  <ConfirmDialog title={t(kind === 'tag' ? 'collections.confirm_tag_title' : 'collections.confirm_playlist_title')} onconfirm={confirmDelete} oncancel={() => (pending = null)}>
    <p>{@html th(kind === 'tag' ? 'collections.confirm_tag' : 'collections.confirm_playlist', { name: pending.name, n: pending.video_count })}</p>
  </ConfirmDialog>
{/if}

<style>
  .head { margin-bottom: 4px; }
  .intro { margin-bottom: 16px; }
  .search { width: 100%; max-width: 360px; margin-bottom: 12px; }
  .tablewrap { padding: 0; overflow: hidden; }
  table { width: 100%; border-collapse: collapse; }
  th, td { text-align: left; padding: 9px 14px; border-bottom: 1px solid var(--border); }
  tbody tr:last-child td { border-bottom: none; }
  th { font-size: 12px; color: var(--muted); font-weight: 600; text-transform: uppercase; letter-spacing: .04em; }
  .num { text-align: right; width: 90px; }
  .colorcol { width: 70px; }
  .actions { width: 130px; text-align: right; white-space: nowrap; }
  .empty { text-align: center; padding: 22px; }
  .full { width: 100%; }
  .save { color: var(--primary); }
  .pager { justify-content: center; margin-top: 14px; gap: 6px; }
  .backdrop { position: fixed; inset: 0; background: #000a; display: grid; place-items: center; z-index: 60; }
  .dialog { width: min(440px, 92vw); }
  .fields { margin: 14px 0 4px; }
  .buttons { margin-top: 16px; }
</style>
