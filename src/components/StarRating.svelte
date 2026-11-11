<script lang="ts">
  import Star from '@lucide/svelte/icons/star';

  let { value, onchange, size = 16, readonly = false }: {
    value: number; onchange?: (v: number) => void; size?: number; readonly?: boolean;
  } = $props();

  let hover = $state<number | null>(null);
  const shown = $derived(hover ?? value);
  const fill = (i: number) => Math.max(0, Math.min(1, shown - i)) * 100;

  // Left half of a star = x.5, right half = x+1.
  function valueAt(e: MouseEvent, i: number): number {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    return i + (e.clientX - r.left < r.width / 2 ? 0.5 : 1);
  }
</script>

<div class="stars" role="group" aria-label="Rating" onmouseleave={() => (hover = null)}
  title={value ? `${value} / 5` : 'Not rated'}>
  {#each [0, 1, 2, 3, 4] as i}
    <button type="button" class="star" disabled={readonly} style:width="{size}px" style:height="{size}px"
      aria-label="Rate {i + 1}"
      onmousemove={(e) => !readonly && (hover = valueAt(e, i))}
      onclick={(e) => { const v = valueAt(e, i); onchange?.(v === value ? 0 : v); }}>
      <Star {size} class="outline" />
      <span class="fill" style:width="{fill(i)}%"><Star {size} fill="currentColor" /></span>
    </button>
  {/each}
</div>

<style>
  .stars { display: inline-flex; align-items: center; }
  .star { all: unset; position: relative; display: inline-block; cursor: pointer; color: var(--primary); }
  .star:disabled { cursor: default; }
  .star :global(.outline) { color: var(--border); position: absolute; inset: 0; }
  .star:hover :global(.outline) { color: var(--muted); }
  .fill { position: absolute; left: 0; top: 0; bottom: 0; overflow: hidden; pointer-events: none; }
</style>
