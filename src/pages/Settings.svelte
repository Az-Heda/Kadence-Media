<script lang="ts">
  import { api } from '../lib/api';
  import { app, guarded, saveSetting, toast, volumeMax } from '../lib/store.svelte';
  import LayoutGrid from '@lucide/svelte/icons/layout-grid';
  import Table2 from '@lucide/svelte/icons/table-2';
  import GripVertical from '@lucide/svelte/icons/grip-vertical';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import { columnDef, resolveColumns } from '../lib/columns';
  import { dragHandlers, dropMark } from '../lib/dnd';

  let volMax = $state(Number(app.settings.volume_max) || 300);
  let mpvPath = $state(app.settings.mpv_path);
  let ffmpegPath = $state(app.settings.ffmpeg_path);
  let dbPath = $state('');
  guarded(async () => (dbPath = await api.databasePath()));

  let gridSize = $state(Number(app.settings.grid_page_size) || 60);
  let tableSize = $state(Number(app.settings.table_page_size) || 20);
  async function commitPageSize(key: 'grid_page_size' | 'table_page_size', value: number, fallback: number) {
    const v = Math.min(1000, Math.max(1, Math.round(value) || fallback));
    if (key === 'grid_page_size') gridSize = v; else tableSize = v;
    await saveSetting(key, String(v));
  }

  // Table columns: order + visibility, stored as JSON in the `table_columns` setting.
  const columns = $derived(resolveColumns(app.settings.table_columns));
  const columnsCustom = $derived(app.settings.table_columns !== '');
  function saveColumns(list: { id: string; visible: boolean }[]) {
    saveSetting('table_columns', JSON.stringify(list));
  }
  /** Drag-and-drop: put row `from` before/after row `target` (rows are identified by their index). */
  function moveColumn(from: number, target: number, after: boolean) {
    const list = columns.map((c) => ({ ...c }));
    const [item] = list.splice(from, 1);
    const t = target > from ? target - 1 : target;
    list.splice(after ? t + 1 : t, 0, item);
    saveColumns(list);
  }
  function setColumnVisible(i: number, visible: boolean) {
    const list = columns.map((c) => ({ ...c }));
    // Keep at least one column visible.
    if (!visible && list.filter((c) => c.visible).length <= 1) return;
    list[i].visible = visible;
    saveColumns(list);
  }

  async function commitVolMax() {
    const v = Math.min(1000, Math.max(100, Math.round(volMax) || 300));
    volMax = v;
    await saveSetting('volume_max', String(v));
    if (app.player.volume > v) await guarded(() => api.setVolume(v));
  }
</script>

<h1>Settings</h1>
<div class="muted intro">Changes are saved automatically.</div>

<section class="card">
  <h2>Audio</h2>
  <div class="field">
    <div class="grow">
      <div class="label">Maximum volume</div>
      <div class="muted">Boost quiet videos beyond 100%. The volume slider in the player goes up to this value.</div>
    </div>
    <div class="row">
      <input type="range" min="100" max="1000" step="25" bind:value={volMax} onchange={commitVolMax} style="width:180px" />
      <input type="number" min="100" max="1000" step="25" bind:value={volMax} onchange={commitVolMax} style="width:84px" />
      <span>%</span>
    </div>
  </div>
  <div class="muted hint">Current limit: {volumeMax()}%. Above ~200% some videos may distort.</div>
</section>

<section class="card">
  <h2>Library</h2>
  <div class="field">
    <div class="grow">
      <div class="label">Library view</div>
      <div class="muted">Show videos as a grid of cards or as a table.</div>
    </div>
    <div class="seg">
      <button class:on={app.settings.library_view !== 'table'} onclick={() => saveSetting('library_view', 'grid')}>
        <LayoutGrid size={15} /> Grid
      </button>
      <button class:on={app.settings.library_view === 'table'} onclick={() => saveSetting('library_view', 'table')}>
        <Table2 size={15} /> Table
      </button>
    </div>
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">Videos per page (grid)</div>
      <div class="muted">How many cards the library shows on each page in grid view.</div>
    </div>
    <input type="number" min="1" max="1000" step="10" bind:value={gridSize} style="width:84px"
      onchange={() => commitPageSize('grid_page_size', gridSize, 60)} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">Videos per page (table)</div>
      <div class="muted">How many rows the library shows on each page in table view.</div>
    </div>
    <input type="number" min="1" max="1000" step="5" bind:value={tableSize} style="width:84px"
      onchange={() => commitPageSize('table_page_size', tableSize, 20)} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">Show hidden videos</div>
      <div class="muted">Adds a "Hidden" tab to the library to see the videos you hid from their right-click menu. They never appear in the other tabs.</div>
    </div>
    <input type="checkbox" checked={app.settings.show_hidden === 'true'}
      onchange={(e) => { saveSetting('show_hidden', String(e.currentTarget.checked)); if (!e.currentTarget.checked) app.filter.hidden_only = false; }} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">Table column widths</div>
      <div class="muted">Drag a column's right edge in the table to resize it; widths are remembered.</div>
    </div>
    <button disabled={!app.settings.col_widths} onclick={() => saveSetting('col_widths', '')}>Reset column widths</button>
  </div>
</section>

{#if app.settings.library_view === 'table'}
  <section class="card">
    <h2>Table columns</h2>
    <div class="muted hint">Choose which columns the library table shows; drag the rows to change their order. The drag handle used to reorder playlists appears automatically.</div>
    <div class="cols">
      {#each columns as c, i (c.id)}
        <div class="colrow {dropMark(i)}" draggable="true" {...dragHandlers(i, 'y', moveColumn)}>
          <span class="handle" title="Drag to reorder"><GripVertical size={16} /></span>
          <input type="checkbox" checked={c.visible} onchange={(e) => setColumnVisible(i, e.currentTarget.checked)} aria-label="Show {columnDef(c.id).name}" />
          <span class="grow" class:off={!c.visible}>{columnDef(c.id).name}</span>
        </div>
      {/each}
    </div>
    <div class="row" style="margin-top:10px">
      <button disabled={!columnsCustom} onclick={() => saveSetting('table_columns', '')}><RotateCcw size={14} /> Reset order and visibility</button>
    </div>
  </section>
{/if}

<section class="card">
  <h2>Player</h2>
  <div class="field">
    <div class="grow">
      <div class="label">Escape key</div>
      <div class="muted">What <kbd>Esc</kbd> does in the player (it always leaves fullscreen first).</div>
    </div>
    <select value={app.settings.escape_action} onchange={(e) => saveSetting('escape_action', e.currentTarget.value)}>
      <option value="stop">Stop the video and return to the library</option>
      <option value="back">Return to the library, keep playing</option>
      <option value="none">Do nothing else</option>
    </select>
  </div>
</section>

<section class="card">
  <h2>Playback engine</h2>
  <div class="field">
    <div class="grow">
      <div class="label">Embed video in the window</div>
      <div class="muted">Turn off if the video doesn't appear: mpv will use its own window instead.</div>
    </div>
    <input type="checkbox" checked={app.settings.embed_video === 'true'}
      onchange={(e) => saveSetting('embed_video', String(e.currentTarget.checked))} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">mpv executable</div>
      <div class="muted">Leave empty to use the one next to the app, or <code>mpv</code> from PATH.</div>
    </div>
    <input type="text" placeholder="mpv" bind:value={mpvPath} style="width:260px"
      onchange={() => { saveSetting('mpv_path', mpvPath.trim()); toast('Applies the next time the player starts'); }} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">ffmpeg executable</div>
      <div class="muted">Used for thumbnails. Leave empty to use the one next to the app, or <code>ffmpeg</code> from PATH.</div>
    </div>
    <input type="text" placeholder="ffmpeg" bind:value={ffmpegPath} style="width:260px"
      onchange={() => { saveSetting('ffmpeg_path', ffmpegPath.trim()); toast('Applies the next time thumbnails are generated'); }} />
  </div>
</section>

<section class="card">
  <h2>Database</h2>
  <div class="field">
    <div class="grow">
      <div class="label">Library file</div>
      <div class="muted">Stored next to the executable. If that folder isn't writable the app falls back to its user data folder.</div>
    </div>
  </div>
  <code class="dbpath">{dbPath}</code>
</section>

<style>
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 8px; overflow: hidden; }
  .seg button { border: 0; border-radius: 0; background: var(--surface); }
  .seg button.on { background: var(--primary-soft); color: var(--primary); }
  .dbpath { display: block; word-break: break-all; user-select: text; }
  .cols { display: flex; flex-direction: column; margin-top: 8px; }
  .colrow { display: flex; align-items: center; gap: 10px; padding: 3px 0; border-bottom: 1px solid var(--border); }
  .colrow:last-child { border-bottom: 0; }
  .handle { display: inline-flex; color: var(--muted); cursor: grab; }
  .colrow.drop-before { box-shadow: inset 0 3px 0 var(--primary); }
  .colrow.drop-after { box-shadow: inset 0 -3px 0 var(--primary); }
  .off { color: var(--muted); text-decoration: line-through; }
  .intro { margin-bottom: 18px; }
  section { margin-bottom: 16px; }
  .field { display: flex; align-items: center; gap: 18px; padding: 8px 0; }
  .label { font-weight: 600; margin-bottom: 2px; }
  .hint { font-size: 12px; margin-top: 4px; }
  input[type='checkbox'] { width: 20px; height: 20px; accent-color: var(--primary); }
  kbd { background: var(--surface-2); border: 1px solid var(--border); border-radius: 4px; padding: 0 5px; font-size: 12px; }
  code { background: var(--surface-2); padding: 1px 5px; border-radius: 4px; }
</style>
