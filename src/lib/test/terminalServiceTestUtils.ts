import { vi } from 'vitest';

export type InvokeCall = {
  command: string;
  args: Record<string, unknown> | undefined;
};

export type InvokeHandler = (
  command: string,
  args: Record<string, unknown> | undefined,
  calls: InvokeCall[]
) => Promise<unknown> | unknown;

type EventPayload = {
  payload?: Record<string, unknown>;
};

type EventCallback = (event: EventPayload) => void;

export class MockTerminal {
  cols = 120;
  rows = 40;
  writes: string[] = [];
  private dataListeners: Array<(data: string) => void> = [];

  write(data: string, cb?: () => void): void {
    this.writes.push(data);
    cb?.();
  }

  focus(): void {}

  onData(listener: (data: string) => void): { dispose: () => void } {
    this.dataListeners.push(listener);
    let disposed = false;
    return {
      dispose: () => {
        if (disposed) return;
        disposed = true;
        this.dataListeners = this.dataListeners.filter((item) => item !== listener);
      },
    };
  }

  emitData(data: string): void {
    const listeners = [...this.dataListeners];
    for (const listener of listeners) {
      listener(data);
    }
  }
}

type TerminalServiceHarnessOptions = {
  parseHostKeyPromptResult?: unknown;
  buildHostKeyConfirmMessageResult?: string;
};

export async function createTerminalServiceHarness(
  invokeHandler?: InvokeHandler,
  options: TerminalServiceHarnessOptions = {}
) {
  vi.resetModules();

  const localStorageData = new Map<string, string>();
  const localStorageMock = {
    getItem: (key: string) => (localStorageData.has(key) ? localStorageData.get(key)! : null),
    setItem: (key: string, value: string) => {
      localStorageData.set(key, value);
    },
    removeItem: (key: string) => {
      localStorageData.delete(key);
    },
    clear: () => {
      localStorageData.clear();
    },
  };

  Object.defineProperty(globalThis, 'localStorage', {
    value: localStorageMock,
    configurable: true,
    writable: true,
  });

  const windowListeners = new Map<string, Set<(event: Event) => void>>();
  const windowMock = {
    setTimeout,
    clearTimeout,
    open: vi.fn(),
    prompt: vi.fn(() => null),
    confirm: vi.fn(() => false),
    addEventListener: vi.fn((type: string, listener: (event: Event) => void) => {
      const listeners = windowListeners.get(type) ?? new Set<(event: Event) => void>();
      listeners.add(listener);
      windowListeners.set(type, listeners);
    }),
    removeEventListener: vi.fn((type: string, listener: (event: Event) => void) => {
      const listeners = windowListeners.get(type);
      listeners?.delete(listener);
      if (listeners && listeners.size === 0) {
        windowListeners.delete(type);
      }
    }),
    dispatchEvent: vi.fn((event: Event) => {
      const listeners = [...(windowListeners.get(event.type) ?? [])];
      for (const listener of listeners) {
        listener(event);
      }
      return true;
    }),
  };
  Object.defineProperty(globalThis, 'window', {
    value: windowMock,
    configurable: true,
    writable: true,
  });

  const eventListeners = new Map<string, Set<EventCallback>>();
  const invokeCalls: InvokeCall[] = [];

  const listenMock = vi.fn(async (eventName: string, cb: EventCallback) => {
    const listeners = eventListeners.get(eventName) ?? new Set<EventCallback>();
    listeners.add(cb);
    eventListeners.set(eventName, listeners);
    return () => {
      const current = eventListeners.get(eventName);
      current?.delete(cb);
      if (current && current.size === 0) {
        eventListeners.delete(eventName);
      }
    };
  });

  const invokeMock = vi.fn(
    async (command: string, args?: Record<string, unknown>): Promise<unknown> => {
      invokeCalls.push({ command, args });
      if (invokeHandler) {
        return await invokeHandler(command, args, invokeCalls);
      }
      if (command === 'connect') return 'session-auto';
      if (command === 'start_terminal') return true;
      return undefined;
    }
  );

  vi.doMock('@tauri-apps/api/core', () => ({
    invoke: invokeMock,
  }));

  vi.doMock('@tauri-apps/api/event', () => ({
    listen: listenMock,
  }));

  vi.doMock('../terminalPool', () => ({
    terminalPool: {
      migrateSession: vi.fn(() => true),
      destroyInstance: vi.fn(),
      registerInstance: vi.fn(),
      retrieveInstance: vi.fn(),
    },
  }));

  vi.doMock('../hostKeyPrompt', () => ({
    buildHostKeyConfirmMessage: vi.fn(() => options.buildHostKeyConfirmMessageResult ?? 'confirm host key'),
    parseHostKeyPrompt: vi.fn(() => options.parseHostKeyPromptResult ?? null),
    saveHostKeyPrompt: vi.fn(async () => undefined),
  }));

  vi.doMock('@xterm/xterm', () => ({
    Terminal: class Terminal {},
  }));
  vi.doMock('@xterm/addon-fit', () => ({
    FitAddon: class FitAddon {},
  }));
  vi.doMock('@xterm/addon-search', () => ({
    SearchAddon: class SearchAddon {},
  }));
  vi.doMock('@xterm/addon-webgl', () => ({
    WebglAddon: class WebglAddon {
      onContextLoss(): void {}
      dispose(): void {}
    },
  }));
  vi.doMock('@xterm/addon-web-links', () => ({
    WebLinksAddon: class WebLinksAddon {},
  }));
  vi.doMock('@xterm/xterm/css/xterm.css', () => ({}));

  const terminalService = await import('../terminalService');
  const store = await import('../store');

  const emitEvent = (eventName: string, payload?: Record<string, unknown>) => {
    const listeners = [...(eventListeners.get(eventName) ?? [])];
    for (const listener of listeners) {
      listener({ payload });
    }
  };

  const flush = async () => {
    await Promise.resolve();
    await Promise.resolve();
  };

  const resetStores = () => {
    store.activeTerminals.set([]);
    store.broadcastSessionIds.set([]);
    store.terminalSessionMap.set(new Map());
    store.selectedTerminalIndex.set(0);
    store.broadcastInputEnabled.set(false);
    store.settings.update((value) => ({
      ...value,
      connection: {
        ...value.connection,
        autoReconnect: false,
      },
    }));
  };

  return {
    terminalService,
    store,
    invokeMock,
    listenMock,
    invokeCalls,
    emitEvent,
    flush,
    resetStores,
  };
}
