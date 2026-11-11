<script lang="ts">
  import { app, currentVideo, stopPlayback, togglePause } from '../lib/store.svelte';
  import { fmtTime } from '../lib/api';
  import Play from '@lucide/svelte/icons/play';
  import Pause from '@lucide/svelte/icons/pause';
  import Square from '@lucide/svelte/icons/square';
  import Maximize2 from '@lucide/svelte/icons/maximize-2';
  const video = $derived(currentVideo());
</script>

<div class="mini">
  <button class="primary" onclick={togglePause}>{#if app.player.paused}<Play size={16} />{:else}<Pause size={16} />{/if}</button>
  <div class="grow title">{video?.title ?? 'Playing…'}</div>
  <span class="muted">{fmtTime(app.player.position)} / {fmtTime(app.player.duration)}</span>
  <button onclick={() => (app.playerView = true)}><Maximize2 size={15} /> Open player</button>
  <button class="danger" onclick={stopPlayback} title="Stop"><Square size={15} /></button>
</div>

<style>
  .mini { display: flex; align-items: center; gap: 12px; padding: 10px 18px; background: var(--surface); border-top: 1px solid var(--primary); }
  .title { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
