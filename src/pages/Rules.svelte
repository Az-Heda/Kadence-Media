<script lang="ts">
  import { api, type Rule, type RuleInput } from '../lib/api';
  import { app, guarded, refreshAll, toast } from '../lib/store.svelte';
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
  let applying = $state(false);

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
    if (!pattern) { matchInfo = ''; return; }
    timer = setTimeout(async () => {
      try {
        const n = await api.countPatternMatches(pattern, ci);
        matchInfo = `Matches ${n} video${n === 1 ? '' : 's'} currently in the library.`;
      } catch (e) {
        matchInfo = `Invalid pattern: ${typeof e === 'string' ? e : (e as Error).message}`;
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

  async function remove(id: number) {
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
      toast(`Rules applied: ${n} video${n === 1 ? '' : 's'} matched`);
      await refreshAll();
    }
  }
</script>

<div class="head">
  <div class="grow">
    <h1>Automatic Rules</h1>
    <div class="muted">
      When a new video is found while scanning, its full path is tested against each enabled rule. For every rule
      that matches, all of its tags and playlists are applied.
    </div>
  </div>
  <button onclick={applyToExisting} disabled={applying || rules.length === 0}
    title="Applies the rules to videos that are already in the library (adds tags/playlists, never removes)">
    <WandSparkles size={15} /> {applying ? 'Applying…' : 'Apply to existing videos'}
  </button>
  <button class="primary" onclick={startNew}><Plus size={15} /> New rule</button>
</div>

{#if draft}
  <form class="card editor" onsubmit={(e) => { e.preventDefault(); save(); }}>
    <div class="row">
      <input type="text" class="grow" placeholder="Rule name (e.g. Comedy shows)" bind:value={draft.name} />
      <label class="check"><input type="checkbox" bind:checked={draft.enabled} /> Enabled</label>
    </div>
    <div class="row">
      <input type="text" class="grow mono" placeholder={String.raw`Regex on the full path, e.g. [\\/]Comedy[\\/]`} bind:value={draft.pattern} />
      <label class="check"><input type="checkbox" bind:checked={draft.case_insensitive} /> Ignore case</label>
    </div>
    <div class="hint" class:bad={matchInfo.startsWith('Invalid')}>
      {matchInfo || 'The pattern is matched anywhere in the full path.'}
    </div>
    <div class="muted small">
      Windows paths use <code>\</code> and Linux paths use <code>/</code>: use <code>[\\/]</code> to match both.
      Standard regex syntax (no lookaround or backreferences).
    </div>

    <h2>Apply tags</h2>
    <div class="chips">
      {#each app.tags as t (t.id)}
        <button type="button" class="chip" class:active={draft.tag_ids.includes(t.id)}
          onclick={() => draft && (draft.tag_ids = toggle(draft.tag_ids, t.id))}>
          <span class="dot" style:background={t.color}></span>{t.name}
        </button>
      {:else}<span class="muted">Create tags in “Tags & Playlists” first.</span>{/each}
    </div>

    <h2>Add to playlists</h2>
    <div class="chips">
      {#each app.playlists as p (p.id)}
        <button type="button" class="chip" class:active={draft.playlist_ids.includes(p.id)}
          onclick={() => draft && (draft.playlist_ids = toggle(draft.playlist_ids, p.id))}>{p.name}</button>
      {:else}<span class="muted">Create playlists in “Tags & Playlists” first.</span>{/each}
    </div>

    <div class="row actions">
      <button type="submit" class="primary"><Check size={15} /> Save rule</button>
      <button type="button" onclick={() => (draft = null)}><X size={15} /> Cancel</button>
    </div>
  </form>
{/if}

<div class="list">
  {#each rules as r (r.id)}
    <div class="card rule" class:off={!r.enabled}>
      <div class="row">
        <input type="checkbox" checked={r.enabled} onchange={(e) => setEnabled(r, e.currentTarget.checked)} title="Enabled" />
        <div class="grow">
          <div class="name">{r.name}</div>
          <code class="pattern">{r.pattern}</code>{#if !r.case_insensitive}<span class="muted small"> · case-sensitive</span>{/if}
        </div>
        <button class="ghost" onclick={() => startEdit(r)} title="Edit"><Pencil size={15} /></button>
        <button class="ghost danger" onclick={() => remove(r.id)} title="Delete"><Trash2 size={15} /></button>
      </div>
      <div class="chips applied">
        {#each r.tag_ids as id (id)}
          {@const t = tagName(id)}
          {#if t}<span class="chip"><span class="dot" style:background={t.color}></span>{t.name}</span>{/if}
        {/each}
        {#each r.playlist_ids as id (id)}
          <span class="chip"><ListVideo size={12} /> {plName(id)}</span>
        {/each}
        {#if !r.tag_ids.length && !r.playlist_ids.length}<span class="muted small">No tags or playlists selected.</span>{/if}
      </div>
    </div>
  {:else}
    {#if !draft}<div class="card empty">No rules yet. Click <b>New rule</b> to create one.</div>{/if}
  {/each}
</div>

<style>
  .head { display: flex; align-items: flex-start; gap: 10px; margin-bottom: 18px; }
  .editor { margin-bottom: 18px; display: flex; flex-direction: column; gap: 10px; }
  .mono, code { font-family: ui-monospace, 'Cascadia Code', Menlo, Consolas, monospace; }
  code { background: var(--surface-2); padding: 1px 6px; border-radius: 4px; user-select: text; }
  .check { display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; }
  .hint { font-size: 13px; color: var(--primary); min-height: 18px; }
  .hint.bad { color: var(--danger); }
  .small { font-size: 12px; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; }
  .actions { margin-top: 6px; }
  .list { display: flex; flex-direction: column; gap: 10px; }
  .rule.off { opacity: .55; }
  .name { font-weight: 600; margin-bottom: 3px; }
  .applied { margin-top: 10px; padding-left: 28px; }
  .empty { text-align: center; padding: 36px; color: var(--muted); }
  input[type='checkbox'] { width: 18px; height: 18px; accent-color: var(--primary); }
  h2 { margin: 6px 0 0; }
</style>
