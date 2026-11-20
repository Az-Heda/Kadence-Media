<script lang="ts">
  import { api, errMsg, type Video } from '../lib/api';
  import { untrack } from 'svelte';
  import { t } from '../lib/i18n.svelte';
  import Pencil from '@lucide/svelte/icons/pencil';

  let { video, ondone, onclose }: { video: Video; ondone: (v: Video) => void; onclose: () => void } = $props();

  // The extension stays: the user edits the part before it.
  const v0 = untrack(() => video); // the dialog works on the video it was opened for
  const dot = v0.path.lastIndexOf('.');
  const slash = Math.max(v0.path.lastIndexOf('/'), v0.path.lastIndexOf('\\'));
  const ext = dot > slash ? v0.path.slice(dot) : '';
  let name = $state(v0.title);
  let error = $state('');
  let busy = $state(false);

  async function submit() {
    if (busy) return;
    busy = true;
    error = '';
    try {
      ondone(await api.renameVideo(v0.id, name));
    } catch (e) {
      error = errMsg(e); // shown here: a toast would sit behind the dialog
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && onclose()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <form class="dialog card" aria-label={t('library.rename_title')} onclick={(e) => e.stopPropagation()}
    onsubmit={(e) => { e.preventDefault(); submit(); }}>
    <h1>{t('library.rename_title')}</h1>
    <label class="label" for="rename-name">{t('library.rename_label')}</label>
    <div class="row">
      <!-- svelte-ignore a11y_autofocus -->
      <input id="rename-name" type="text" class="grow" bind:value={name} autofocus onfocus={(e) => e.currentTarget.select()} />
      <span class="muted">{ext}</span>
    </div>
    <div class="muted hint">{t('library.rename_hint', { ext: ext || '—' })}</div>
    {#if error}<div class="error">{error}</div>{/if}
    <div class="row buttons">
      <span class="grow"></span>
      <button type="button" onclick={onclose}>{t('common.cancel')}</button>
      <button type="submit" class="primary" disabled={busy || !name.trim()}><Pencil size={15} /> {t('common.rename')}</button>
    </div>
  </form>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: #000a; display: grid; place-items: center; z-index: 60; }
  .dialog { width: min(520px, 92vw); }
  .label { display: block; font-weight: 600; margin: 12px 0 6px; }
  .hint { font-size: 12px; margin-top: 6px; }
  .error { color: var(--danger); margin-top: 10px; word-break: break-word; user-select: text; }
  .buttons { margin-top: 16px; }
</style>
