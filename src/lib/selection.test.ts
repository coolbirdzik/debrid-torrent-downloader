import { describe, expect, it } from 'vitest';
import { emptySelection, reconcileSelection, selectItem, selectionKey } from './selection';

const keys = ['a', 'b', 'c', 'd', 'e', 'f'];
const keyEvent = (key: string, extra = {}) => ({ key, target: null, ...extra }) as KeyboardEvent;
const selected = (state: ReturnType<typeof emptySelection>) => [...state.selected];

describe('shared file selection', () => {
  it('selects forward/backward ranges and shrinks around the fixed anchor', () => {
    let state = selectItem(emptySelection(), keys, 'c');
    state = selectItem(state, keys, 'f', { shift: true });
    expect(selected(state)).toEqual(['c', 'd', 'e', 'f']);
    state = selectItem(state, keys, 'd', { shift: true });
    expect(selected(state)).toEqual(['c', 'd']);
    state = selectItem(state, keys, 'a', { shift: true });
    expect(selected(state)).toEqual(['a', 'b', 'c']);
    expect(state.anchor).toBe('c');
    expect(state.focus).toBe('a');
  });

  it('Ctrl/Cmd toggles individual rows and Ctrl+Shift preserves separate selections', () => {
    let state = selectItem(emptySelection(), keys, 'f');
    state = selectItem(state, keys, 'a', { ctrl: true });
    state = selectItem(state, keys, 'd', { ctrl: true, shift: true });
    expect(selected(state)).toEqual(['f', 'a', 'b', 'c', 'd']);
    state = selectItem(state, keys, 'b', { ctrl: true, shift: true });
    expect(selected(state)).toEqual(['f', 'a', 'b']);
    state = selectItem(state, keys, 'f', { ctrl: true });
    expect(selected(state)).toEqual(['a', 'b']);
  });

  it('Shift checkboxes check or uncheck whole ranges while preserving other checked files', () => {
    let state = selectItem(emptySelection(), keys, 'f', {}, true);
    state = selectItem(state, keys, 'a', {}, true);
    state = selectItem(state, keys, 'd', { shift: true }, true);
    expect(selected(state).sort()).toEqual(['a', 'b', 'c', 'd', 'f']);
    state = selectItem(state, keys, 'c', { shift: true }, true);
    expect(selected(state).sort()).toEqual(['d', 'f']);
  });

  it('missing or removed anchors safely start a new range at the clicked file', () => {
    const state = selectItem(emptySelection(), keys, 'a');
    expect(selected(selectItem(state, ['d', 'e', 'f'], 'e', { shift: true }))).toEqual(['e']);
    expect(selected(selectItem(emptySelection(), keys, 'd', { shift: true }))).toEqual(['d']);
    expect(selectItem(state, keys, 'hidden', { shift: true })).toBe(state);
  });

  it('ranges follow the full visible ordering, including virtualized offscreen files', () => {
    const visible = Array.from({ length: 1000 }, (_, i) => `torBox|account|${i}`);
    let state = selectItem(emptySelection(), visible, visible[1]);
    state = selectItem(state, visible, visible[900], { shift: true });
    expect(state.selected.size).toBe(900);
    expect(state.selected.has(visible[900])).toBe(true);
    // Provider filtering cannot pull hidden services into the range.
    expect(selected(selectItem(state, ['rd|1', 'rd|3'], 'rd|3', { shift: true }))).toEqual(['rd|3']);
  });

  it('repeated Shift+Arrow moves the endpoint and can reverse/shrink', () => {
    let state = selectItem(emptySelection(), keys, 'b');
    for (let i = 0; i < 3; i++) state = selectionKey(state, keys, keyEvent('ArrowDown', { shiftKey: true }))!.selection;
    expect(selected(state)).toEqual(['b', 'c', 'd', 'e']);
    state = selectionKey(state, keys, keyEvent('ArrowUp', { shiftKey: true }))!.selection;
    expect(selected(state)).toEqual(['b', 'c', 'd']);
    state = selectionKey(state, keys, keyEvent('Home', { shiftKey: true }))!.selection;
    expect(selected(state)).toEqual(['a', 'b']);
    state = selectionKey(state, keys, keyEvent('End', { shiftKey: true }))!.selection;
    expect(selected(state)).toEqual(['b', 'c', 'd', 'e', 'f']);
  });

  it('grid arrows use the number of columns and Ctrl+Arrow moves focus without changing selection', () => {
    let state = selectItem(emptySelection(), keys, 'a');
    const result = selectionKey(state, keys, keyEvent('ArrowDown', { shiftKey: true }), 3)!;
    expect(result.index).toBe(3);
    expect(selected(result.selection)).toEqual(['a', 'b', 'c', 'd']);
    state = selectionKey(result.selection, keys, keyEvent('ArrowRight', { ctrlKey: true }), 3)!.selection;
    expect(state.focus).toBe('e');
    expect(selected(state)).toEqual(['a', 'b', 'c', 'd']);
  });

  it('Ctrl+A always selects all, Escape clears, and deletion prunes anchor/focus', () => {
    let state = selectionKey(emptySelection(), keys, keyEvent('a', { ctrlKey: true }))!.selection;
    state = selectionKey(state, keys, keyEvent('a', { ctrlKey: true }))!.selection;
    expect(selected(state)).toEqual(keys);
    state = selectItem(state, keys, 'b');
    expect(reconcileSelection(state, ['a', 'c'])).toEqual(emptySelection());
    expect(selectionKey(state, keys, keyEvent('Escape'))!.selection).toEqual(emptySelection());
    expect(selectionKey(state, [], keyEvent('ArrowDown'))!.selection).toEqual(emptySelection());
    expect(selectionKey(state, keys, keyEvent('ArrowLeft', { altKey: true }))).toBeNull();
  });
});
