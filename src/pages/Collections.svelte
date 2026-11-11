<script lang="ts">
  import { api, type Video } from '../lib/api';
  import { app, guarded, playQueue, refreshAll } from '../lib/store.svelte';
  import X from '@lucide/svelte/icons/x';
  import Play from '@lucide/svelte/icons/play';
  import ChevronUp from '@lucide/svelte/icons/chevron-up';
  import ChevronDown from '@lucide/svelte/icons/chevron-down';
  import ChevronRight from '@lucide/svelte/icons/chevron-right';

  let tagName = $state('');
  let tagColor = $state('#d4a017');
  let plName = $state('');
  let selectedPl = $state<number | null>(null);
  let plVideos = $state<Video[]>([]);

  async function loadPlaylistVideos() {
    if (selectedPl == null) { plVideos = []; return; }
    const v = await guarded(() => api.listVideos({ playlist_id: selectedPl }));
    plVideos = v ?? [];
  }

  $effect(() => {
    void selectedPl;
    void app.playlists; // refresh when counts change
    loadPlaylistVideos();
  });

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
    const p = await guarded(() => api.createPlaylist(plName));
    plName = '';
    await refreshAll();
    if (p) selectedPl = p.id;
  }
  async function renamePl(id: number, name: string) {
    await guarded(() => api.renamePlaylist(id, name));
    await refreshAll();
  }
  async function delPl(id: number) {
    if (app.filter.playlist_id === id) app.filter.playlist_id = null;
    await guarded(() => api.deletePlaylist(id));
    if (selectedPl === id) selectedPl = null;
    await refreshAll();
  }
  async function move(i: number, delta: number) {
    if (selectedPl == null) return;
    const ids = plVideos.map((v) => v.id);
    const j = i + delta;
    if (j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    await guarded(() => api.reorderPlaylist(selectedPl!, ids));
    await loadPlaylistVideos();
  }
  async function removeFrom(videoId: number) {
    if (selectedPl == null) return;
    await guarded(() => api.removeFromPlaylist(selectedPl!, videoId));
    await refreshAll();
  }
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
      <div class="card row item" class:sel={selectedPl === p.id}>
        <input type="text" class="grow" value={p.name} onchange={(e) => renamePl(p.id, e.currentTarget.value)} />
        <button class="ghost" title="Show videos" onclick={() => (selectedPl = selectedPl === p.id ? null : p.id)}>
          {p.video_count} {#if selectedPl === p.id}<ChevronDown size={14} />{:else}<ChevronRight size={14} />{/if}
        </button>
        <button class="danger ghost" onclick={() => delPl(p.id)} title="Delete"><X size={15} /></button>
      </div>
      {#if selectedPl === p.id}
        <div class="plitems">
          <div class="row" style="margin-bottom:8px">
            <button class="primary" disabled={!plVideos.length}
              onclick={() => playQueue(plVideos.map((v) => v.id), 0)}><Play size={15} fill="currentColor" /> Play playlist</button>
          </div>
          {#each plVideos as v, i (v.id)}
            <div class="row plrow">
              <span class="muted num">{i + 1}</span>
              <button class="ghost grow name" onclick={() => playQueue(plVideos.map((x) => x.id), i)}>{v.title}</button>
              <button class="ghost" disabled={i === 0} onclick={() => move(i, -1)}><ChevronUp size={15} /></button>
              <button class="ghost" disabled={i === plVideos.length - 1} onclick={() => move(i, 1)}><ChevronDown size={15} /></button>
              <button class="ghost danger" onclick={() => removeFrom(v.id)}><X size={15} /></button>
            </div>
          {:else}<div class="muted">Empty. Add videos from the library with “Organize”.</div>{/each}
        </div>
      {/if}
    {:else}<div class="muted">No playlists yet.</div>{/each}
  </section>
</div>

<style>
  .intro { margin-bottom: 18px; }
  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: 28px; }
  @media (max-width: 1000px) { .cols { grid-template-columns: 1fr; } }
  .add { margin-bottom: 12px; }
  .item { padding: 8px 10px; margin-bottom: 8px; }
  .item.sel { border-color: var(--primary); }
  .name { text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .plitems { margin: -2px 0 12px 14px; padding-left: 12px; border-left: 2px solid var(--primary-soft); }
  .plrow { gap: 4px; }
  .num { width: 22px; text-align: right; }
</style>
