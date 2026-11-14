<script lang="ts">
  import { api } from '../lib/api';
  import { app, guarded, refreshAll } from '../lib/store.svelte';
  import { t, th } from '../lib/i18n.svelte';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import X from '@lucide/svelte/icons/x';
  import Check from '@lucide/svelte/icons/check';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  let tagName = $state('');
  let tagColor = $state('#d4a017');
  let plName = $state('');

  // Inline playlist rename: the name turns into an input; pencil becomes cancel + save.
  let renamingId = $state<number | null>(null);
  let renameValue = $state('');

  async function addTag() {
    if (!tagName.trim()) return;
    await guarded(() => api.createTag(tagName, tagColor));
    tagName = '';
    await refreshAll();
  }
  async function editTag(id: number, name: string, color: string) {
    await guarded(() => api.updateTag(id, name, color));
    await refreshAll();
  }
  let pending = $state<{ kind: 'tag' | 'playlist'; id: number; name: string; count: number } | null>(null);
  async function confirmDelete() {
    const p = pending;
    pending = null;
    if (!p) return;
    if (p.kind === 'tag') await delTag(p.id); else await delPl(p.id);
  }

  async function delTag(id: number) {
    app.filter.tag_ids = app.filter.tag_ids.filter((t) => t !== id);
    await guarded(() => api.deleteTag(id));
    await refreshAll();
  }
  function filterByTag(id: number) {
    app.filter.tag_ids = [id];
    app.filter.playlist_id = null;
    app.page = 'library';
  }

  async function addPlaylist() {
    if (!plName.trim()) return;
    await guarded(() => api.createPlaylist(plName));
    plName = '';
    await refreshAll();
  }
  function startRename(id: number, name: string) {
    renamingId = id;
    renameValue = name;
  }
  function cancelRename() {
    renamingId = null;
  }
  async function saveRename() {
    if (renamingId == null) return;
    const id = renamingId;
    const ok = await guarded(async () => {
      await api.renamePlaylist(id, renameValue);
      return true;
    });
    if (ok) {
      renamingId = null;
      await refreshAll();
    }
  }
  async function delPl(id: number) {
    if (app.filter.playlist_id === id) app.filter.playlist_id = null;
    await guarded(() => api.deletePlaylist(id));
    await refreshAll();
  }
  function showPlaylist(id: number) {
    app.filter.playlist_id = id;
    app.filter.sort_by = 'default'; // playlist order
    app.filter.tag_ids = [];
    app.page = 'library';
  }
  const focusOnMount = (el: HTMLInputElement) => { el.focus(); el.select(); };
</script>

<h1>{t('collections.title')}</h1>
<div class="muted intro">{t('collections.intro')}</div>

<div class="cols">
  <section>
    <h2>{t('common.tags')}</h2>
    <form class="row add" onsubmit={(e) => { e.preventDefault(); addTag(); }}>
      <input type="color" bind:value={tagColor} />
      <input type="text" class="grow" placeholder={t('common.new_tag')} bind:value={tagName} />
      <button class="primary" type="submit">{t('common.create')}</button>
    </form>
    {#each app.tags as tag (tag.id)}
      <div class="card row item">
        <input type="color" value={tag.color} onchange={(e) => editTag(tag.id, tag.name, e.currentTarget.value)} />
        <input type="text" class="grow" value={tag.name}
          onchange={(e) => editTag(tag.id, e.currentTarget.value, tag.color)} />
        <button class="ghost" title={t('common.show_in_library')} onclick={() => filterByTag(tag.id)}>{tag.video_count} <ChevronRight size={14} /></button>
        <button class="danger ghost" onclick={() => (pending = { kind: 'tag', id: tag.id, name: tag.name, count: tag.video_count })} title={t('common.delete')}><X size={15} /></button>
      </div>
    {:else}<div class="muted">{t('common.no_tags')}</div>{/each}
  </section>

  <section>
    <h2>{t('common.playlists')}</h2>
    <form class="row add" onsubmit={(e) => { e.preventDefault(); addPlaylist(); }}>
      <input type="text" class="grow" placeholder={t('common.new_playlist')} bind:value={plName} />
      <button class="primary" type="submit">{t('common.create')}</button>
    </form>
    {#each app.playlists as p (p.id)}
      <div class="card row item">
        {#if renamingId === p.id}
          <input type="text" class="grow" bind:value={renameValue} use:focusOnMount
            onkeydown={(e) => { if (e.key === 'Enter') saveRename(); else if (e.key === 'Escape') cancelRename(); }} />
          <button class="ghost" title={t('common.cancel')} onclick={cancelRename}><X size={15} /></button>
          <button class="ghost save" title={t('common.save')} onclick={saveRename}><Check size={15} /></button>
        {:else}
          <button class="ghost grow name" title={t('common.show_in_library')} onclick={() => showPlaylist(p.id)}>
            {p.name} <span class="muted">· {p.video_count}</span>
          </button>
          <button class="ghost" title={t('common.rename')} onclick={() => startRename(p.id, p.name)}><Pencil size={15} /></button>
          <button class="danger ghost" onclick={() => (pending = { kind: 'playlist', id: p.id, name: p.name, count: p.video_count })} title={t('common.delete')}><Trash2 size={15} /></button>
        {/if}
      </div>
    {:else}<div class="muted">{t('common.no_playlists')}</div>{/each}
  </section>
</div>

{#if pending}
  <ConfirmDialog title={t(pending.kind === 'tag' ? 'collections.confirm_tag_title' : 'collections.confirm_playlist_title')} onconfirm={confirmDelete} oncancel={() => (pending = null)}>
    <p>{@html th(pending.kind === 'tag' ? 'collections.confirm_tag' : 'collections.confirm_playlist', { name: pending.name, n: pending.count })}</p>
  </ConfirmDialog>
{/if}

<style>
  .intro { margin-bottom: 18px; }
  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: 28px; }
  @media (max-width: 1000px) { .cols { grid-template-columns: 1fr; } }
  .add { margin-bottom: 12px; }
  .item { padding: 8px 10px; margin-bottom: 8px; }
  .name { justify-content: flex-start; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .save { color: var(--primary); }
</style>
