import { get } from 'svelte/store';
import { describe, expect, it } from 'vitest';

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

describe('terminalService reconnect/manual listener behavior', () => {
  it('detaches normal input listener after session-closed and does not send input to the closed session', async () => {
    const connectResponses: Array<string | Error> = ['session-new-1', 'session-new-2'];
    const harness = await createTerminalServiceHarness(async (command) => {
      if (command === 'connect') {
        const next = connectResponses.shift();
        if (next instanceof Error) throw next;
        return next;
      }
      if (command === 'start_terminal') return true;
      return undefined;
    });

    const { terminalService, store, invokeCalls, emitEvent, flush, resetStores } = harness;
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

    const firstReconnect = await terminalService.reconnectTerminal('session-old');
    expect(firstReconnect).toBe(true);

    const activeAfterReconnect = get(store.activeTerminals).map((item) => item.sessionId);
    expect(activeAfterReconnect).toEqual(['session-new-1']);

    emitEvent('session-closed-session-new-1', { reason: 'connection_lost' });
    await flush();

    term.emitData('x');
    await flush();

    const sendsAfterNormalKey = invokeCalls.filter(
      (call) =>
        call.command === 'send_terminal_data' &&
        (call.args?.sessionId === 'session-new-1' || call.args?.sessionId === 'session-old')
    );
    expect(sendsAfterNormalKey).toHaveLength(0);

    term.emitData('R');
    await flush();
    await flush();

    const sendCallsAfterManualReconnect = invokeCalls.filter(
      (call) =>
        call.command === 'send_terminal_data' &&
        (call.args?.sessionId === 'session-new-1' || call.args?.sessionId === 'session-old')
    );
    expect(sendCallsAfterManualReconnect).toHaveLength(0);
    expect(invokeCalls.filter((call) => call.command === 'connect')).toHaveLength(2);
  });

  it('re-arms manual reconnect key after a failed reconnect so pressing R can retry', async () => {
    const harness = await createTerminalServiceHarness(async (command) => {
      if (command === 'connect') {
        throw new Error('simulated connect failure');
      }
      if (command === 'start_terminal') return true;
      return undefined;
    });

    const { terminalService, store, invokeCalls, emitEvent, flush, resetStores } = harness;
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

    await terminalService.setupTerminalListeners('session-old', term as any);
    emitEvent('session-closed-session-old', { reason: 'connection_lost' });
    await flush();

    term.emitData('R');
    await flush();
    await flush();
    await new Promise((resolve) => setTimeout(resolve, 0));

    term.emitData('R');
    await flush();
    await flush();
    await new Promise((resolve) => setTimeout(resolve, 0));

    expect(invokeCalls.filter((call) => call.command === 'connect')).toHaveLength(2);
    expect(
      term.writes.filter((line) => line.includes('Press R to reconnect')).length
    ).toBeGreaterThanOrEqual(2);
  });

  it('migrates resize dedupe state after a successful reconnect migration', async () => {
    const harness = await createTerminalServiceHarness(async (command) => {
      if (command === 'connect') return 'session-new-1';
      if (command === 'start_terminal') return true;
      if (command === 'resize_terminal') return undefined;
      return undefined;
    });

    const { terminalService, store, invokeCalls, resetStores } = harness;
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

    await terminalService.sendTerminalResize('session-old', 120, 40);
    await expect(terminalService.reconnectTerminal('session-old')).resolves.toBe(true);
    await terminalService.sendTerminalResize('session-new-1', 120, 40);
    await terminalService.sendTerminalResize('session-new-1', 121, 40);

    const resizeCalls = invokeCalls.filter((call) => call.command === 'resize_terminal');
    expect(resizeCalls).toHaveLength(2);
    expect(resizeCalls[0]?.args).toEqual({ sessionId: 'session-old', width: 120, height: 40 });
    expect(resizeCalls[1]?.args).toEqual({ sessionId: 'session-new-1', width: 121, height: 40 });
  });

  it('clears old reconnect state after a successful reconnect migration', async () => {
    const harness = await createTerminalServiceHarness(async (command) => {
      if (command === 'connect') return 'session-new-1';
      if (command === 'start_terminal') return true;
      return undefined;
    });

    const reconnectState = await import('./terminalReconnectState');
    const { terminalService, store, resetStores } = harness;
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

    reconnectState.rememberReconnectConfig('session-old', { host: 'old-host' });
    reconnectState.setReconnectInFlight('session-new-1', Promise.resolve(false));

    await expect(terminalService.reconnectTerminal('session-old')).resolves.toBe(true);

    expect(reconnectState.getReconnectConfig('session-old')).toBeUndefined();
    expect(reconnectState.getReconnectInFlight('session-old')).toBeUndefined();
    expect(reconnectState.getReconnectInFlight('session-new-1')).toBeUndefined();
  });
});
