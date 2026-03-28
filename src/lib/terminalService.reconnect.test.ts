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
});
