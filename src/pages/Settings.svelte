<script lang="ts">
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { api } from '../lib/api';
  import { app, guarded, refreshAll, saveSetting, toast, volumeMax } from '../lib/store.svelte';
  import LayoutGrid from '@lucide/svelte/icons/layout-grid';
  import Table2 from '@lucide/svelte/icons/table-2';
  import GripVertical from '@lucide/svelte/icons/grip-vertical';
  import Upload from '@lucide/svelte/icons/upload';
  import Download from '@lucide/svelte/icons/download';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import { colName, resolveColumns } from '../lib/columns';
  import { DATE_SEPARATORS, LANGUAGES, fmtDateTime, t, type Key } from '../lib/i18n.svelte';
  import { dragHandlers, dropMark } from '../lib/dnd';

  // Preview of the date/time format (a fixed sample moment, so it is stable while the settings change).
  const SAMPLE = Math.floor(new Date(2026, 2, 7, 15, 4, 9).getTime() / 1000);
  const dateOrders: [string, Key][] = [['dmy', 'settings.order_dmy'], ['mdy', 'settings.order_mdy'], ['ymd', 'settings.order_ymd']];

  let volMax = $state(Number(app.settings.volume_max) || 300);
  let mpvPath = $state(app.settings.mpv_path);
  let ffmpegPath = $state(app.settings.ffmpeg_path);
  let dbPath = $state('');
  guarded(async () => (dbPath = await api.databasePath()));

  // Export / import of tag, playlist and rule definitions (never videos).
  let xTags = $state(true), xPlaylists = $state(true), xRules = $state(true);
  const JSON_FILTER = [{ name: 'JSON', extensions: ['json'] }];
  async function exportData() {
    if (!(xTags || xPlaylists || xRules)) return toast(t('settings.export_pick'), true);
    const path = await save({ defaultPath: 'kadence-media-export.json', filters: JSON_FILTER });
    if (!path) return;
    const ok = await guarded(async () => (await api.exportData(path, xTags, xPlaylists, xRules), true));
    if (ok) toast(t('settings.export_done', { path }));
  }
  async function importData() {
    const path = await open({ multiple: false, filters: JSON_FILTER });
    if (typeof path !== 'string') return;
    const r = await guarded(() => api.importData(path));
    if (!r) return;
    await refreshAll();
    const skipped = r.tags_skipped + r.playlists_skipped + r.rules_skipped;
    let msg = t('settings.import_done', { tags: r.tags_added, playlists: r.playlists_added, rules: r.rules_added, skipped });
    if (r.rules_invalid.length) msg += '\n' + t('settings.import_invalid', { names: r.rules_invalid.join(', ') });
    toast(msg, r.rules_invalid.length > 0);
  }

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

<h1>{t('settings.title')}</h1>
<div class="muted intro">{t('settings.intro')}</div>

<section class="card">
  <h2>{t('settings.locale_card')}</h2>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.language')}</div>
      <div class="muted">{t('settings.language_hint')}</div>
    </div>
    <select value={app.settings.language} onchange={(e) => saveSetting('language', e.currentTarget.value)}>
      <option value="system">{t('settings.lang_system')}</option>
      {#each LANGUAGES as l (l.id)}<option value={l.id}>{l.name}</option>{/each}
    </select>
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.date_order')}</div>
      <div class="muted">{t('settings.date_order_hint')}</div>
    </div>
    <select value={app.settings.date_order} onchange={(e) => saveSetting('date_order', e.currentTarget.value)}>
      {#each dateOrders as [val, label]}<option value={val}>{t(label)}</option>{/each}
    </select>
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.date_separator')}</div>
      <div class="muted">{t('settings.date_separator_hint')}</div>
    </div>
    <select value={app.settings.date_separator} onchange={(e) => saveSetting('date_separator', e.currentTarget.value)}>
      {#each DATE_SEPARATORS as sep}<option value={sep}>{sep}</option>{/each}
    </select>
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.time_format')}</div>
      <div class="muted">{t('settings.time_format_hint')}</div>
    </div>
    <select value={app.settings.time_format} onchange={(e) => saveSetting('time_format', e.currentTarget.value)}>
      <option value="24h">{t('settings.time_24')}</option>
      <option value="12h">{t('settings.time_12')}</option>
    </select>
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.show_seconds')}</div>
      <div class="muted">{t('settings.show_seconds_hint')}</div>
    </div>
    <input type="checkbox" checked={app.settings.time_seconds !== 'false'}
      onchange={(e) => saveSetting('time_seconds', String(e.currentTarget.checked))} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.layout')}</div>
      <div class="muted">{t('settings.layout_hint')}</div>
    </div>
    <select value={app.settings.date_layout} onchange={(e) => saveSetting('date_layout', e.currentTarget.value)}>
      <option value="time_first">{t('settings.layout_time_first')}</option>
      <option value="date_first">{t('settings.layout_date_first')}</option>
    </select>
  </div>
  <div class="muted hint">{t('settings.preview')}: <code>{fmtDateTime(SAMPLE)}</code></div>
</section>

<section class="card">
  <h2>{t('settings.audio')}</h2>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.max_volume')}</div>
      <div class="muted">{t('settings.max_volume_hint')}</div>
    </div>
    <div class="row">
      <input type="range" min="100" max="1000" step="25" bind:value={volMax} onchange={commitVolMax} style="width:180px" />
      <input type="number" min="100" max="1000" step="25" bind:value={volMax} onchange={commitVolMax} style="width:84px" />
      <span>%</span>
    </div>
  </div>
  <div class="muted hint">{t('settings.volume_limit', { limit: volumeMax() })}</div>
</section>

<section class="card">
  <h2>{t('settings.library')}</h2>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.view')}</div>
      <div class="muted">{t('settings.view_hint')}</div>
    </div>
    <div class="seg">
      <button class:on={app.settings.library_view !== 'table'} onclick={() => saveSetting('library_view', 'grid')}>
        <LayoutGrid size={15} /> {t('settings.grid')}
      </button>
      <button class:on={app.settings.library_view === 'table'} onclick={() => saveSetting('library_view', 'table')}>
        <Table2 size={15} /> {t('settings.table')}
      </button>
    </div>
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.page_grid')}</div>
      <div class="muted">{t('settings.page_grid_hint')}</div>
    </div>
    <input type="number" min="1" max="1000" step="10" bind:value={gridSize} style="width:84px"
      onchange={() => commitPageSize('grid_page_size', gridSize, 60)} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.page_table')}</div>
      <div class="muted">{t('settings.page_table_hint')}</div>
    </div>
    <input type="number" min="1" max="1000" step="5" bind:value={tableSize} style="width:84px"
      onchange={() => commitPageSize('table_page_size', tableSize, 20)} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.show_hidden')}</div>
      <div class="muted">{t('settings.show_hidden_hint')}</div>
    </div>
    <input type="checkbox" checked={app.settings.show_hidden === 'true'}
      onchange={(e) => { saveSetting('show_hidden', String(e.currentTarget.checked)); if (!e.currentTarget.checked) app.filter.hidden_only = false; }} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.col_widths')}</div>
      <div class="muted">{t('settings.col_widths_hint')}</div>
    </div>
    <button disabled={!app.settings.col_widths} onclick={() => saveSetting('col_widths', '')}>{t('settings.reset_widths')}</button>
  </div>
</section>

{#if app.settings.library_view === 'table'}
  <section class="card">
    <h2>{t('settings.table_columns')}</h2>
    <div class="muted hint">{t('settings.columns_hint')}</div>
    <div class="cols">
      {#each columns as c, i (c.id)}
        <div class="colrow {dropMark(i)}" draggable="true" {...dragHandlers(i, 'y', moveColumn)}>
          <span class="handle" title={t('dnd.drag_reorder')}><GripVertical size={16} /></span>
          <input type="checkbox" checked={c.visible} onchange={(e) => setColumnVisible(i, e.currentTarget.checked)} aria-label={t('settings.show_column', { name: colName(c.id) })} />
          <span class="grow" class:off={!c.visible}>{colName(c.id)}</span>
        </div>
      {/each}
    </div>
    <div class="row" style="margin-top:10px">
      <button disabled={!columnsCustom} onclick={() => saveSetting('table_columns', '')}><RotateCcw size={14} /> {t('settings.reset_columns')}</button>
    </div>
  </section>
{/if}

<section class="card">
  <h2>{t('settings.player')}</h2>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.escape_key')}</div>
      <div class="muted">{@html t('settings.escape_hint')}</div>
    </div>
    <select value={app.settings.escape_action} onchange={(e) => saveSetting('escape_action', e.currentTarget.value)}>
      <option value="stop">{t('settings.esc_stop')}</option>
      <option value="back">{t('settings.esc_back')}</option>
      <option value="none">{t('settings.esc_none')}</option>
    </select>
  </div>
</section>

<section class="card">
  <h2>{t('settings.engine')}</h2>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.embed')}</div>
      <div class="muted">{t('settings.embed_hint')}</div>
    </div>
    <input type="checkbox" checked={app.settings.embed_video === 'true'}
      onchange={(e) => saveSetting('embed_video', String(e.currentTarget.checked))} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.mpv')}</div>
      <div class="muted">{@html t('settings.mpv_hint')}</div>
    </div>
    <input type="text" placeholder="mpv" bind:value={mpvPath} style="width:260px"
      onchange={() => { saveSetting('mpv_path', mpvPath.trim()); toast(t('settings.mpv_applied')); }} />
  </div>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.ffmpeg')}</div>
      <div class="muted">{@html t('settings.ffmpeg_hint')}</div>
    </div>
    <input type="text" placeholder="ffmpeg" bind:value={ffmpegPath} style="width:260px"
      onchange={() => { saveSetting('ffmpeg_path', ffmpegPath.trim()); toast(t('settings.ffmpeg_applied')); }} />
  </div>
</section>

<section class="card">
  <h2>{t('settings.transfer')}</h2>
  <div class="muted">{t('settings.transfer_hint')}</div>
  <div class="checks">
    <label><input type="checkbox" bind:checked={xTags} /> {t('settings.transfer_tags')}</label>
    <label><input type="checkbox" bind:checked={xPlaylists} /> {t('settings.transfer_playlists')}</label>
    <label><input type="checkbox" bind:checked={xRules} /> {t('settings.transfer_rules')}</label>
  </div>
  <div class="row">
    <button onclick={exportData}><Upload size={15} /> {t('settings.export')}</button>
    <button onclick={importData}><Download size={15} /> {t('settings.import')}</button>
  </div>
</section>

<section class="card">
  <h2>{t('settings.database')}</h2>
  <div class="field">
    <div class="grow">
      <div class="label">{t('settings.db_file')}</div>
      <div class="muted">{t('settings.db_hint')}</div>
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
  .checks { display: flex; flex-direction: column; gap: 8px; margin: 12px 0; }
  .checks label { display: flex; align-items: center; gap: 10px; cursor: pointer; }
  .intro { margin-bottom: 18px; }
  section { margin-bottom: 16px; }
  .field { display: flex; align-items: center; gap: 18px; padding: 8px 0; }
  .label { font-weight: 600; margin-bottom: 2px; }
  .hint { font-size: 12px; margin-top: 4px; }
  input[type='checkbox'] { width: 20px; height: 20px; accent-color: var(--primary); }
  :global(kbd) { background: var(--surface-2); border: 1px solid var(--border); border-radius: 4px; padding: 0 5px; font-size: 12px; }
  :global(code) { background: var(--surface-2); padding: 1px 5px; border-radius: 4px; }
</style>
