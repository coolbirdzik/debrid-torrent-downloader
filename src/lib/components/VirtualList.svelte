<script lang="ts" generics="T">
  import type { Snippet } from 'svelte';

  interface Props {
    items: T[];
    rowHeight: number;
    /** Grid mode: fixed tile width; rows hold as many tiles as fit. */
    tileWidth?: number;
    gap?: number;
    key: (item: T) => string;
    row: Snippet<[T, number]>;
    onkeydown?: (event: KeyboardEvent) => void;
    label?: string;
    role?: 'listbox' | 'list';
  }

  let { items, rowHeight, tileWidth, gap = 8, key, row, onkeydown, label, role = 'listbox' }: Props = $props();

  let viewport: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let height = $state(400);
  let width = $state(800);

  const perRow = $derived(tileWidth ? Math.max(1, Math.floor((width - 16 + gap) / (tileWidth + gap))) : 1);
  const rows = $derived(Math.ceil(items.length / perRow));
  const stride = $derived(rowHeight + (tileWidth ? gap : 0));
  const first = $derived(Math.max(0, Math.floor(scrollTop / stride) - 4));
  const last = $derived(Math.min(rows, Math.ceil((scrollTop + height) / stride) + 4));
  const visible = $derived.by(() => {
    const out: { index: number; item: T }[] = [];
    for (let r = first; r < last; r++) for (let c = 0; c < perRow; c++) {
      const index = r * perRow + c;
      if (index < items.length) out.push({ index, item: items[index] });
    }
    return out;
  });

  let frame = 0;
  function onscroll() {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      scrollTop = viewport?.scrollTop ?? 0;
    });
  }

  /** Scroll so a row is visible (keyboard navigation). */
  export function columns() { return perRow; }

  export function reveal(index: number) {
    if (!viewport) return;
    const top = Math.floor(index / perRow) * stride;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + rowHeight > viewport.scrollTop + height) viewport.scrollTop = top + rowHeight - height;
  }
</script>

<!-- A scrollable read-only list is focusable so keyboard users can review offscreen items. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="viewport" bind:this={viewport} bind:clientHeight={height} bind:clientWidth={width} {onscroll} {onkeydown} {role} aria-multiselectable={role === 'listbox' ? true : undefined} aria-label={label} tabindex="0">
  <div class="space" style:height="{rows * stride}px">
    {#each visible as { index, item } (key(item))}
      <div
        class="cell"
        style:top="{Math.floor(index / perRow) * stride}px"
        style:height="{rowHeight}px"
        style:left={tileWidth ? `${8 + (index % perRow) * (tileWidth + gap)}px` : '0'}
        style:width={tileWidth ? `${tileWidth}px` : '100%'}
      >
        {@render row(item, index)}
      </div>
    {/each}
  </div>
</div>

<style>
  .viewport { position: relative; overflow-y: auto; height: 100%; outline: none; }
  .space { position: relative; width: 100%; }
  .cell { position: absolute; }
</style>
