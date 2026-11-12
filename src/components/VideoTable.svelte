<script lang="ts">
  import { fmtDateTime, fmtSize, fmtTime, type SortBy, type Video } from '../lib/api';
  import { app, saveSetting, setRating, setSort, toggleWatched } from '../lib/store.svelte';
  import { COLUMNS, DRAG_WIDTH, columnDef, resolveColumns } from '../lib/columns';
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

  // Visible columns, in the order chosen in Settings. The drag handle is extra and playlist-only.
  const shown = $derived(resolveColumns(app.settings.table_columns).filter((c) => c.visible).map((c) => columnDef(c.id)));
  const reorder = $derived(!!playlist?.canReorder);

  // ---- resizable columns (widths persisted in the `col_widths` setting as JSON) ----
  const DEFAULTS: Record<string, number> = Object.fromEntries(COLUMNS.map((c) => [c.id, c.width]));
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
  const totalWidth = $derived(shown.reduce((a, c) => a + widths[c.id], reorder ? DRAG_WIDTH : 0));

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
      {#if reorder}<col style:width="{DRAG_WIDTH}px" />{/if}
      {#each shown as c (c.id)}<col style:width="{widths[c.id]}px" />{/each}
    </colgroup>
    <thead>
      <tr>
        {#if reorder}<th></th>{/if}
        {#each shown as c (c.id)}
          <th class:num={c.num}>
            {#if c.sort}
              <button class="sort" class:active={app.filter.sort_by === c.sort} onclick={() => setSort(c.sort as SortBy)}>
                {c.label}
                {#if app.filter.sort_by === c.sort}
                  {#if app.filter.sort_desc}<ArrowDown size={13} />{:else}<ArrowUp size={13} />{/if}
                {/if}
              </button>
            {:else}{c.label}{/if}
            {@render grip(c.id)}
          </th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each videos as v (v.id)}
        <tr class="{dropMark(v.id)}" class:watched={v.watched} ondblclick={() => onplay(v)}
          draggable={reorder ? 'true' : undefined}
          {...reorder ? dragHandlers(v.id, 'y', playlist!.onmove) : {}}>
          {#if reorder}
            <td class="cell"><span class="grip" title="Drag to reorder"><GripVertical size={15} /></span></td>
          {/if}
          {#each shown as c (c.id)}
            {#if c.id === 'play'}
              <td class="cell"><button class="ghost" onclick={() => onplay(v)} title="Play"><Play size={15} fill="currentColor" /></button></td>
            {:else if c.id === 'watched'}
              <td class="cell">
                <button class="ghost" onclick={() => toggleWatched(v)} title="Toggle watched">
                  {#if v.watched}<Eye size={15} />{:else}<EyeOff size={15} />{/if}
                </button>
              </td>
            {:else if c.id === 'rating'}
              <td class="cell"><StarRating value={v.rating} size={14} onchange={(r) => setRating(v, r)} /></td>
            {:else if c.id === 'title'}
              <td class="title cell" title={v.path}>{v.title}</td>
            {:else if c.id === 'created'}
              <td class="muted cell">{fmtDateTime(v.created)}</td>
            {:else if c.id === 'duration'}
              <td class="num muted cell">{v.duration && v.duration > 0 ? fmtTime(v.duration) : '—'}</td>
            {:else if c.id === 'size'}
              <td class="num muted cell">{fmtSize(v.size)}</td>
            {:else if c.id === 'path'}
              <td class="muted cell" title={folderOf(v)}>{folderOf(v)}</td>
            {:else if c.id === 'tags'}
              <td>
                <div class="tags">
                  {#each v.tags as t (t.id)}
                    <span class="chip"><span class="dot" style:background={t.color}></span>{t.name}</span>
                  {/each}
                </div>
              </td>
            {:else if c.id === 'actions'}
              <td class="cell">
                {#if playlist}
                  <button class="ghost danger" onclick={() => playlist.onremove(v.id)} title="Remove from this playlist"><ListX size={15} /></button>
                {/if}
                <button class="ghost" onclick={() => onmanage(v)} title="Tags & playlists"><Tag size={15} /></button>
              </td>
            {/if}
          {/each}
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
