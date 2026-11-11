<script lang="ts">
  import { app, currentVideo, stopPlayback, togglePause } from '../lib/store.svelte';
  import { fmtTime } from '../lib/api';
  const video = $derived(currentVideo());
</script>

<div class="mini">
  <button class="primary" onclick={togglePause}>{app.player.paused ? '▶' : '❚❚'}</button>
  <div class="grow title">{video?.title ?? 'Playing…'}</div>
  <span class="muted">{fmtTime(app.player.position)} / {fmtTime(app.player.duration)}</span>
  <button onclick={() => (app.playerView = true)}>Open player</button>
  <button class="danger" onclick={stopPlayback}>■</button>
</div>

<style>
  .mini { display: flex; align-items: center; gap: 12px; padding: 10px 18px; background: var(--surface); border-top: 1px solid var(--primary); }
  .title { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
