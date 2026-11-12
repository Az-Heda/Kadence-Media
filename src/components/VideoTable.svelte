<script lang="ts">
  import { fmtDateTime, fmtSize, fmtTime, type SortBy, type Video } from '../lib/api';
  import { app, saveSetting, setRating, setSort, toggleWatched } from '../lib/store.svelte';
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
  const dateOf = (v: Video) => fmtDateTime(v.created);

  // ---- resizable columns (widths persisted in the `col_widths` setting as JSON) ----
  const DEFAULTS: Record<string, number> = {
    play: 90, title: 320, path: 220, size: 100, created: 170, duration: 100, rating: 120, tags: 220, watched: 80, actions: 90,
  };
  const MIN_W = 50;
  function parseWidths(raw: string): Record<string, number> {
    try {
      const o = JSON.parse(raw || '{}');
      return Object.fromEntries(Object.entries(o).filter(([k, v]) => k in DEFAULTS && typeof v === 'number' && v >= MIN_W)) as Record<string, number>;
    } catch { return {}; }
  }
  let widths = $state<Record<string, number>>({ ...DEFAULTS, ...parseWidths(app.settings.col_widths) });
  let resizing = $state(false);
  // Follow external changes (e.g. "Reset column widths" in Settings).
  $effect(() => { if (!resizing) widths = { ...DEFAULTS, ...parseWidths(app.settings.col_widths) }; });
  const totalWidth = $derived(Object.values(widths).reduce((a, b) => a + b, 0));

  function persist() {
    const custom = Object.fromEntries(Object.entries(widths).filter(([k, v]) => v !== DEFAULTS[k]));
    saveSetting('col_widths', Object.keys(custom).length ? JSON.stringify(custom) : '');
  }
  function startResize(e: PointerEvent, key: string) {
    e.preventDefault();
    e.stopPropagation();
    const startX = e.clientX, startW = widths[key];
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    resizing = true;
    const move = (ev: PointerEvent) => { widths[key] = Math.max(MIN_W, Math.round(startW + ev.clientX - startX)); };
    const up = () => {
      target.removeEventListener('pointermove', move);
      target.removeEventListener('pointerup', up);
      target.removeEventListener('pointercancel', up);
      resizing = false;
      persist();
    };
    target.addEventListener('pointermove', move);
    target.addEventListener('pointerup', up);
    target.addEventListener('pointercancel', up);
  }
  function resetWidth(key: string) { widths[key] = DEFAULTS[key]; persist(); }
</script>

{#snippet grip(key: string)}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span class="grip-h" onpointerdown={(e) => startResize(e, key)} ondblclick={() => resetWidth(key)} title="Drag to resize, double-click to reset"></span>
{/snippet}

<div class="wrap card">
  <table style:width="{totalWidth}px">
    <colgroup>
      {#each Object.keys(DEFAULTS) as k}<col style:width="{widths[k]}px" />{/each}
    </colgroup>
    <thead>
      <tr>
        <th>{@render grip('play')}</th>
        {#each [['title', 'Title', ''], ['path', 'Folder', ''], ['size', 'Size', 'num'], ['created', 'Created', ''], ['duration', 'Duration', 'num'], ['rating', 'Rating', '']] as [key, label, cls]}
          <th class={cls}>
            <button class="sort" class:active={app.filter.sort_by === key} onclick={() => setSort(key as SortBy)}>
              {label}
              {#if app.filter.sort_by === key}
                {#if app.filter.sort_desc}<ArrowDown size={13} />{:else}<ArrowUp size={13} />{/if}
              {/if}
            </button>
            {@render grip(key)}
          </th>
        {/each}
        <th>Tags{@render grip('tags')}</th>
        <th>Watched{@render grip('watched')}</th>
        <th>{@render grip('actions')}</th>
      </tr>
    </thead>
    <tbody>
      {#each videos as v (v.id)}
        <tr class="{dropMark(v.id)}" class:watched={v.watched} ondblclick={() => onplay(v)}
          draggable={playlist?.canReorder ? 'true' : undefined}
          {...playlist?.canReorder ? dragHandlers(v.id, 'y', playlist.onmove) : {}}>
          <td class="cell">
            {#if playlist?.canReorder}<span class="grip" title="Drag to reorder"><GripVertical size={15} /></span>{/if}
            <button class="ghost" onclick={() => onplay(v)} title="Play"><Play size={15} fill="currentColor" /></button>
          </td>
          <td class="title cell" title={v.path}>{v.title}</td>
          <td class="muted cell" title={folderOf(v)}>{folderOf(v)}</td>
          <td class="num muted cell">{fmtSize(v.size)}</td>
          <td class="muted cell">{dateOf(v)}</td>
          <td class="num muted cell">{v.duration && v.duration > 0 ? fmtTime(v.duration) : '—'}</td>
          <td class="cell"><StarRating value={v.rating} size={14} onchange={(r) => setRating(v, r)} /></td>
          <td>
            <div class="tags">
              {#each v.tags as t (t.id)}
                <span class="chip"><span class="dot" style:background={t.color}></span>{t.name}</span>
              {/each}
            </div>
          </td>
          <td class="cell">
            <button class="ghost" onclick={() => toggleWatched(v)} title="Toggle watched">
              {#if v.watched}<Eye size={15} />{:else}<EyeOff size={15} />{/if}
            </button>
          </td>
          <td class="cell">
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
  table { min-width: 100%; table-layout: fixed; border-collapse: collapse; }
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
  .cell { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .grip-h { position: absolute; top: 0; right: 0; bottom: 0; width: 8px; cursor: col-resize; touch-action: none; }
  .grip-h:hover, .grip-h:active { background: var(--primary); opacity: .6; }
  th { overflow: hidden; white-space: nowrap; }
  .num { text-align: right; white-space: nowrap; }
  .title { font-weight: 600; }
  .tags { display: flex; flex-wrap: wrap; gap: 5px; }
  tr.watched .title { opacity: .65; }
  td button { padding: 4px 8px; }
</style>
