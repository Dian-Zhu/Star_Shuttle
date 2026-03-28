import type { Terminal } from '@xterm/xterm';

type Disposable = { dispose: () => void };

const outputListeners = new Map<string, () => void>();
const inputListeners = new Map<string, Disposable>();
const reconnectKeyListeners = new Map<string, Disposable>();

export function getOutputListenerCleanup(sessionId: string): (() => void) | undefined {
  return outputListeners.get(sessionId);
}

export function setOutputListenerCleanup(sessionId: string, cleanup: () => void) {
  outputListeners.set(sessionId, cleanup);
}

export function clearOutputListenerCleanup(sessionId: string) {
  const cleanup = outputListeners.get(sessionId);
  if (cleanup) {
    cleanup();
    outputListeners.delete(sessionId);
  }
}

export function deleteOutputListenerCleanup(sessionId: string) {
  outputListeners.delete(sessionId);
}

export function detachTerminalInputListener(sessionId: string) {
  const inputListener = inputListeners.get(sessionId);
  if (inputListener) {
    inputListener.dispose();
    inputListeners.delete(sessionId);
  }
}

export function attachTerminalInputListener(
  sessionId: string,
  term: Terminal,
  onData: (data: string) => void
) {
  detachTerminalInputListener(sessionId);
  const disposable = term.onData(onData);
  inputListeners.set(sessionId, disposable);
}

export function clearReconnectKeyListener(sessionId: string) {
  const listener = reconnectKeyListeners.get(sessionId);
  if (listener) {
    listener.dispose();
    reconnectKeyListeners.delete(sessionId);
  }
}

export function armTerminalKeyListener(
  sessionId: string,
  term: Terminal,
  matcher: (data: string) => boolean,
  onTrigger: () => void | Promise<void>
) {
  clearReconnectKeyListener(sessionId);
  const disposable = term.onData((data: string) => {
    if (!matcher(data)) return;
    disposable.dispose();
    reconnectKeyListeners.delete(sessionId);
    void onTrigger();
  });
  reconnectKeyListeners.set(sessionId, disposable);
}
