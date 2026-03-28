import { describe, expect, it, vi } from 'vitest';
import type { Terminal } from 'xterm';
import {
  __terminalIoBufferTestHooks,
  cleanupBufferedIoState,
  enqueueTerminalOutput,
  initializeOutputBuffer,
  sendTerminalDataBuffered,
} from './terminalIoBuffer';

const logger = {
  info: vi.fn(),
  error: vi.fn(),
  perf: vi.fn(),
};

async function flushAsync() {
  await Promise.resolve();
  await Promise.resolve();
}

describe('terminalIoBuffer', () => {
  const resetState = () => {
    __terminalIoBufferTestHooks.cleanupAll();
    vi.useRealTimers();
    vi.clearAllMocks();
  };

  it('sends input immediately when immediate=true', async () => {
    const invokeSend = vi.fn(async () => {});
    sendTerminalDataBuffered('s1', 'pwd\n', true, {
      invokeSend,
      logger,
    });

    await flushAsync();

    expect(invokeSend).toHaveBeenCalledTimes(1);
    expect(invokeSend).toHaveBeenCalledWith('s1', 'pwd\n');
    resetState();
  });

  it('cancels buffered input flush after cleanup', async () => {
    vi.useFakeTimers();
    const invokeSend = vi.fn(async () => {});
    sendTerminalDataBuffered('s2', 'hello', false, {
      invokeSend,
      logger,
    });

    cleanupBufferedIoState('s2');
    vi.runAllTimers();
    await flushAsync();

    expect(invokeSend).not.toHaveBeenCalled();
    resetState();
  });

  it('flushes queued output chunks into terminal write', async () => {
    vi.useFakeTimers();
    const writes: string[] = [];
    const term = {
      write: (data: string, done?: () => void) => {
        writes.push(data);
        done?.();
      },
    } as unknown as Terminal;

    initializeOutputBuffer('s3');
    enqueueTerminalOutput('s3', term, 'hello', { logger, isDev: false });
    enqueueTerminalOutput('s3', term, ' world', { logger, isDev: false });

    vi.runAllTimers();
    await flushAsync();

    expect(writes.join('')).toBe('hello world');
    resetState();
  });
});
