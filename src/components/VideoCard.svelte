<script lang="ts">
  import { fmtSize, type Video } from '../lib/api';
  import { toggleWatched } from '../lib/store.svelte';
  import Play from '@lucide/svelte/icons/play';
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';
  import Tag from '@lucide/svelte/icons/tag';

  let { video, onplay, onmanage }: { video: Video; onplay: () => void; onmanage: () => void } = $props();
</script>

<div class="card video" class:watched={video.watched}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="thumb" onclick={onplay} title="Play">
    <span class="play"><Play size={34} fill="currentColor" /></span>
    {#if video.watched}<span class="badge">Watched</span>{/if}
  </div>
  <div class="info">
    <div class="title" title={video.path}>{video.title}</div>
    <div class="muted small">{fmtSize(video.size)} · {video.path.split(/[\\/]/).slice(-2, -1)[0] ?? ''}</div>
    {#if video.tags.length}
      <div class="tags">
        {#each video.tags as t (t.id)}
          <span class="chip"><span class="dot" style:background={t.color}></span>{t.name}</span>
        {/each}
      </div>
    {/if}
  </div>
  <div class="actions">
    <button class="ghost" onclick={() => toggleWatched(video)} title="Toggle watched">
      {#if video.watched}<Eye size={14} /> Watched{:else}<EyeOff size={14} /> Unwatched{/if}
    </button>
    <button class="ghost" onclick={onmanage} title="Tags & playlists"><Tag size={14} /> Organize</button>
  </div>
</div>

<style>
  .video { padding: 0; overflow: hidden; display: flex; flex-direction: column; transition: border-color .12s, transform .12s; }
  .video:hover { border-color: var(--primary); }
  .thumb {
    position: relative; aspect-ratio: 16 / 9; display: grid; place-items: center; cursor: pointer;
    background: radial-gradient(circle at 30% 20%, #2a2412, #131316 70%);
  }
  .play { display: inline-flex; color: var(--primary); opacity: .75; transition: transform .12s, opacity .12s; }
  .thumb:hover .play { transform: scale(1.2); opacity: 1; }
  .badge { position: absolute; top: 8px; right: 8px; font-size: 11px; padding: 2px 8px; border-radius: 999px; background: var(--primary-soft); color: var(--primary); border: 1px solid var(--primary); }
  .info { padding: 10px 12px 4px; flex: 1; }
  .title { font-weight: 600; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; word-break: break-word; }
  .small { font-size: 12px; margin-top: 3px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .tags { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 8px; }
  .actions { display: flex; justify-content: space-between; padding: 6px 8px 8px; }
  .actions button { font-size: 12px; padding: 4px 8px; }
  .watched .title { opacity: .7; }
</style>
