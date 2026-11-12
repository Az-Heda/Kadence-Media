<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { api } from '../lib/api';
  import { app, guarded, refreshAll, setDefaultFolder, toast } from '../lib/store.svelte';
  import FolderOpen from '@lucide/svelte/icons/folder-open';
  import FolderPlus from '@lucide/svelte/icons/folder-plus';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import Star from '@lucide/svelte/icons/star';
  import Trash2 from '@lucide/svelte/icons/trash-2';

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

  let confirming = $state<{ id: number; path: string; count: number } | null>(null);

  async function remove(id: number) {
    confirming = null;
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
  <button onclick={rescan} disabled={busy}><RefreshCw size={15} /> Rescan all</button>
  <button class="primary" onclick={add} disabled={busy}><FolderPlus size={15} /> Add folder</button>
</div>

{#if busy}<div class="muted">Scanning…</div>{/if}

<div class="list">
  {#each app.folders as f (f.id)}
    <div class="card row">
      <span class="ico"><FolderOpen size={22} /></span>
      <div class="grow">
        <div class="path">{f.path}{#if f.is_default}<span class="chip default">Default</span>{/if}</div>
        <div class="muted">{f.video_count} video{f.video_count === 1 ? '' : 's'}</div>
      </div>
      <button class:primary={f.is_default} onclick={() => setDefaultFolder(f.is_default ? null : f.id)}
        title={f.is_default ? 'Stop using as default' : 'Show only this folder in the library by default'}>
        <Star size={15} fill={f.is_default ? 'currentColor' : 'none'} /> {f.is_default ? 'Default' : 'Set as default'}
      </button>
      <button class="danger" onclick={() => (confirming = { id: f.id, path: f.path, count: f.video_count })}><Trash2 size={15} /> Remove</button>
    </div>
  {:else}
    <div class="card empty">No folders yet. Click <b>Add folder</b> to pick one.</div>
  {/each}
</div>

{#if confirming}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="backdrop" onclick={() => (confirming = null)}>
    <div class="dialog card" role="alertdialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
      <h1>Remove this folder?</h1>
      <div class="path">{confirming.path}</div>
      <p>
        Removing the folder deletes its {confirming.count} video{confirming.count === 1 ? '' : 's'} from the library,
        and <b>all the data attached to them will be permanently lost</b>: watched status, ratings, tags and playlist memberships.
        Your files on disk are not touched.
      </p>
      <div class="row buttons">
        <span class="grow"></span>
        <button onclick={() => (confirming = null)}>Cancel</button>
        <button class="primary danger-solid" onclick={() => remove(confirming!.id)}><Trash2 size={15} /> Remove folder</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop { position: fixed; inset: 0; background: #000a; display: grid; place-items: center; z-index: 40; }
  .dialog { width: min(480px, 92vw); }
  .dialog p { line-height: 1.5; }
  .buttons { margin-top: 16px; }
  button.danger-solid { background: var(--danger); border-color: var(--danger); color: #fff; }
  button.danger-solid:hover { background: var(--danger); filter: brightness(1.15); }
  .head { display: flex; align-items: center; gap: 10px; margin-bottom: 18px; }
  .list { display: flex; flex-direction: column; gap: 10px; }
  .ico { display: inline-flex; color: var(--primary); }
  .default { margin-left: 10px; border-color: var(--primary); color: var(--primary); background: var(--primary-soft); }
  .path { font-weight: 600; word-break: break-all; user-select: text; }
  .empty { text-align: center; padding: 36px; color: var(--muted); }
</style>
