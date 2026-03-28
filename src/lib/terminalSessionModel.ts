export type SessionConnectionPhase = 'connected' | 'reconnecting' | 'closing' | 'closed';

export type SessionTerminalPhase = 'detached' | 'starting' | 'attached' | 'closed';

export type BackendSessionLifecycleStatus =
  | 'connected'
  | 'connecting'
  | 'disconnecting'
  | 'disconnected'
  | 'error'
  | 'unknown';

export type BackendSessionInfo = {
  id: string;
  connection_id: string;
  status: string;
  terminal_id?: string | null;
};

export type InterpretedBackendSession = {
  sessionId: string;
  connectionId: string;
  backendStatus: BackendSessionLifecycleStatus;
  connectionPhase: SessionConnectionPhase;
  terminalPhase: SessionTerminalPhase;
  terminalId: string | null;
  canRestoreTerminal: boolean;
};

export type TerminalSessionState = {
  connectionPhase: SessionConnectionPhase;
  terminalPhase: SessionTerminalPhase;
  reason: string | null;
  restored: boolean;
  updatedAt: number;
};

export type TerminalSessionStatePatch = {
  connectionPhase?: SessionConnectionPhase;
  terminalPhase?: SessionTerminalPhase;
  reason?: string | null;
  restored?: boolean;
  updatedAt?: number;
};

export function createTerminalSessionState(
  patch: TerminalSessionStatePatch & {
    connectionPhase: SessionConnectionPhase;
    terminalPhase: SessionTerminalPhase;
  }
): TerminalSessionState {
  return {
    connectionPhase: patch.connectionPhase,
    terminalPhase: patch.terminalPhase,
    reason: patch.reason ?? null,
    restored: patch.restored ?? false,
    updatedAt: patch.updatedAt ?? Date.now(),
  };
}

export function mergeTerminalSessionState(
  current: TerminalSessionState | undefined,
  patch: TerminalSessionStatePatch
): TerminalSessionState {
  if (!current) {
    if (!patch.connectionPhase || !patch.terminalPhase) {
      throw new Error('Initial terminal session state requires connectionPhase and terminalPhase');
    }
    return createTerminalSessionState({
      connectionPhase: patch.connectionPhase,
      terminalPhase: patch.terminalPhase,
      reason: patch.reason,
      restored: patch.restored,
      updatedAt: patch.updatedAt,
    });
  }

  return {
    connectionPhase: patch.connectionPhase ?? current.connectionPhase,
    terminalPhase: patch.terminalPhase ?? current.terminalPhase,
    reason: patch.reason === undefined ? current.reason : patch.reason,
    restored: patch.restored ?? current.restored,
    updatedAt: patch.updatedAt ?? Date.now(),
  };
}

export function parseBackendSessionLifecycleStatus(
  status: unknown
): BackendSessionLifecycleStatus {
  const normalized = String(status ?? '').trim().toLowerCase();
  switch (normalized) {
    case 'connected':
      return 'connected';
    case 'connecting':
      return 'connecting';
    case 'disconnecting':
      return 'disconnecting';
    case 'disconnected':
      return 'disconnected';
    case 'error':
      return 'error';
    default:
      return 'unknown';
  }
}

export function interpretBackendSession(
  session: BackendSessionInfo | Record<string, unknown>
): InterpretedBackendSession | null {
  const sessionId = String((session as { id?: unknown }).id ?? '');
  const connectionId = String((session as { connection_id?: unknown }).connection_id ?? '');
  if (!sessionId || !connectionId) {
    return null;
  }

  const backendStatus = parseBackendSessionLifecycleStatus(
    (session as { status?: unknown }).status
  );
  const terminalId = (() => {
    const raw = (session as { terminal_id?: unknown }).terminal_id;
    if (typeof raw !== 'string') return null;
    const trimmed = raw.trim();
    return trimmed ? trimmed : null;
  })();

  let connectionPhase: SessionConnectionPhase;
  switch (backendStatus) {
    case 'connected':
      connectionPhase = 'connected';
      break;
    case 'connecting':
      connectionPhase = 'reconnecting';
      break;
    case 'disconnecting':
      connectionPhase = 'closing';
      break;
    case 'disconnected':
    case 'error':
    case 'unknown':
      connectionPhase = 'closed';
      break;
  }

  const terminalPhase: SessionTerminalPhase =
    connectionPhase === 'connected'
      ? terminalId
        ? 'attached'
        : 'detached'
      : 'closed';

  return {
    sessionId,
    connectionId,
    backendStatus,
    connectionPhase,
    terminalPhase,
    terminalId,
    canRestoreTerminal: connectionPhase === 'connected' && terminalId !== null,
  };
}

export type TerminalSessionReconnectContext = {
  inUi: boolean;
  reconnectSuppressed: boolean;
  reconnectInFlight?: boolean;
};

export function isTerminalSessionInteractive(
  state: TerminalSessionState | undefined
): boolean {
  if (!state) return false;
  return state.connectionPhase === 'connected' && state.terminalPhase === 'attached';
}

export function isTerminalSessionRecoverable(
  state: TerminalSessionState | undefined
): boolean {
  if (!state) return true;
  if (state.connectionPhase === 'closing') return false;
  if (state.connectionPhase === 'closed') {
    return state.reason === 'reconnect_failed';
  }
  return true;
}

export function canAttemptSessionReconnect(
  state: TerminalSessionState | undefined,
  context: TerminalSessionReconnectContext
): boolean {
  if (!context.inUi || context.reconnectSuppressed) return false;
  if (context.reconnectInFlight === true) return false;
  return isTerminalSessionRecoverable(state);
}

export function canContinueSessionReconnectFlow(
  state: TerminalSessionState | undefined,
  context: Omit<TerminalSessionReconnectContext, 'reconnectInFlight'>
): boolean {
  if (!context.inUi || context.reconnectSuppressed) return false;
  return isTerminalSessionRecoverable(state);
}
