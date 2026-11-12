<script lang="ts">
  import { fmtSize, type Video } from '../lib/api';
  import { setRating, toggleWatched } from '../lib/store.svelte';
  import Thumb from './Thumb.svelte';
  import StarRating from './StarRating.svelte';
  import { dragHandlers, dropMark, type PlaylistCtx } from '../lib/dnd';
  import ListX from '@lucide/svelte/icons/list-x';
  import Eye from '@lucide/svelte/icons/eye';
  import EyeOff from '@lucide/svelte/icons/eye-off';
  import Tag from '@lucide/svelte/icons/tag';

  let { video, onplay, onmanage, playlist = null }: {
    video: Video; onplay: () => void; onmanage: () => void; playlist?: PlaylistCtx | null;
  } = $props();
  const dnd = $derived(playlist?.canReorder ? dragHandlers(video.id, 'x', playlist.onmove) : {});
</script>

<div class="card video {dropMark(video.id)}" class:watched={video.watched} class:reorderable={playlist?.canReorder}
  draggable={playlist?.canReorder ? 'true' : undefined} {...dnd}>
  <Thumb {video} {onplay} />
  <div class="info">
    <div class="title" title={video.path}>{video.title}</div>
    <div class="muted small">{fmtSize(video.size)} · {video.path.split(/[\\/]/).slice(-2, -1)[0] ?? ''}</div>
    <div class="rating"><StarRating value={video.rating} size={15} onchange={(r) => setRating(video, r)} /></div>
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
    {#if playlist}
      <button class="ghost danger" onclick={() => playlist.onremove(video.id)} title="Remove from this playlist"><ListX size={14} /></button>
    {/if}
    <button class="ghost" onclick={onmanage} title="Tags & playlists"><Tag size={14} /> Organize</button>
  </div>
</div>

<style>
  .video { padding: 0; overflow: hidden; display: flex; flex-direction: column; transition: border-color .12s, transform .12s; }
  .reorderable { cursor: grab; }
  .video.drop-before { box-shadow: -4px 0 0 var(--primary); }
  .video.drop-after { box-shadow: 4px 0 0 var(--primary); }
  .video:hover { border-color: var(--primary); }
  .info { padding: 10px 12px 4px; flex: 1; }
  .title { font-weight: 600; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; word-break: break-word; }
  .small { font-size: 12px; margin-top: 3px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .rating { margin-top: 7px; }
  .tags { display: flex; flex-wrap: wrap; gap: 5px; margin-top: 8px; }
  .actions { display: flex; justify-content: space-between; padding: 6px 8px 8px; }
  .actions button { font-size: 12px; padding: 4px 8px; }
  .watched .title { opacity: .7; }
</style>
