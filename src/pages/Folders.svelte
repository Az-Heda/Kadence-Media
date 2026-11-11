<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { api } from '../lib/api';
  import { app, guarded, refreshAll, toast } from '../lib/store.svelte';

  let busy = $state(false);

  async function add() {
    const dir = await open({ directory: true, multiple: false, title: 'Choose a folder with videos' });
    if (typeof dir !== 'string') return;
    busy = true;
    const f = await guarded(() => api.addFolder(dir));
    busy = false;
    if (f) toast(`Added ${f.path}: ${f.video_count} videos`);
    await refreshAll();
  }

  async function remove(id: number) {
    await guarded(() => api.removeFolder(id));
    await refreshAll();
  }

  async function rescan() {
    busy = true;
    const res = await guarded(() => api.rescanAll());
    busy = false;
    if (res) {
      const [r, errors] = res;
      if (errors.length) toast(errors.join('\n'), true);
      else toast(`Scan done: ${r.added} added, ${r.removed} removed`);
    }
    await refreshAll();
  }
</script>

<div class="head">
  <div class="grow">
    <h1>Folders</h1>
    <div class="muted">Every video in these folders (and their subfolders) shows up in the library.</div>
  </div>
  <button onclick={rescan} disabled={busy}>⟳ Rescan all</button>
  <button class="primary" onclick={add} disabled={busy}>＋ Add folder</button>
</div>

{#if busy}<div class="muted">Scanning…</div>{/if}

<div class="list">
  {#each app.folders as f (f.id)}
    <div class="card row">
      <span class="ico">🗀</span>
      <div class="grow">
        <div class="path">{f.path}</div>
        <div class="muted">{f.video_count} video{f.video_count === 1 ? '' : 's'}</div>
      </div>
      <button class="danger" onclick={() => remove(f.id)}>Remove</button>
    </div>
  {:else}
    <div class="card empty">No folders yet. Click <b>Add folder</b> to pick one.</div>
  {/each}
</div>

<style>
  .head { display: flex; align-items: center; gap: 10px; margin-bottom: 18px; }
  .list { display: flex; flex-direction: column; gap: 10px; }
  .ico { font-size: 22px; color: var(--primary); }
  .path { font-weight: 600; word-break: break-all; user-select: text; }
  .empty { text-align: center; padding: 36px; color: var(--muted); }
</style>
