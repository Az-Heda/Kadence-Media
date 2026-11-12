<script lang="ts">
  import { app, applySavedTheme, currentColors, DEFAULT_PRIMARY, deleteTheme, saveCurrentTheme, saveSetting, savedThemes, THEME_COLORS, type SavedTheme } from '../lib/store.svelte';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
  import Save from '@lucide/svelte/icons/save';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import Check from '@lucide/svelte/icons/check';

  const presets = ['#d4a017', '#e8b923', '#f2994a', '#e5484d', '#3dd68c', '#4cc2ff', '#a78bfa', '#f472b6'];
  const themeCustom = $derived(THEME_COLORS.some((c) => app.settings[c.key] !== ''));
  async function resetTheme() {
    for (const c of THEME_COLORS) if (app.settings[c.key] !== '') await saveSetting(c.key, '');
  }

  let name = $state('');
  let deleting = $state<string | null>(null);
  const themes = $derived(savedThemes());
  async function save() {
    const n = name.trim();
    if (!n) return;
    await saveCurrentTheme(n);
    name = '';
  }
  const isActive = (t: SavedTheme) => {
    const cur = currentColors();
    return Object.keys(cur).every((k) => (t.colors[k as keyof typeof cur] ?? (k === 'primary_color' ? DEFAULT_PRIMARY : '')) === cur[k as keyof typeof cur]);
  };
  const swatches = (t: SavedTheme) => [
    t.colors.bg_color || '#0d0d10', t.colors.surface_color || '#16161b', t.colors.text_color || '#ececf1', t.colors.primary_color || DEFAULT_PRIMARY,
  ];
</script>

<h1>Themes</h1>
<div class="muted intro">Customize the colors and save them as named themes. Changes are saved automatically.</div>

<section class="card">
  <h2>Saved themes</h2>
  <form class="row" onsubmit={(e) => { e.preventDefault(); save(); }}>
    <input type="text" class="grow" placeholder="Theme name (saves the current colors)…" bind:value={name} />
    <button class="primary" type="submit" disabled={!name.trim()}><Save size={15} /> Save theme</button>
  </form>
  <div class="hint muted">Saving with an existing name replaces that theme.</div>
  <div class="list">
    {#each themes as t (t.name)}
      <div class="trow" class:active={isActive(t)}>
        <span class="sw">{#each swatches(t) as c}<i style:background={c}></i>{/each}</span>
        <span class="grow tname">{t.name}{#if isActive(t)}<Check size={14} />{/if}</span>
        <button onclick={() => applySavedTheme(t)} disabled={isActive(t)}>Apply</button>
        <button class="ghost danger" onclick={() => (deleting = t.name)} title="Delete theme"><Trash2 size={15} /></button>
      </div>
    {:else}
      <div class="muted">No saved themes yet.</div>
    {/each}
  </div>
</section>

<section class="card">
  <h2>Colors</h2>
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
  {#each THEME_COLORS as c (c.key)}
    <div class="field">
      <div class="grow">
        <div class="label">{c.label}</div>
        <div class="muted">{c.hint}</div>
      </div>
      <div class="row">
        <input type="color" value={app.settings[c.key] || c.fallback}
          onchange={(e) => saveSetting(c.key, e.currentTarget.value)} />
        <button class="ghost" disabled={app.settings[c.key] === ''} onclick={() => saveSetting(c.key, '')}>Reset</button>
      </div>
    </div>
  {/each}
  <div class="row" style="margin-top:6px">
    <button disabled={!themeCustom} onclick={resetTheme}><RotateCcw size={14} /> Reset all colors</button>
  </div>
</section>


{#if deleting}
  <ConfirmDialog title="Delete this theme?" onconfirm={async () => { const n = deleting!; deleting = null; await deleteTheme(n); }} oncancel={() => (deleting = null)}>
    <p>The saved theme <b>{deleting}</b> will be deleted. Your current colors are not changed. This cannot be undone.</p>
  </ConfirmDialog>
{/if}

<style>
  .intro { margin-bottom: 18px; }
  section { margin-bottom: 16px; }
  .field { display: flex; align-items: center; gap: 18px; padding: 8px 0; }
  .label { font-weight: 600; margin-bottom: 2px; }
  .hint { font-size: 12px; margin: 6px 0 10px; }
  .list { display: flex; flex-direction: column; gap: 6px; }
  .trow { display: flex; align-items: center; gap: 10px; padding: 6px 8px; border: 1px solid var(--border); border-radius: 8px; }
  .trow.active { border-color: var(--primary); background: var(--primary-soft); }
  .tname { display: flex; align-items: center; gap: 6px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .sw { display: inline-flex; border-radius: 6px; overflow: hidden; border: 1px solid var(--border); }
  .sw i { width: 18px; height: 22px; display: block; }
  .swatch { width: 26px; height: 26px; padding: 0; border-radius: 50%; border: 2px solid transparent; }
  .swatch.on { border-color: #fff; }
</style>
