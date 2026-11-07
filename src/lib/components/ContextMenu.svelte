<script lang="ts" module>
  // Every Phosphor icon has this component type.
  import type Icon from 'phosphor-svelte/lib/Circle';

  export type MenuItem =
    | {
        label: string;
        icon: typeof Icon;
        /** Icon tint, one of the `.ic` colors. */
        tone?: 'blue' | 'red' | 'green' | 'amber' | 'purple' | 'teal' | 'orange';
        weight?: 'regular' | 'bold' | 'fill' | 'duotone';
        disabled?: boolean;
        danger?: boolean;
        action: () => unknown;
      }
    | 'separator';
</script>

<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte';
  import { t } from '../i18n.svelte';

  /**
   * A right-click menu at the pointer. It stays inside the window, closes on Escape, an outside click, scrolling,
   * resizing or losing focus, and supports arrow keys, Home/End and Enter.
   */
  interface Props { x: number; y: number; items: MenuItem[]; onclose: () => void }
  let { x, y, items, onclose }: Props = $props();

  let box: HTMLDivElement | undefined = $state();
  let left = $state(0);
  let top = $state(0);

  // Trailing, leading and doubled separators are dropped, so callers can build lists with conditionals.
  const shown = $derived(items.filter((item, i, all) => item !== 'separator' || (i > 0 && i < all.length - 1 && all[i - 1] !== 'separator')));

  // Give focus back to the list the menu was opened from, unless an action moved it on (a dialog focuses itself).
  let previous: HTMLElement | null = null;
  onDestroy(() => queueMicrotask(() => {
    if ((document.activeElement === document.body || document.activeElement === null) && previous?.isConnected) previous.focus({ preventScroll: true });
  }));

  onMount(async () => {
    previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    left = x;
    top = y;
    await tick();
    if (!box) return;
    const rect = box.getBoundingClientRect();
    left = Math.max(8, Math.min(x, window.innerWidth - rect.width - 8));
    top = Math.max(8, Math.min(y, window.innerHeight - rect.height - 8));
    buttons()[0]?.focus();
  });

  const buttons = () => [...(box?.querySelectorAll<HTMLButtonElement>('button:not(:disabled)') ?? [])];

  function run(item: Exclude<MenuItem, 'separator'>) {
    if (item.disabled) return;
    onclose();
    void item.action();
  }

  function keydown(event: KeyboardEvent) {
    const list = buttons();
    const index = list.indexOf(document.activeElement as HTMLButtonElement);
    const move = (next: number) => { event.preventDefault(); list[(next + list.length) % list.length]?.focus(); };
    if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); onclose(); }
    else if (event.key === 'ArrowDown') move(index + 1);
    else if (event.key === 'ArrowUp') move(index - 1);
    else if (event.key === 'Home') move(0);
    else if (event.key === 'End') move(list.length - 1);
    else if (event.key === 'Tab') onclose();
  }

  function outside(event: PointerEvent) {
    if (box && !box.contains(event.target as Node)) onclose();
  }
</script>

<svelte:window onpointerdown={outside} onwheel={onclose} onblur={onclose} onresize={onclose} />

<div class="ctx card menu-glass" bind:this={box} role="menu" tabindex="-1" aria-label={t('Menu chuột phải')} style:left="{left}px" style:top="{top}px" oncontextmenu={(e) => e.preventDefault()} onkeydown={keydown}>
  {#each shown as item, i (i)}
    {#if item === 'separator'}
      <span class="sep" role="separator"></span>
    {:else}
      <button class="menu-item" class:danger={item.danger} role="menuitem" disabled={item.disabled} onclick={() => run(item)}>
        <span class="ic {item.tone ?? 'blue'}"><item.icon size={16} weight={item.weight ?? 'duotone'} /></span>
        <span class="grow">{item.label}</span>
      </button>
    {/if}
  {/each}
</div>

<style>
  .ctx { position: fixed; z-index: 65; min-width: 220px; padding: 6px; display: flex; flex-direction: column; gap: 2px; border-radius: 18px; outline: none; }
  .menu-item { display: flex; align-items: center; gap: 10px; padding: 9px 12px; border-radius: 12px; font-size: 12px; text-align: left; transition: background 0.15s; }
  .menu-item:hover, .menu-item:focus-visible { background: var(--hover); outline: none; }
  .menu-item:disabled { opacity: 0.45; pointer-events: none; }
  .menu-item.danger { color: var(--danger); }
  .grow { flex: 1; }
  .sep { height: 1px; margin: 4px 10px; background: var(--stroke); }
</style>
