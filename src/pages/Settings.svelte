<script lang="ts">
  import { api } from '../lib/api';
  import { app, DEFAULT_PRIMARY, guarded, saveSetting, toast, volumeMax } from '../lib/store.svelte';
  import LayoutGrid from '@lucide/svelte/icons/layout-grid';
  import Table2 from '@lucide/svelte/icons/table-2';

  const presets = ['#d4a017', '#e8b923', '#f2994a', '#e5484d', '#3dd68c', '#4cc2ff', '#a78bfa', '#f472b6'];

  let volMax = $state(Number(app.settings.volume_max) || 300);
  let mpvPath = $state(app.settings.mpv_path);
  let dbPath = $state('');
  guarded(async () => (dbPath = await api.databasePath()));

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
</section>

<section class="card">
  <h2>Theme</h2>
  <div class="field">
    <div class="grow">
      <div class="label">Primary color</div>
      <div class="muted">Accent used for buttons, highlights and the player.</div>
    </div>
    <div class="row">
      {#each presets as c}
        <button class="swatch" class:on={app.settings.primary_color === c} style:background={c}
          aria-label={c} onclick={() => saveSetting('primary_color', c)}></button>
      {/each}
      <input type="color" value={app.settings.primary_color}
        onchange={(e) => saveSetting('primary_color', e.currentTarget.value)} />
      <button class="ghost" onclick={() => saveSetting('primary_color', DEFAULT_PRIMARY)}>Reset</button>
    </div>
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
  .intro { margin-bottom: 18px; }
  section { margin-bottom: 16px; }
  .field { display: flex; align-items: center; gap: 18px; padding: 8px 0; }
  .label { font-weight: 600; margin-bottom: 2px; }
  .hint { font-size: 12px; margin-top: 4px; }
  .swatch { width: 26px; height: 26px; padding: 0; border-radius: 50%; border: 2px solid transparent; }
  .swatch.on { border-color: #fff; }
  input[type='checkbox'] { width: 20px; height: 20px; accent-color: var(--primary); }
  code { background: var(--surface-2); padding: 1px 5px; border-radius: 4px; }
</style>
