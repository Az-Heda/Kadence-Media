<script lang="ts">
  import { api } from '../lib/api';
  import { app, guarded, refreshAll } from '../lib/store.svelte';
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

<h1>Tags & Playlists</h1>
<div class="muted intro">Tags label videos (many per video, used as filters). Playlists are ordered lists you can play in sequence.</div>

<div class="cols">
  <section>
    <h2>Tags</h2>
    <form class="row add" onsubmit={(e) => { e.preventDefault(); addTag(); }}>
      <input type="color" bind:value={tagColor} />
      <input type="text" class="grow" placeholder="New tag…" bind:value={tagName} />
      <button class="primary" type="submit">Create</button>
    </form>
    {#each app.tags as t (t.id)}
      <div class="card row item">
        <input type="color" value={t.color} onchange={(e) => editTag(t.id, t.name, e.currentTarget.value)} />
        <input type="text" class="grow" value={t.name}
          onchange={(e) => editTag(t.id, e.currentTarget.value, t.color)} />
        <button class="ghost" title="Show in library" onclick={() => filterByTag(t.id)}>{t.video_count} <ChevronRight size={14} /></button>
        <button class="danger ghost" onclick={() => delTag(t.id)} title="Delete"><X size={15} /></button>
      </div>
    {:else}<div class="muted">No tags yet.</div>{/each}
  </section>

  <section>
    <h2>Playlists</h2>
    <form class="row add" onsubmit={(e) => { e.preventDefault(); addPlaylist(); }}>
      <input type="text" class="grow" placeholder="New playlist…" bind:value={plName} />
      <button class="primary" type="submit">Create</button>
    </form>
    {#each app.playlists as p (p.id)}
      <div class="card row item">
        {#if renamingId === p.id}
          <input type="text" class="grow" bind:value={renameValue} use:focusOnMount
            onkeydown={(e) => { if (e.key === 'Enter') saveRename(); else if (e.key === 'Escape') cancelRename(); }} />
          <button class="ghost" title="Cancel" onclick={cancelRename}><X size={15} /></button>
          <button class="ghost save" title="Save" onclick={saveRename}><Check size={15} /></button>
        {:else}
          <button class="ghost grow name" title="Show in library" onclick={() => showPlaylist(p.id)}>
            {p.name} <span class="muted">· {p.video_count}</span>
          </button>
          <button class="ghost" title="Rename" onclick={() => startRename(p.id, p.name)}><Pencil size={15} /></button>
          <button class="danger ghost" onclick={() => delPl(p.id)} title="Delete"><Trash2 size={15} /></button>
        {/if}
      </div>
    {:else}<div class="muted">No playlists yet.</div>{/each}
  </section>
</div>

<style>
  .intro { margin-bottom: 18px; }
  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: 28px; }
  @media (max-width: 1000px) { .cols { grid-template-columns: 1fr; } }
  .add { margin-bottom: 12px; }
  .item { padding: 8px 10px; margin-bottom: 8px; }
  .name { justify-content: flex-start; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .save { color: var(--primary); }
</style>
