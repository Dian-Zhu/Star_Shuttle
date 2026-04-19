import { describe, expect, it } from 'vitest';

import { extractTerminalWorkingDirectory } from './terminalCwd';

describe('extractTerminalWorkingDirectory', () => {
  it('extracts cwd from a complete OSC 7 sequence', () => {
    const result = extractTerminalWorkingDirectory('\u001b]7;file://host/home/rust/project\u0007');
    expect(result.cwd).toBe('/home/rust/project');
    expect(result.remainder).toBe('');
  });

  it('supports split OSC 7 sequences across chunks', () => {
    const first = extractTerminalWorkingDirectory('\u001b]7;file://host/home/rust', '');
    expect(first.cwd).toBeNull();
    expect(first.remainder).toBe('\u001b]7;file://host/home/rust');

    const second = extractTerminalWorkingDirectory('/workspace\u001b\\', first.remainder);
    expect(second.cwd).toBe('/home/rust/workspace');
    expect(second.remainder).toBe('');
  });

  it('keeps the latest cwd when multiple OSC 7 sequences appear', () => {
    const result = extractTerminalWorkingDirectory(
      '\u001b]7;file://host/tmp\u0007output\u001b]7;file://host/var/log\u0007'
    );
    expect(result.cwd).toBe('/var/log');
  });
});

