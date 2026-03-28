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
