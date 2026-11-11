<script lang="ts">
  import { fmtSize, type Video } from '../lib/api';
  import { toggleWatched } from '../lib/store.svelte';
  import Play from '@lucide/svelte/icons/play';
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';
  import Tag from '@lucide/svelte/icons/tag';

  let { videos, onplay, onmanage }: { videos: Video[]; onplay: (v: Video) => void; onmanage: (v: Video) => void } = $props();

  const folderOf = (v: Video) => v.path.split(/[\\/]/).slice(0, -1).join('/') || '/';
  const dateOf = (v: Video) => (v.modified ? new Date(v.modified * 1000).toLocaleDateString() : '');
</script>

<div class="wrap card">
  <table>
    <thead>
      <tr>
        <th class="narrow"></th>
        <th>Title</th>
        <th>Folder</th>
        <th class="num">Size</th>
        <th>Modified</th>
        <th>Tags</th>
        <th class="narrow">Watched</th>
        <th class="narrow"></th>
      </tr>
    </thead>
    <tbody>
      {#each videos as v (v.id)}
        <tr class:watched={v.watched} ondblclick={() => onplay(v)}>
          <td class="narrow">
            <button class="ghost" onclick={() => onplay(v)} title="Play"><Play size={15} fill="currentColor" /></button>
          </td>
          <td class="title" title={v.path}>{v.title}</td>
          <td class="muted folder" title={folderOf(v)}>{folderOf(v)}</td>
          <td class="num muted">{fmtSize(v.size)}</td>
          <td class="muted">{dateOf(v)}</td>
          <td>
            <div class="tags">
              {#each v.tags as t (t.id)}
                <span class="chip"><span class="dot" style:background={t.color}></span>{t.name}</span>
              {/each}
            </div>
          </td>
          <td class="narrow">
            <button class="ghost" onclick={() => toggleWatched(v)} title="Toggle watched">
              {#if v.watched}<Eye size={15} />{:else}<EyeOff size={15} />{/if}
            </button>
          </td>
          <td class="narrow">
            <button class="ghost" onclick={() => onmanage(v)} title="Tags & playlists"><Tag size={15} /></button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .wrap { padding: 0; overflow-x: auto; }
  table { width: 100%; border-collapse: collapse; }
  th { text-align: left; font-size: 12px; text-transform: uppercase; letter-spacing: .05em; color: var(--muted); padding: 10px 12px; border-bottom: 1px solid var(--border); position: sticky; top: 0; background: var(--surface); }
  td { padding: 6px 12px; border-bottom: 1px solid var(--border); vertical-align: middle; }
  tbody tr:last-child td { border-bottom: 0; }
  tbody tr:hover { background: var(--primary-soft); }
  .narrow { width: 1%; white-space: nowrap; }
  .num { text-align: right; white-space: nowrap; }
  .title { font-weight: 600; max-width: 380px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .folder { max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tags { display: flex; flex-wrap: wrap; gap: 5px; }
  tr.watched .title { opacity: .65; }
  td button { padding: 4px 8px; }
</style>
