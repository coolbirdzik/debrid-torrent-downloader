/** Shared selection for ordered lists and virtual grids. Keys must include the item's service/account identity. */
export interface Selection {
  selected: Set<string>;
  anchor: string | null;
  focus: string | null;
  rangeBase: Set<string> | null;
}

export const emptySelection = (): Selection => ({ selected: new Set(), anchor: null, focus: null, rangeBase: null });
export const selectAll = (keys: string[]): Selection => ({ ...emptySelection(), selected: new Set(keys) });

export function reconcileSelection(state: Selection, keys: string[]): Selection {
  const visible = new Set(keys);
  return {
    selected: new Set([...state.selected].filter((key) => visible.has(key))),
    anchor: state.anchor && visible.has(state.anchor) ? state.anchor : null,
    focus: state.focus && visible.has(state.focus) ? state.focus : null,
    rangeBase: state.rangeBase && new Set([...state.rangeBase].filter((key) => visible.has(key)))
  };
}

export function selectItem(state: Selection, keys: string[], key: string, modifiers: { ctrl?: boolean; shift?: boolean } = {}, checkbox = false): Selection {
  if (!keys.includes(key)) return state;
  const current = reconcileSelection(state, keys);
  const anchor = current.anchor ?? key;
  const range = keys.slice(Math.min(keys.indexOf(anchor), keys.indexOf(key)), Math.max(keys.indexOf(anchor), keys.indexOf(key)) + 1);
  if (modifiers.shift) {
    // A checkbox applies its new checked state to the range, preserving other checked items.
    if (checkbox) {
      const selected = new Set(current.selected);
      const remove = selected.has(key);
      for (const item of range) remove ? selected.delete(item) : selected.add(item);
      return { selected, anchor, focus: key, rangeBase: null };
    }
    // Keep the fixed anchor separate from the moving endpoint, so Shift can extend AND shrink.
    const rangeBase = modifiers.ctrl ? current.rangeBase ?? current.selected : new Set<string>();
    return { selected: new Set([...rangeBase, ...range]), anchor, focus: key, rangeBase };
  }
  const selected = new Set(modifiers.ctrl || checkbox ? current.selected : []);
  if ((modifiers.ctrl || checkbox) && selected.has(key)) selected.delete(key);
  else selected.add(key);
  return { selected, anchor: key, focus: key, rangeBase: null };
}

/** Native controls keep their own keyboard behavior (including Space on a checkbox). */
export function selectionControl(event: KeyboardEvent): boolean {
  if (!event.target) return false;
  if (!(event.target instanceof Element)) return false;
  const control = event.target.closest('input, textarea, select, button, a, [contenteditable="true"]');
  if (control instanceof HTMLInputElement && control.type === 'checkbox') return event.key === ' ' || event.key === 'Enter';
  return !!control;
}

export function selectionKey(state: Selection, keys: string[], event: KeyboardEvent, columns = 1): { selection: Selection; index?: number } | null {
  if (event.altKey || selectionControl(event)) return null;
  const ctrl = event.ctrlKey || event.metaKey;
  if (ctrl && event.key.toLowerCase() === 'a') return { selection: selectAll(keys) };
  if (event.key === 'Escape') return { selection: emptySelection() };
  const current = reconcileSelection(state, keys);
  const index = keys.indexOf(current.focus ?? '');
  if (event.key === ' ' && index >= 0) return { selection: selectItem(current, keys, keys[index], { shift: event.shiftKey }, true), index };
  const step = ({ ArrowDown: columns, ArrowUp: -columns, ArrowRight: 1, ArrowLeft: -1 } as Record<string, number>)[event.key];
  if (step === undefined && event.key !== 'Home' && event.key !== 'End') return null;
  if (!keys.length) return { selection: emptySelection() };
  const next = event.key === 'Home' ? 0 : event.key === 'End' ? keys.length - 1 : index < 0 ? 0 : Math.max(0, Math.min(keys.length - 1, index + step));
  const selection = ctrl && !event.shiftKey
    ? { ...current, anchor: current.anchor ?? keys[next], focus: keys[next], rangeBase: null }
    : selectItem(current, keys, keys[next], { ctrl, shift: event.shiftKey });
  return { selection, index: next };
}

/** Prevent Shift-click from also selecting text, without changing native checkbox behavior. */
export function selectionMouseDown(event: MouseEvent) {
  if (event.shiftKey && event.target instanceof Element && !event.target.closest('input, button, a')) event.preventDefault();
}
