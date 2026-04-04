import { describe, expect, it } from 'vitest';

import { sanitizeTerminalDisplayText } from './terminalDisplaySanitizer';

describe('terminalDisplaySanitizer', () => {
  it('removes ansi and osc escape sequences', () => {
    const value = 'oops\x1b[31mred\x1b[0m\x1b]8;;https://evil.example\x07link\x1b]8;;\x07';
    expect(sanitizeTerminalDisplayText(value)).toBe('oopsredlink');
  });

  it('removes other control characters while keeping newlines and unicode', () => {
    const value = '你\u0000好\r\nworld\u0007';
    expect(sanitizeTerminalDisplayText(value)).toBe('你好\nworld');
  });
});
