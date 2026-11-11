<script lang="ts">
  import { fmtSize, fmtTime, type SortBy, type Video } from '../lib/api';
  import { app, setRating, setSort, toggleWatched } from '../lib/store.svelte';
  import StarRating from './StarRating.svelte';
  import { dragHandlers, dropMark, type PlaylistCtx } from '../lib/dnd';
  import GripVertical from '@lucide/svelte/icons/grip-vertical';
  import ListX from '@lucide/svelte/icons/list-x';
  import ArrowUp from '@lucide/svelte/icons/arrow-up';
  import ArrowDown from '@lucide/svelte/icons/arrow-down';
  import Play from '@lucide/svelte/icons/play';
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';
  import Tag from '@lucide/svelte/icons/tag';

  let { videos, onplay, onmanage, playlist = null }: {
    videos: Video[]; onplay: (v: Video) => void; onmanage: (v: Video) => void; playlist?: PlaylistCtx | null;
  } = $props();

  const folderOf = (v: Video) => v.path.split(/[\\/]/).slice(0, -1).join('/') || '/';
  const dateOf = (v: Video) => (v.created ? new Date(v.created * 1000).toLocaleDateString() : '');
</script>

<div class="wrap card">
  <table>
    <thead>
      <tr>
        <th class="narrow"></th>
        {#each [['title', 'Title', ''], ['path', 'Folder', ''], ['size', 'Size', 'num'], ['created', 'Created', ''], ['duration', 'Duration', 'num'], ['rating', 'Rating', '']] as [key, label, cls]}
          <th class={cls}>
            <button class="sort" class:active={app.filter.sort_by === key} onclick={() => setSort(key as SortBy)}>
              {label}
              {#if app.filter.sort_by === key}
                {#if app.filter.sort_desc}<ArrowDown size={13} />{:else}<ArrowUp size={13} />{/if}
              {/if}
            </button>
          </th>
        {/each}
        <th>Tags</th>
        <th class="narrow">Watched</th>
        <th class="narrow"></th>
      </tr>
    </thead>
    <tbody>
      {#each videos as v (v.id)}
        <tr class="{dropMark(v.id)}" class:watched={v.watched} ondblclick={() => onplay(v)}
          draggable={playlist?.canReorder ? 'true' : undefined}
          {...playlist?.canReorder ? dragHandlers(v.id, 'y', playlist.onmove) : {}}>
          <td class="narrow">
            {#if playlist?.canReorder}<span class="grip" title="Drag to reorder"><GripVertical size={15} /></span>{/if}
            <button class="ghost" onclick={() => onplay(v)} title="Play"><Play size={15} fill="currentColor" /></button>
          </td>
          <td class="title" title={v.path}>{v.title}</td>
          <td class="muted folder" title={folderOf(v)}>{folderOf(v)}</td>
          <td class="num muted">{fmtSize(v.size)}</td>
          <td class="muted">{dateOf(v)}</td>
          <td class="num muted">{v.duration && v.duration > 0 ? fmtTime(v.duration) : '—'}</td>
          <td><StarRating value={v.rating} size={14} onchange={(r) => setRating(v, r)} /></td>
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
            {#if playlist}
              <button class="ghost danger" onclick={() => playlist.onremove(v.id)} title="Remove from this playlist"><ListX size={15} /></button>
            {/if}
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
  .sort { background: none; border: 0; padding: 0; color: inherit; font: inherit; text-transform: inherit; letter-spacing: inherit; }
  .sort:hover, .sort.active { color: var(--primary); }
  th.num .sort { margin-left: auto; }
  tbody tr:last-child td { border-bottom: 0; }
  tbody tr:hover { background: var(--primary-soft); }
  .grip { display: inline-flex; vertical-align: middle; color: var(--muted); cursor: grab; margin-right: 2px; }
  tr.drop-before td { box-shadow: inset 0 3px 0 var(--primary); }
  tr.drop-after td { box-shadow: inset 0 -3px 0 var(--primary); }
  .narrow { width: 1%; white-space: nowrap; }
  .num { text-align: right; white-space: nowrap; }
  .title { font-weight: 600; max-width: 380px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .folder { max-width: 260px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tags { display: flex; flex-wrap: wrap; gap: 5px; }
  tr.watched .title { opacity: .65; }
  td button { padding: 4px 8px; }
</style>
