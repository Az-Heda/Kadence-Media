<script lang="ts">
  import { api, type Rule, type RuleInput } from '../lib/api';
  import { app, guarded, refreshAll, toast } from '../lib/store.svelte';
  import { t, th } from '../lib/i18n.svelte';
  import ConfirmDialog from '../components/ConfirmDialog.svelte';
  import Plus from '@lucide/svelte/icons/plus';
  import Pencil from '@lucide/svelte/icons/pencil';
  import Trash2 from '@lucide/svelte/icons/trash-2';
  import WandSparkles from '@lucide/svelte/icons/wand-sparkles';
  import Check from '@lucide/svelte/icons/check';
  import X from '@lucide/svelte/icons/x';
  import ListVideo from '@lucide/svelte/icons/list-video';

  let rules = $state<Rule[]>([]);
  let draft = $state<(RuleInput & { id: number | null }) | null>(null);
  let matchInfo = $state<string>('');
  let matchBad = $state(false);
  let applying = $state(false);
  let search = $state('');
  // The rule being edited stays visible even if it no longer matches the search.
  const visible = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return q ? rules.filter((r) => r.name.toLowerCase().includes(q) || r.id === draft?.id) : rules;
  });

  async function load() {
    rules = (await guarded(() => api.listRules())) ?? [];
  }
  load();

  const tagName = (id: number) => app.tags.find((t) => t.id === id);
  const plName = (id: number) => app.playlists.find((p) => p.id === id)?.name ?? '?';

  function startNew() {
    draft = { id: null, name: '', pattern: '', case_insensitive: true, enabled: true, tag_ids: [], playlist_ids: [] };
    matchInfo = '';
  }
  function startEdit(r: Rule) {
    draft = { ...r, tag_ids: [...r.tag_ids], playlist_ids: [...r.playlist_ids] };
  }

  // A rule named exactly like a tag/playlist selects it (the user can still deselect it).
  function autoSelectByName() {
    if (!draft) return;
    const name = draft.name.trim();
    if (!name) return;
    const tag = app.tags.find((x) => x.name === name);
    if (tag && !draft.tag_ids.includes(tag.id)) draft.tag_ids = [...draft.tag_ids, tag.id];
    const pl = app.playlists.find((p) => p.name === name);
    if (pl && !draft.playlist_ids.includes(pl.id)) draft.playlist_ids = [...draft.playlist_ids, pl.id];
  }

  function toggle(list: number[], id: number): number[] {
    return list.includes(id) ? list.filter((x) => x !== id) : [...list, id];
  }

  // Live preview: how many library videos this pattern matches (also reports invalid regexes).
  let timer: ReturnType<typeof setTimeout>;
  $effect(() => {
    const d = draft;
    if (!d) return;
    const [pattern, ci] = [d.pattern, d.case_insensitive];
    clearTimeout(timer);
    if (!pattern) { matchInfo = ''; matchBad = false; return; }
    timer = setTimeout(async () => {
      try {
        const n = await api.countPatternMatches(pattern, ci);
        matchInfo = t('rules.matches', { n });
        matchBad = false;
      } catch (e) {
        matchInfo = t('rules.invalid', { msg: typeof e === 'string' ? e : (e as Error).message });
        matchBad = true;
      }
    }, 250);
    return () => clearTimeout(timer);
  });

  async function save() {
    if (!draft) return;
    const { id, ...input } = draft;
    const ok = await guarded(async () => {
      if (id == null) await api.createRule(input);
      else await api.updateRule(id, input);
      return true;
    });
    if (ok) {
      draft = null;
      await load();
    }
  }

  let deleting = $state<Rule | null>(null);

  async function remove(id: number) {
    deleting = null;
    await guarded(() => api.deleteRule(id));
    await load();
  }

  async function setEnabled(r: Rule, enabled: boolean) {
    const { id, ...input } = r;
    await guarded(() => api.updateRule(id, { ...input, enabled }));
    await load();
  }

  async function applyToExisting() {
    applying = true;
    const n = await guarded(() => api.applyRulesToExisting());
    applying = false;
    if (n !== undefined) {
      toast(t('rules.applied', { n }));
      await refreshAll();
    }
  }
</script>

<div class="head">
  <div class="grow">
    <h1>{t('rules.title')}</h1>
    <div class="muted">{t('rules.intro')}</div>
  </div>
  <button onclick={applyToExisting} disabled={applying || rules.length === 0}
    title={t('rules.apply_title')}>
    <WandSparkles size={15} /> {applying ? t('rules.applying') : t('rules.apply_existing')}
  </button>
  <button class="primary" onclick={startNew}><Plus size={15} /> {t('rules.new')}</button>
</div>

{#snippet editor(d: RuleInput & { id: number | null })}
  <form class="card editor" class:invalid={!d.tag_ids.length && !d.playlist_ids.length} onsubmit={(e) => { e.preventDefault(); save(); }}>
    <div class="row">
      <input type="text" class="grow" placeholder={t('rules.name_placeholder')} bind:value={d.name} oninput={autoSelectByName} />
      <label class="check"><input type="checkbox" bind:checked={d.enabled} /> {t('common.enabled')}</label>
    </div>
    <div class="row">
      <input type="text" class="grow mono" placeholder={t('rules.pattern_placeholder')} bind:value={d.pattern} />
      <label class="check"><input type="checkbox" bind:checked={d.case_insensitive} /> {t('rules.ignore_case')}</label>
    </div>
    <div class="hint" class:bad={matchBad}>
      {matchInfo || t('rules.pattern_hint')}
    </div>
    <div class="muted small">{@html t('rules.paths_note')}</div>

    <h2>{t('rules.apply_tags')}</h2>
    <div class="chips">
      {#each app.tags as t (t.id)}
        <button type="button" class="chip" class:active={d.tag_ids.includes(t.id)}
          onclick={() => draft && (draft.tag_ids = toggle(draft.tag_ids, t.id))}>
          <span class="dot" style:background={t.color}></span>{t.name}
        </button>
      {:else}<span class="muted">{t('rules.create_tags_first')}</span>{/each}
    </div>

    <h2>{t('rules.add_to_playlists')}</h2>
    <div class="chips">
      {#each app.playlists as p (p.id)}
        <button type="button" class="chip" class:active={d.playlist_ids.includes(p.id)}
          onclick={() => draft && (draft.playlist_ids = toggle(draft.playlist_ids, p.id))}>{p.name}</button>
      {:else}<span class="muted">{t('rules.create_playlists_first')}</span>{/each}
    </div>

    <div class="row actions">
      <button type="submit" class="primary"><Check size={15} /> {t('rules.save')}</button>
      <button type="button" onclick={() => (draft = null)}><X size={15} /> {t('common.cancel')}</button>
    </div>
  </form>
{/snippet}

<input type="text" class="search" placeholder={t('rules.search')} bind:value={search} />

{#if draft && draft.id == null}{@render editor(draft)}{/if}

<div class="list">
  {#each visible as r (r.id)}
    {#if draft && draft.id === r.id}
      {@render editor(draft)}
    {:else}
    <div class="card rule" class:off={!r.enabled} class:invalid={!r.tag_ids.length && !r.playlist_ids.length}>
      <div class="row">
        <input type="checkbox" checked={r.enabled} onchange={(e) => setEnabled(r, e.currentTarget.checked)} title={t('common.enabled')} />
        <div class="grow">
          <div class="name">{r.name}</div>
          <code class="pattern">{r.pattern}</code>{#if !r.case_insensitive}<span class="muted small">{t('rules.case_sensitive')}</span>{/if}
        </div>
        <button class="ghost" onclick={() => startEdit(r)} title={t('common.edit')}><Pencil size={15} /></button>
        <button class="ghost danger" onclick={() => (deleting = r)} title={t('common.delete')}><Trash2 size={15} /></button>
      </div>
      <div class="chips applied">
        {#each r.tag_ids as id (id)}
          {@const t = tagName(id)}
          {#if t}<span class="chip"><span class="dot" style:background={t.color}></span>{t.name}</span>{/if}
        {/each}
        {#each r.playlist_ids as id (id)}
          <span class="chip"><ListVideo size={12} /> {plName(id)}</span>
        {/each}
        {#if !r.tag_ids.length && !r.playlist_ids.length}<span class="muted small">{t('rules.no_selection')}</span>{/if}
      </div>
    </div>
    {/if}
  {:else}
    {#if !draft}<div class="card empty">{@html t(rules.length ? 'rules.no_results' : 'rules.empty')}</div>{/if}
  {/each}
</div>

{#if deleting}
  <ConfirmDialog title={t('rules.delete_title')} onconfirm={() => remove(deleting!.id)} oncancel={() => (deleting = null)}>
    <p>{@html th('rules.delete_body', { name: deleting.name })}</p>
  </ConfirmDialog>
{/if}

<style>
  .search { width: 100%; max-width: 360px; margin-bottom: 14px; }
  .head { display: flex; align-items: flex-start; gap: 10px; margin-bottom: 18px; }
  .editor { margin-bottom: 18px; display: flex; flex-direction: column; gap: 10px; }
  .mono, code, .small :global(code) { font-family: ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace; }
  code, .small :global(code) { background: var(--surface-2); padding: 1px 6px; border-radius: 4px; user-select: text; }
  .check { display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; }
  .hint { font-size: 13px; color: var(--primary); min-height: 18px; }
  .hint.bad { color: var(--danger); }
  .small { font-size: 12px; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; }
  .actions { margin-top: 6px; }
  .list { display: flex; flex-direction: column; gap: 10px; }
  .invalid { border-color: var(--danger); }
  .rule.off { opacity: .55; }
  .name { font-weight: 600; margin-bottom: 3px; }
  .applied { margin-top: 10px; padding-left: 28px; }
  .empty { text-align: center; padding: 36px; color: var(--muted); }
  input[type='checkbox'] { width: 18px; height: 18px; accent-color: var(--primary); }
  h2 { margin: 6px 0 0; }
</style>
