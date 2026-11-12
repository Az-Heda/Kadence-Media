<script lang="ts">
  import { fmtTime, type Video } from '../lib/api';
  import Play from '@lucide/svelte/icons/play';

  let { video, onplay, size = 34 }: { video: Video; onplay?: () => void; size?: number } = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="thumb" class:clickable={!!onplay} onclick={onplay} title={onplay ? 'Play' : undefined}>
  <span class="play"><Play {size} fill="currentColor" /></span>
  {#if video.watched}<span class="badge">Watched</span>{/if}
  {#if video.duration && video.duration > 0}<span class="dur">{fmtTime(video.duration)}</span>{/if}
</div>

<style>
  .thumb {
    position: relative; aspect-ratio: 16 / 9; display: grid; place-items: center;
    background: radial-gradient(circle at 30% 20%, #2a2412, #131316 70%);
  }
  .clickable { cursor: pointer; }
  .play { display: inline-flex; color: var(--primary); opacity: .75; transition: transform .12s, opacity .12s; }
  .clickable:hover .play { transform: scale(1.2); opacity: 1; }
  .badge { position: absolute; top: 8px; right: 8px; font-size: 11px; padding: 2px 8px; border-radius: 999px; background: var(--primary-soft); color: var(--primary); border: 1px solid var(--primary); }
  .dur { position: absolute; bottom: 8px; right: 8px; font-size: 11px; padding: 1px 7px; border-radius: 6px; background: #000b; font-variant-numeric: tabular-nums; }
</style>
