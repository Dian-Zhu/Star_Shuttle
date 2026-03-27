import { describe, expect, it } from 'vitest';

import {
  getShortcutFromKeyboardEvent,
  matchShortcut,
  normalizeShortcut,
} from './shortcuts';

describe('shortcuts', () => {
  it('normalizes aliases and key casing', () => {
    expect(normalizeShortcut('ctrl+shift+p')).toEqual({ value: 'Ctrl+Shift+P' });
    expect(normalizeShortcut('del')).toEqual({ value: 'Delete' });
    expect(normalizeShortcut('esc')).toEqual({ value: 'Escape' });
  });

  it('matches punctuation shortcuts with code fallback', () => {
    const event = new KeyboardEvent('keydown', {
      key: 'Process',
      code: 'BracketLeft',
      ctrlKey: true,
      shiftKey: true,
    });

    expect(matchShortcut(event, 'Ctrl+Shift+[')).toBe(true);
  });

  it('captures keyboard events into normalized shortcuts', () => {
    const shortcut = getShortcutFromKeyboardEvent(
      new KeyboardEvent('keydown', {
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
        new KeyboardEvent('keydown', {
          key: 'F5',
          code: 'F5',
        })
      )
    ).toBe('F5');

    expect(
      getShortcutFromKeyboardEvent(
        new KeyboardEvent('keydown', {
          key: 'Delete',
          code: 'Delete',
        })
      )
    ).toBe('Delete');
  });
});
