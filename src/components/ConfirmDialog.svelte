<script lang="ts">
  import type { Snippet } from 'svelte';
  import Trash2 from '@lucide/svelte/icons/trash-2';

  let { title, confirmLabel = 'Delete', onconfirm, oncancel, children }: {
    title: string; confirmLabel?: string; onconfirm: () => void; oncancel: () => void; children: Snippet;
  } = $props();
</script>

<svelte:window onkeydown={(e) => e.key === 'Escape' && oncancel()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={oncancel}>
  <div class="dialog card" role="alertdialog" aria-modal="true" tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <h1>{title}</h1>
    <div class="body">{@render children()}</div>
    <div class="row buttons">
      <span class="grow"></span>
      <button onclick={oncancel}>Cancel</button>
      <button class="solid" onclick={onconfirm}><Trash2 size={15} /> {confirmLabel}</button>
    </div>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: #000a; display: grid; place-items: center; z-index: 60; }
  .dialog { width: min(480px, 92vw); }
  .body { line-height: 1.5; word-break: break-word; user-select: text; }
  .buttons { margin-top: 16px; }
  .solid { background: var(--danger); border-color: var(--danger); color: #fff; font-weight: 600; }
  .solid:hover { background: var(--danger); filter: brightness(1.15); }
</style>
