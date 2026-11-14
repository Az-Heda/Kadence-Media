<script lang="ts">
  import { api, type Video } from '../lib/api';
  import { t } from '../lib/i18n.svelte';
  import X from '@lucide/svelte/icons/x';
  import { app, guarded, refreshAll } from '../lib/store.svelte';

  let { video, onclose }: { video: Video; onclose: () => void } = $props();
  let newTag = $state('');
  let newPlaylist = $state('');

  const hasTag = (id: number) => video.tags.some((t) => t.id === id);
  const inPlaylist = (id: number) => video.playlist_ids.includes(id);

  async function toggleTag(id: number) {
    await guarded(() => api.setVideoTag(video.id, id, !hasTag(id)));
    await refreshAll();
  }
  async function togglePlaylist(id: number) {
    await guarded(() => (inPlaylist(id) ? api.removeFromPlaylist(id, video.id) : api.addToPlaylist(id, video.id)));
    await refreshAll();
  }
  async function createTag() {
    if (!newTag.trim()) return;
    const t = await guarded(() => api.createTag(newTag, app.settings.primary_color));
    newTag = '';
    if (t) await guarded(() => api.setVideoTag(video.id, t.id, true));
    await refreshAll();
  }
  async function createPlaylist() {
    if (!newPlaylist.trim()) return;
    const p = await guarded(() => api.createPlaylist(newPlaylist));
    newPlaylist = '';
    if (p) await guarded(() => api.addToPlaylist(p.id, video.id));
    await refreshAll();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onclose}>
  <div class="dialog card" onclick={(e) => e.stopPropagation()}>
    <div class="row"><h1 class="grow">{video.title}</h1><button class="ghost" onclick={onclose}><X size={16} /></button></div>

    <h2>{t('common.tags')}</h2>
    <div class="chips">
      {#each app.tags as t (t.id)}
        <button class="chip" class:active={hasTag(t.id)} onclick={() => toggleTag(t.id)}>
          <span class="dot" style:background={t.color}></span>{t.name}
        </button>
      {:else}<span class="muted">{t('common.no_tags')}</span>{/each}
    </div>
    <form class="row add" onsubmit={(e) => { e.preventDefault(); createTag(); }}>
      <input type="text" class="grow" placeholder={t('common.new_tag')} bind:value={newTag} />
      <button class="primary" type="submit">{t('common.add')}</button>
    </form>

    <h2>{t('common.playlists')}</h2>
    <div class="chips">
      {#each app.playlists as p (p.id)}
        <button class="chip" class:active={inPlaylist(p.id)} onclick={() => togglePlaylist(p.id)}>{p.name}</button>
      {:else}<span class="muted">{t('common.no_playlists')}</span>{/each}
    </div>
    <form class="row add" onsubmit={(e) => { e.preventDefault(); createPlaylist(); }}>
      <input type="text" class="grow" placeholder={t('common.new_playlist')} bind:value={newPlaylist} />
      <button class="primary" type="submit">{t('common.add')}</button>
    </form>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: #000a; display: grid; place-items: center; z-index: 40; }
  .dialog { width: min(520px, 92vw); max-height: 85vh; overflow-y: auto; }
  .chips { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 10px; }
  .add { margin-bottom: 18px; }
  h1 { font-size: 17px; word-break: break-word; }
</style>
