<script lang="ts" generics="T extends string | number">
  import CaretDown from 'phosphor-svelte/lib/CaretDown';
  import Check from 'phosphor-svelte/lib/Check';
  import { tick } from 'svelte';

  /**
   * A dropdown drawn by the app instead of the native one, so it follows the theme. The list floats with fixed
   * positioning (scroll containers cannot clip it), opens upward when there is no room below, and supports the usual
   * keys: arrows, Home/End, Enter/Space, Escape, Tab and typing the first letter.
   */
  interface Props {
    value: T;
    options: { value: T; label: string }[];
    onchange: (value: T) => void;
    label?: string;
    width?: string;
  }

  let { value, options, onchange, label, width }: Props = $props();

  const id = `select-${Math.random().toString(36).slice(2, 9)}`;
  let open = $state(false);
  let active = $state(0);
  let trigger: HTMLButtonElement | undefined = $state();
  let list: HTMLDivElement | undefined = $state();
  let place = $state({ left: 0, top: 0, width: 0, up: false });

  const current = $derived(options.find((o) => o.value === value));

  async function show() {
    if (!trigger) return;
    const rect = trigger.getBoundingClientRect();
    const height = Math.min(options.length * 38 + 12, 320);
    const up = rect.bottom + height + 8 > window.innerHeight && rect.top - height - 8 > 0;
    place = { left: rect.left, top: up ? rect.top - 6 : rect.bottom + 6, width: rect.width, up };
    active = Math.max(0, options.findIndex((o) => o.value === value));
    open = true;
    await tick();
    list?.querySelectorAll<HTMLElement>('[role=option]')[active]?.scrollIntoView({ block: 'nearest' });
  }

  function hide(focus = true) {
    open = false;
    if (focus) trigger?.focus();
  }

  function choose(index: number) {
    const option = options[index];
    hide();
    if (option && option.value !== value) onchange(option.value);
  }

  function move(to: number) {
    active = (to + options.length) % options.length;
    list?.querySelectorAll<HTMLElement>('[role=option]')[active]?.scrollIntoView({ block: 'nearest' });
  }

  function keydown(event: KeyboardEvent) {
    if (!open) {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) {
        event.preventDefault();
        void show();
      }
      return;
    }
    if (event.key === 'ArrowDown') move(active + 1);
    else if (event.key === 'ArrowUp') move(active - 1);
    else if (event.key === 'Home') move(0);
    else if (event.key === 'End') move(options.length - 1);
    else if (event.key === 'Enter' || event.key === ' ') choose(active);
    else if (event.key === 'Escape') hide();
    else if (event.key === 'Tab') { hide(false); return; }
    else if (event.key.length === 1) {
      const letter = event.key.toLowerCase();
      const next = options.findIndex((o, i) => i > active && o.label.toLowerCase().startsWith(letter));
      const first = options.findIndex((o) => o.label.toLowerCase().startsWith(letter));
      if (next >= 0 || first >= 0) move(next >= 0 ? next : first);
      else return;
    } else return;
    event.preventDefault();
  }

  function outside(event: PointerEvent) {
    const target = event.target as Node;
    if (open && !trigger?.contains(target) && !list?.contains(target)) hide(false);
  }
</script>

<svelte:window onpointerdown={outside} onresize={() => open && hide(false)} onblur={() => open && hide(false)} />
<!-- Scroll events do not bubble: capture them from any scrolling container except the list itself. -->
<svelte:document onscrollcapture={(event) => open && !list?.contains(event.target as Node) && hide(false)} />

<button
  bind:this={trigger}
  type="button"
  class="trigger"
  class:open
  style:width
  role="combobox"
  aria-haspopup="listbox"
  aria-expanded={open}
  aria-controls={id}
  aria-activedescendant={open ? `${id}-${active}` : undefined}
  aria-label={label}
  onclick={() => (open ? hide() : show())}
  onkeydown={keydown}
>
  <span class="value">{current?.label ?? ''}</span>
  <span class="caret"><CaretDown size={13} weight="bold" /></span>
</button>

{#if open}
  <div
    bind:this={list}
    {id}
    class="list"
    class:up={place.up}
    role="listbox"
    aria-label={label}
    tabindex="-1"
    style:left="{place.left}px"
    style:top="{place.top}px"
    style:min-width="{place.width}px"
  >
    {#each options as option, index (option.value)}
      <div
        id="{id}-{index}"
        class="option"
        class:active={index === active}
        class:selected={option.value === value}
        role="option"
        aria-selected={option.value === value}
        tabindex="-1"
        onpointerenter={() => (active = index)}
        onclick={() => choose(index)}
        onkeydown={() => {}}
      >
        <span class="grow">{option.label}</span>
        {#if option.value === value}<span class="tick"><Check size={14} weight="bold" /></span>{/if}
      </div>
    {/each}
  </div>
{/if}

<style>
  .trigger { display: flex; align-items: center; gap: 8px; width: 100%; min-height: 36px; padding: 8px 12px; border-radius: 12px; background: var(--well); color: var(--ink); text-align: left; transition: box-shadow 0.15s, background 0.15s; }
  .trigger:hover { background: var(--hover); }
  .trigger:focus-visible, .trigger.open { outline: none; background: var(--surface); box-shadow: 0 0 0 1.5px var(--focus); }
  .value { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .caret { display: grid; color: var(--muted); transition: transform 0.18s; }
  .open .caret { transform: rotate(180deg); }
  .list { position: fixed; z-index: 60; max-height: 320px; overflow-y: auto; padding: 6px; display: flex; flex-direction: column; gap: 2px; border-radius: 14px; background: var(--glass-strong); box-shadow: var(--float); backdrop-filter: blur(20px); animation: drop 0.14s ease-out; }
  .list.up { transform: translateY(-100%); animation-name: lift; }
  .option { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border-radius: 9px; cursor: default; white-space: nowrap; }
  .option.active { background: var(--hover); }
  .option.selected { color: var(--info); font-weight: 600; }
  .tick { display: grid; color: var(--info); }
  .grow { flex: 1; }
  @keyframes drop { from { opacity: 0; transform: translateY(-4px); } }
  @keyframes lift { from { opacity: 0; transform: translateY(calc(-100% + 4px)); } }
</style>
