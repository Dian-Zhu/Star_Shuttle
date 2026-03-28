import { describe, expect, it } from 'vitest';

import {
  canAttemptSessionReconnect,
  canContinueSessionReconnectFlow,
  createTerminalSessionState,
  interpretBackendSession,
  isTerminalSessionInteractive,
  isTerminalSessionRecoverable,
  mergeTerminalSessionState,
  parseBackendSessionLifecycleStatus,
} from './terminalSessionModel';

describe('terminalSessionModel', () => {
  it('creates an initial session state with defaults', () => {
    const state = createTerminalSessionState({
      connectionPhase: 'connected',
      terminalPhase: 'detached',
    });

    expect(state.connectionPhase).toBe('connected');
    expect(state.terminalPhase).toBe('detached');
    expect(state.reason).toBeNull();
    expect(state.restored).toBe(false);
    expect(typeof state.updatedAt).toBe('number');
  });

  it('merges partial updates onto existing state', () => {
    const current = createTerminalSessionState({
      connectionPhase: 'connected',
      terminalPhase: 'starting',
      restored: true,
    });

    const next = mergeTerminalSessionState(current, {
      terminalPhase: 'attached',
      reason: null,
    });

    expect(next.connectionPhase).toBe('connected');
    expect(next.terminalPhase).toBe('attached');
    expect(next.restored).toBe(true);
    expect(next.updatedAt).toBeGreaterThanOrEqual(current.updatedAt);
  });

  it('requires explicit phases for the initial state', () => {
    expect(() => mergeTerminalSessionState(undefined, { reason: 'missing' })).toThrow(
      /requires connectionPhase and terminalPhase/
    );
  });

  it('exposes interactive and recoverable predicates', () => {
    const attached = createTerminalSessionState({
      connectionPhase: 'connected',
      terminalPhase: 'attached',
    });
    expect(isTerminalSessionInteractive(attached)).toBe(true);
    expect(isTerminalSessionRecoverable(attached)).toBe(true);

    const reconnectFailed = createTerminalSessionState({
      connectionPhase: 'closed',
      terminalPhase: 'closed',
      reason: 'reconnect_failed',
    });
    expect(isTerminalSessionRecoverable(reconnectFailed)).toBe(true);

    const userClosed = createTerminalSessionState({
      connectionPhase: 'closed',
      terminalPhase: 'closed',
      reason: 'user_closed',
    });
    expect(isTerminalSessionRecoverable(userClosed)).toBe(false);
  });

  it('centralizes reconnect eligibility checks', () => {
    const connected = createTerminalSessionState({
      connectionPhase: 'connected',
      terminalPhase: 'closed',
    });

    expect(
      canAttemptSessionReconnect(connected, {
        inUi: true,
        reconnectSuppressed: false,
        reconnectInFlight: false,
      })
    ).toBe(true);

    expect(
      canAttemptSessionReconnect(connected, {
        inUi: true,
        reconnectSuppressed: false,
        reconnectInFlight: true,
      })
    ).toBe(false);

    expect(
      canContinueSessionReconnectFlow(connected, {
        inUi: true,
        reconnectSuppressed: false,
      })
    ).toBe(true);
  });

  it('normalizes backend lifecycle status values', () => {
    expect(parseBackendSessionLifecycleStatus('Connected')).toBe('connected');
    expect(parseBackendSessionLifecycleStatus('Disconnecting')).toBe('disconnecting');
    expect(parseBackendSessionLifecycleStatus('wat')).toBe('unknown');
  });

  it('interprets attached connected backend sessions as restorable terminals', () => {
    const interpreted = interpretBackendSession({
      id: 'session-1',
      connection_id: 'conn-1',
      status: 'Connected',
      terminal_id: 'term-1',
    });

    expect(interpreted).toEqual({
      sessionId: 'session-1',
      connectionId: 'conn-1',
      backendStatus: 'connected',
      connectionPhase: 'connected',
      terminalPhase: 'attached',
      terminalId: 'term-1',
      canRestoreTerminal: true,
    });
  });

  it('keeps connected detached sessions out of terminal restore set', () => {
    const interpreted = interpretBackendSession({
      id: 'session-2',
      connection_id: 'conn-2',
      status: 'Connected',
      terminal_id: null,
    });

    expect(interpreted?.connectionPhase).toBe('connected');
    expect(interpreted?.terminalPhase).toBe('detached');
    expect(interpreted?.canRestoreTerminal).toBe(false);
  });

  it('rejects backend sessions missing stable identifiers', () => {
    expect(
      interpretBackendSession({
        id: '',
        connection_id: 'conn-3',
        status: 'Connected',
      })
    ).toBeNull();
  });
});
