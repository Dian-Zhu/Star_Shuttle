const reconnectAttempts = new Map<string, number>();
const reconnectTimers = new Map<string, number>();
const reconnectInFlight = new Map<string, Promise<boolean>>();
const reconnectSuppressed = new Set<string>();
const reconnectConnectConfigs = new Map<string, any>();

export const MAX_AUTO_RECONNECT_RETRIES = 5;
export const BASE_AUTO_RECONNECT_DELAY_MS = 1500;
export const MAX_AUTO_RECONNECT_DELAY_MS = 30000;

function cloneConnectConfig(config: any): any {
  if (typeof structuredClone === 'function') {
    try {
      return structuredClone(config);
    } catch {
      // Fall through to JSON clone.
    }
  }
  try {
    return JSON.parse(JSON.stringify(config));
  } catch {
    return config;
  }
}

export function getReconnectAttempts(sessionId: string): number {
  return reconnectAttempts.get(sessionId) ?? 0;
}

export function setReconnectAttempts(sessionId: string, attempts: number) {
  reconnectAttempts.set(sessionId, attempts);
}

export function clearReconnectAttempts(sessionId: string) {
  reconnectAttempts.delete(sessionId);
}

export function getReconnectTimer(sessionId: string): number | undefined {
  return reconnectTimers.get(sessionId);
}

export function setReconnectTimer(sessionId: string, timer: number) {
  reconnectTimers.set(sessionId, timer);
}

export function clearReconnectTimer(sessionId: string) {
  const timer = reconnectTimers.get(sessionId);
  if (timer !== undefined) {
    clearTimeout(timer);
    reconnectTimers.delete(sessionId);
  }
}

export function isReconnectSuppressed(sessionId: string): boolean {
  return reconnectSuppressed.has(sessionId);
}

export function suppressReconnectState(sessionId: string) {
  reconnectSuppressed.add(sessionId);
}

export function releaseReconnectSuppressionState(sessionId: string) {
  reconnectSuppressed.delete(sessionId);
}

export function getReconnectInFlight(sessionId: string): Promise<boolean> | undefined {
  return reconnectInFlight.get(sessionId);
}

export function setReconnectInFlight(sessionId: string, promise: Promise<boolean>) {
  reconnectInFlight.set(sessionId, promise);
}

export function clearReconnectInFlight(sessionId: string, promise?: Promise<boolean>) {
  if (promise && reconnectInFlight.get(sessionId) !== promise) {
    return;
  }
  reconnectInFlight.delete(sessionId);
}

export function rememberReconnectConfig(sessionId: string, connectConfig: any) {
  reconnectConnectConfigs.set(sessionId, cloneConnectConfig(connectConfig));
}

export function getReconnectConfig(sessionId: string): any {
  return reconnectConnectConfigs.get(sessionId);
}

export function clearReconnectConfig(sessionId: string) {
  reconnectConnectConfigs.delete(sessionId);
}
