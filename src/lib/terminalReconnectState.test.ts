import { describe, expect, it, vi } from 'vitest';

import {
  clearReconnectAttempts,
  clearReconnectConfig,
  clearReconnectInFlight,
  clearReconnectTimer,
  getReconnectAttempts,
  getReconnectConfig,
  getReconnectInFlight,
  isReconnectSuppressed,
  releaseReconnectSuppressionState,
  rememberReconnectConfig,
  setReconnectAttempts,
  setReconnectInFlight,
  setReconnectTimer,
  suppressReconnectState,
} from './terminalReconnectState';

describe('terminalReconnectState', () => {
  it('stores and clears reconnect attempts and suppression state', () => {
    const sessionId = 'reconnect-attempts';

    setReconnectAttempts(sessionId, 3);
    suppressReconnectState(sessionId);

    expect(getReconnectAttempts(sessionId)).toBe(3);
    expect(isReconnectSuppressed(sessionId)).toBe(true);

    clearReconnectAttempts(sessionId);
    releaseReconnectSuppressionState(sessionId);

    expect(getReconnectAttempts(sessionId)).toBe(0);
    expect(isReconnectSuppressed(sessionId)).toBe(false);
  });

  it('clones remembered reconnect config before storing it', () => {
    const sessionId = 'reconnect-config';
    const config = {
      nested: {
        answer: 42,
      },
    };

    rememberReconnectConfig(sessionId, config);
    config.nested.answer = 7;

    expect(getReconnectConfig(sessionId)).toEqual({
      nested: {
        answer: 42,
      },
    });

    clearReconnectConfig(sessionId);
    expect(getReconnectConfig(sessionId)).toBeUndefined();
  });

  it('only clears in-flight reconnect promise when the same promise finishes', async () => {
    const sessionId = 'reconnect-inflight';
    const first = Promise.resolve(true);
    const second = Promise.resolve(false);

    setReconnectInFlight(sessionId, first);
    clearReconnectInFlight(sessionId, second);
    expect(getReconnectInFlight(sessionId)).toBe(first);

    clearReconnectInFlight(sessionId, first);
    expect(getReconnectInFlight(sessionId)).toBeUndefined();
  });

  it('clears scheduled reconnect timers', () => {
    vi.useFakeTimers();
    const sessionId = 'reconnect-timer';
    const callback = vi.fn();

    const timer = setTimeout(callback, 10) as unknown as number;
    setReconnectTimer(sessionId, timer);
    clearReconnectTimer(sessionId);

    vi.advanceTimersByTime(20);
    expect(callback).not.toHaveBeenCalled();
    vi.useRealTimers();
  });
});
