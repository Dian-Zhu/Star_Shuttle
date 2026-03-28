import { describe, expect, it } from 'vitest';

import {
  getShortcutFromKeyboardEvent,
  matchShortcut,
  normalizeShortcut,
} from './shortcuts';

function keyboardEvent(
  overrides: Partial<
    Pick<KeyboardEvent, 'key' | 'code' | 'ctrlKey' | 'shiftKey' | 'altKey' | 'metaKey'>
  >
): KeyboardEvent {
  return {
    key: '',
    code: '',
    ctrlKey: false,
    shiftKey: false,
    altKey: false,
    metaKey: false,
    ...overrides,
  } as KeyboardEvent;
}

describe('shortcuts', () => {
  it('normalizes aliases and key casing', () => {
    expect(normalizeShortcut('ctrl+shift+p')).toEqual({ value: 'Ctrl+Shift+P' });
    expect(normalizeShortcut('del')).toEqual({ value: 'Delete' });
    expect(normalizeShortcut('esc')).toEqual({ value: 'Escape' });
  });

  it('matches punctuation shortcuts with code fallback', () => {
    const event = keyboardEvent({
      key: 'Process',
      code: 'BracketLeft',
      ctrlKey: true,
      shiftKey: true,
    });

    expect(matchShortcut(event, 'Ctrl+Shift+[')).toBe(true);
  });

  it('captures keyboard events into normalized shortcuts', () => {
    const shortcut = getShortcutFromKeyboardEvent(
      keyboardEvent({
        key: 'b',
        code: 'KeyB',
        ctrlKey: true,
        shiftKey: true,
      })
    );

    expect(shortcut).toBe('Ctrl+Shift+B');
  });

  it('captures function and special keys', () => {
    expect(
      getShortcutFromKeyboardEvent(
        keyboardEvent({
          key: 'F5',
          code: 'F5',
        })
      )
    ).toBe('F5');

    expect(
      getShortcutFromKeyboardEvent(
        keyboardEvent({
          key: 'Delete',
          code: 'Delete',
        })
      )
    ).toBe('Delete');
  });
});
