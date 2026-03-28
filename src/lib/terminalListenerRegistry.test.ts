import { describe, expect, it, vi } from 'vitest';

import {
  armTerminalKeyListener,
  attachTerminalInputListener,
  clearOutputListenerCleanup,
  clearReconnectKeyListener,
  detachTerminalInputListener,
  getOutputListenerCleanup,
  setOutputListenerCleanup,
} from './terminalListenerRegistry';

class MockTerminal {
  private listeners = new Set<(data: string) => void>();

  onData(callback: (data: string) => void) {
    this.listeners.add(callback);
    return {
      dispose: () => {
        this.listeners.delete(callback);
      },
    };
  }

  emit(data: string) {
    for (const listener of Array.from(this.listeners)) {
      listener(data);
    }
  }
}

describe('terminalListenerRegistry', () => {
  it('replaces terminal input listeners for the same session', () => {
    const term = new MockTerminal();
    const first = vi.fn();
    const second = vi.fn();

    attachTerminalInputListener('session-input', term as never, first);
    attachTerminalInputListener('session-input', term as never, second);

    term.emit('x');

    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledWith('x');

    detachTerminalInputListener('session-input');
  });

  it('runs and clears output listener cleanup once', () => {
    const cleanup = vi.fn();

    setOutputListenerCleanup('session-output', cleanup);
    expect(getOutputListenerCleanup('session-output')).toBeTypeOf('function');

    clearOutputListenerCleanup('session-output');
    clearOutputListenerCleanup('session-output');

    expect(cleanup).toHaveBeenCalledTimes(1);
    expect(getOutputListenerCleanup('session-output')).toBeUndefined();
  });

  it('arms a one-shot terminal key listener', async () => {
    const term = new MockTerminal();
    const trigger = vi.fn();

    armTerminalKeyListener(
      'session-key',
      term as never,
      (data: string) => data === 'R',
      trigger
    );

    term.emit('x');
    term.emit('R');
    term.emit('R');

    expect(trigger).toHaveBeenCalledTimes(1);

    clearReconnectKeyListener('session-key');
  });
});
