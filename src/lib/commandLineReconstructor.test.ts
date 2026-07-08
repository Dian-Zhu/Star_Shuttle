import { describe, it, expect } from 'vitest';
import {
  createCommandLineState,
  feedTerminalInput,
} from './commandLineReconstructor';

describe('commandLineReconstructor', () => {
  it('commits a simple command on carriage return', () => {
    const state = createCommandLineState();
    expect(feedTerminalInput(state, 'ls -la\r')).toEqual(['ls -la']);
    expect(state.buffer).toBe('');
  });

  it('accumulates across multiple feeds before commit', () => {
    const state = createCommandLineState();
    expect(feedTerminalInput(state, 'ec')).toEqual([]);
    expect(feedTerminalInput(state, 'ho hi')).toEqual([]);
    expect(feedTerminalInput(state, '\r')).toEqual(['echo hi']);
  });

  it('handles backspace edits', () => {
    const state = createCommandLineState();
    expect(feedTerminalInput(state, 'lss')).toEqual([]);
    expect(feedTerminalInput(state, '\x7f')).toEqual([]); // delete last 's'
    expect(feedTerminalInput(state, ' -l\r')).toEqual(['ls -l']);
  });

  it('drops the line on Ctrl-C', () => {
    const state = createCommandLineState();
    feedTerminalInput(state, 'rm -rf /');
    expect(feedTerminalInput(state, '\x03')).toEqual([]);
    expect(feedTerminalInput(state, 'ls\r')).toEqual(['ls']);
  });

  it('drops the line on Ctrl-U', () => {
    const state = createCommandLineState();
    feedTerminalInput(state, 'secret');
    feedTerminalInput(state, '\x15');
    expect(feedTerminalInput(state, 'ls\r')).toEqual(['ls']);
  });

  it('ignores empty / whitespace-only lines', () => {
    const state = createCommandLineState();
    expect(feedTerminalInput(state, '\r')).toEqual([]);
    expect(feedTerminalInput(state, '   \r')).toEqual([]);
  });

  it('handles \\r\\n without emitting a blank line', () => {
    const state = createCommandLineState();
    expect(feedTerminalInput(state, 'pwd\r\n')).toEqual(['pwd']);
  });

  it('commits multiple commands in one chunk', () => {
    const state = createCommandLineState();
    expect(feedTerminalInput(state, 'a\rb\rc\r')).toEqual(['a', 'b', 'c']);
  });

  it('taints a line that uses arrow keys (history recall) and does not record it', () => {
    const state = createCommandLineState();
    // user presses Up arrow (ESC [ A) to recall a previous command, then Enter
    expect(feedTerminalInput(state, '\x1b[A')).toEqual([]);
    expect(feedTerminalInput(state, '\r')).toEqual([]); // tainted -> not recorded
    // next fresh line records normally
    expect(feedTerminalInput(state, 'whoami\r')).toEqual(['whoami']);
  });

  it('taints a line that uses Tab completion', () => {
    const state = createCommandLineState();
    feedTerminalInput(state, 'cd /ho');
    feedTerminalInput(state, '\t'); // completion -> unknown result
    expect(feedTerminalInput(state, '\r')).toEqual([]);
  });

  it('taints on SS3 arrow (ESC O C) sequences', () => {
    const state = createCommandLineState();
    feedTerminalInput(state, 'ls');
    feedTerminalInput(state, '\x1bOC'); // right arrow (SS3)
    expect(feedTerminalInput(state, '\r')).toEqual([]);
  });

  it('skips OSC sequences terminated by BEL and taints the line', () => {
    const state = createCommandLineState();
    feedTerminalInput(state, 'x');
    feedTerminalInput(state, '\x1b]0;title\x07'); // OSC set-title
    feedTerminalInput(state, 'y');
    expect(feedTerminalInput(state, '\r')).toEqual([]); // tainted
  });

  it('does not leak escape sequence bytes into the command text', () => {
    const state = createCommandLineState();
    feedTerminalInput(state, 'ls\x1b[A'); // ls then up-arrow
    // buffer should still be just 'ls' (arrow skipped), though tainted
    expect(state.buffer).toBe('ls');
    expect(state.tainted).toBe(true);
  });
});
