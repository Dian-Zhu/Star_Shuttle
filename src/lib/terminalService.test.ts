import { describe, expect, it, vi } from 'vitest';

import { sanitizeTerminalDisplayText } from './terminalDisplaySanitizer';
import { MockTerminal, createTerminalServiceHarness } from './test/terminalServiceTestUtils';
import type { Connection } from './store';

function createConnection(id: string, name = 'demo'): Connection {
  return {
    id,
    name,
    protocol: 'Ssh',
    host: '127.0.0.1',
    port: 22,
    username: 'root',
    auth_method: {
      KeyboardInteractive: {},
    },
    description: null,
    tags: [],
    created_at: '2026-03-28T00:00:00.000Z',
    updated_at: '2026-03-28T00:00:00.000Z',
    group_id: null,
    local_forwards: [],
    remote_forwards: [],
    proxy_type: 'None',
    socks_proxy_port: null,
    auto_reconnect: false,
  };
}

describe('terminalService display hardening', () => {
  it('sanitizes backend-controlled terminal text', async () => {
    expect(sanitizeTerminalDisplayText('oops\x1b[31mred\x1b[0m\r\nnext')).toBe('oopsred\nnext');
  });

  it('uses event-driven password prompt instead of window.prompt', async () => {
    const seenPasswords: string[] = [];
    const harness = await createTerminalServiceHarness(async (command, args) => {
      if (command === 'connect') {
        const password = (args?.config as { auth_method?: { Password?: { password?: string } } } | undefined)
          ?.auth_method?.Password?.password;
        seenPasswords.push(typeof password === 'string' ? password : '');
        return 'session-password';
      }
      if (command === 'start_terminal') return true;
      return undefined;
    });

    const { terminalService, flush } = harness;
    const connection = {
      ...createConnection('conn-password', 'secret-demo'),
      auth_method: {
        Password: {
          password: '',
          save_password: false,
        },
      },
    } as Connection;

    const passwordRequest = new Promise<CustomEvent>((resolve) => {
      window.addEventListener(
        'starshuttle:password-prompt-request',
        ((event: Event) => {
          event.preventDefault();
          resolve(event as CustomEvent);
        }) as EventListener,
        { once: true }
      );
    });

    const connectPromise = terminalService.createTerminalSession(connection);
    const event = await passwordRequest;
    event.detail.resolve('  typed-secret  ');
    await expect(connectPromise).resolves.toBe('session-password');
    await flush();
    expect(seenPasswords).toEqual(['  typed-secret  ']);
  });

  it('fails fast when no password prompt handler is mounted', async () => {
    const harness = await createTerminalServiceHarness(async (command) => {
      if (command === 'connect') return 'session-password';
      if (command === 'start_terminal') return true;
      return undefined;
    });

    const { terminalService } = harness;
    const connection = {
      ...createConnection('conn-password-missing', 'secret-demo'),
      auth_method: {
        Password: {
          password: '',
          save_password: false,
        },
      },
    } as Connection;

    await expect(terminalService.createTerminalSession(connection)).rejects.toThrow('密码输入不可用');
  });

  it('uses event-driven confirm dialog for host key prompts', async () => {
    let connectAttempts = 0;
    const harness = await createTerminalServiceHarness(
      async (command) => {
        if (command === 'connect') {
          connectAttempts += 1;
          if (connectAttempts === 1) {
            throw new Error('host key mismatch');
          }
          return 'session-host-key';
        }
        if (command === 'start_terminal') return true;
        return undefined;
      },
      {
        parseHostKeyPromptResult: {
          type: 'mismatch',
          payload: {
            host: '127.0.0.1',
            port: 22,
            fingerprint: 'sha256:test',
            key_type: 'ssh-ed25519',
            key_base64: 'abc',
            challenge_token: 'token',
          },
        },
        buildHostKeyConfirmMessageResult: 'confirm host key',
      }
    );

    const { terminalService, flush } = harness;
    const confirmRequest = new Promise<CustomEvent>((resolve) => {
      window.addEventListener(
        'starshuttle:confirm-dialog-request',
        ((event: Event) => {
          event.preventDefault();
          resolve(event as CustomEvent);
        }) as EventListener,
        { once: true }
      );
    });

    const connectPromise = terminalService.createTerminalSession(createConnection('conn-host-key'));
    const event = await confirmRequest;
    expect(event.detail.title).toBe('主机密钥确认');
    expect(event.detail.kind).toBe('danger');
    event.detail.resolve(true);

    await expect(connectPromise).resolves.toBe('session-host-key');
    await flush();
  });

  it('rejects host key prompts when no confirm dialog listener is available', async () => {
    let connectAttempts = 0;
    const harness = await createTerminalServiceHarness(
      async (command) => {
        if (command === 'connect') {
          connectAttempts += 1;
          if (connectAttempts === 1) {
            throw new Error('host key mismatch');
          }
          return 'session-host-key';
        }
        if (command === 'start_terminal') return true;
        return undefined;
      },
      {
        parseHostKeyPromptResult: {
          type: 'mismatch',
          payload: {
            host: '127.0.0.1',
            port: 22,
            fingerprint: 'sha256:test',
            key_type: 'ssh-ed25519',
            key_base64: 'abc',
            challenge_token: 'token',
          },
        },
        buildHostKeyConfirmMessageResult: 'confirm host key',
      }
    );

    const { terminalService } = harness;
    Object.defineProperty(globalThis, 'window', {
      value: {
        dispatchEvent: vi.fn(() => true),
        addEventListener: vi.fn(),
      },
      configurable: true,
      writable: true,
    });

    await expect(terminalService.createTerminalSession(createConnection('conn-host-key-fallback')))
      .rejects.toThrow('确认对话框不可用');
  });
});

describe('terminalService resize dedupe', () => {
  it('deduplicates identical terminal resize requests per session', async () => {
    const harness = await createTerminalServiceHarness(async (command) => {
      if (command === 'resize_terminal') return undefined;
      if (command === 'connect') return 'session-auto';
      if (command === 'start_terminal') return true;
      return undefined;
    });

    const { terminalService, invokeCalls } = harness;

    await terminalService.sendTerminalResize('session-a', 120, 40);
    await terminalService.sendTerminalResize('session-a', 120, 40);
    await terminalService.sendTerminalResize('session-a', 121, 40);

    const resizeCalls = invokeCalls.filter((call) => call.command === 'resize_terminal');
    expect(resizeCalls).toHaveLength(2);
    expect(resizeCalls[0]?.args).toEqual({ sessionId: 'session-a', width: 120, height: 40 });
    expect(resizeCalls[1]?.args).toEqual({ sessionId: 'session-a', width: 121, height: 40 });
  });

  it('clears resize dedupe state when terminal closes', async () => {
    const harness = await createTerminalServiceHarness(async (command) => {
      if (command === 'resize_terminal') return undefined;
      if (command === 'close_terminal') return undefined;
      if (command === 'connect') return 'session-auto';
      if (command === 'start_terminal') return true;
      return undefined;
    });

    const { terminalService, store, invokeCalls, flush, resetStores } = harness;
    resetStores();

    const term = new MockTerminal();
    const connection = createConnection('conn-resize', 'resize-demo');
    store.activeTerminals.set([
      {
        sessionId: 'session-a',
        connection,
        terminal: term as any,
        fitAddon: null as any,
        searchAddon: null as any,
      },
    ]);

    await terminalService.sendTerminalResize('session-a', 120, 40);
    await terminalService.closeTerminal('session-a');
    await flush();
    await terminalService.sendTerminalResize('session-a', 120, 40);

    const resizeCalls = invokeCalls.filter((call) => call.command === 'resize_terminal');
    expect(resizeCalls).toHaveLength(2);
  });
});

describe('terminalService suppression cleanup', () => {
  it('does not leave reconnect suppression residue after terminal close', async () => {
    const harness = await createTerminalServiceHarness(async (command) => {
      if (command === 'connect') return 'session-reconnected';
      if (command === 'start_terminal') return true;
      return undefined;
    });

    const { terminalService, store, invokeCalls, flush, resetStores } = harness;
    resetStores();

    const term = new MockTerminal();
    const connection = createConnection('conn-old', 'old');

    store.activeTerminals.set([
      {
        sessionId: 'session-old',
        connection,
        terminal: term as any,
        fitAddon: null as any,
        searchAddon: null as any,
      },
    ]);

    await terminalService.closeTerminal('session-old');
    await flush();

    // Re-add a visible terminal entry to verify reconnect is no longer blocked by stale suppression.
    store.activeTerminals.set([
      {
        sessionId: 'session-old',
        connection,
        terminal: term as any,
        fitAddon: null as any,
        searchAddon: null as any,
      },
    ]);

    const reconnected = await terminalService.reconnectTerminal('session-old');
    expect(reconnected).toBe(true);
    expect(invokeCalls.filter((call) => call.command === 'connect')).toHaveLength(1);
  });
});
