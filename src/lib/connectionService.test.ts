import { describe, expect, it, vi } from 'vitest';

vi.mock('./store', () => ({
  connectionGroups: {
    subscribe: vi.fn((run: (value: unknown[]) => void) => {
      run([]);
      return () => {};
    }),
    update: vi.fn(),
    set: vi.fn(),
  },
  connections: {
    subscribe: vi.fn((run: (value: unknown[]) => void) => {
      run([]);
      return () => {};
    }),
    set: vi.fn(),
    update: vi.fn(),
  },
  loading: {
    subscribe: vi.fn((run: (value: boolean) => void) => {
      run(false);
      return () => {};
    }),
    set: vi.fn(),
  },
  getGroupIdByPath: vi.fn(() => null),
  clearErrorMessage: vi.fn(),
  showErrorMessage: vi.fn(),
  showSuccessMessage: vi.fn(),
}));

vi.mock('./devLogger', () => ({
  devWarn: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { __connectionServiceTestHooks } from './connectionService';
import type { Connection } from './store';

const {
  createBackendConfig,
  normalizeConnectionsPayload,
  toBackendConnectionConfig,
  parseBackendAuthMethod,
  parseBackendProxyType,
} =
  __connectionServiceTestHooks;

function backendConnection(
  overrides: Partial<Record<string, unknown>> = {}
): Record<string, unknown> {
  return {
    id: 'conn-1',
    name: 'demo',
    protocol: 'Ssh',
    host: '127.0.0.1',
    port: 22,
    username: 'root',
    auth_method: {
      Password: {
        password: 'secret',
        save_password: true,
      },
    },
    description: null,
    tags: ['default'],
    created_at: '2025-01-01T00:00:00.000Z',
    updated_at: '2025-01-01T00:00:00.000Z',
    group_id: null,
    local_forwards: [],
    remote_forwards: [],
    proxy_type: 'None',
    socks_proxy_port: null,
    auto_reconnect: true,
    ...overrides,
  };
}

function connection(overrides: Partial<Connection> = {}): Connection {
  return {
    id: 'conn-1',
    name: 'demo',
    protocol: 'Ssh',
    host: '127.0.0.1',
    port: 22,
    username: 'root',
    auth_method: {
      Password: {
        password: 'secret',
        save_password: true,
      },
    },
    description: null,
    tags: ['default'],
    created_at: '2025-01-01T00:00:00.000Z',
    updated_at: '2025-01-01T00:00:00.000Z',
    group_id: null,
    local_forwards: [],
    remote_forwards: [],
    proxy_type: 'None',
    socks_proxy_port: null,
    auto_reconnect: true,
    ...overrides,
  };
}

describe('connectionService normalization contract', () => {
  it('keeps valid backend connections compatible', () => {
    const payload = [backendConnection()];
    const normalized = normalizeConnectionsPayload(payload);
    expect(normalized).toHaveLength(1);
    expect(normalized[0].id).toBe('conn-1');
    expect(normalized[0].proxy_type).toBe('None');
    expect('Password' in normalized[0].auth_method).toBe(true);
  });

  it('drops backend entries with unknown auth_method variant instead of silently downgrading', () => {
    const payload = [
      backendConnection(),
      backendConnection({
        id: 'conn-2',
        auth_method: { UnknownAuth: { foo: 'bar' } },
      }),
    ];
    const normalized = normalizeConnectionsPayload(payload);
    expect(normalized).toHaveLength(1);
    expect(normalized[0].id).toBe('conn-1');
  });

  it('drops backend entries with unknown proxy_type variant instead of silently downgrading', () => {
    const payload = [
      backendConnection(),
      backendConnection({
        id: 'conn-2',
        proxy_type: { UnknownProxy: { foo: 'bar' } },
      }),
    ];
    const normalized = normalizeConnectionsPayload(payload);
    expect(normalized).toHaveLength(1);
    expect(normalized[0].id).toBe('conn-1');
  });

  it('fails fast when saving connection with invalid auth_method shape', () => {
    const bad = connection({ auth_method: {} as Connection['auth_method'] });
    expect(() => toBackendConnectionConfig(bad)).toThrow(/Invalid auth_method/);
  });

  it('fails fast when saving connection with invalid proxy_type shape', () => {
    const bad = connection({ proxy_type: { UnknownProxy: {} } as Connection['proxy_type'] });
    expect(() => toBackendConnectionConfig(bad)).toThrow(/Invalid proxy_type/);
  });

  it('parses known auth and proxy variants', () => {
    const auth = parseBackendAuthMethod({
      PrivateKey: {
        key_path: '/tmp/id_rsa',
        save_passphrase: false,
      },
    });
    expect(auth.ok).toBe(true);

    const proxy = parseBackendProxyType({
      Socks5: {
        host: '127.0.0.1',
        port: 1080,
        username: null,
        password: null,
      },
    });
    expect(proxy.ok).toBe(true);
  });

  it('preserves stored proxy password on edit when the password field is untouched', async () => {
    const created = await createBackendConfig({
      id: 'conn-1',
      name: 'demo',
      protocol: 'Ssh',
      host: '127.0.0.1',
      port: 22,
      username: 'root',
      authMethod: 'password',
      password: 'secret',
      savePassword: false,
      proxyType: 'socks5',
      proxyHost: 'proxy.example.com',
      proxyPort: 1080,
      proxyUsername: 'proxy-user',
      proxyPassword: '',
      proxyHasStoredPassword: true,
      proxyPasswordDirty: false,
      clearStoredProxyPassword: false,
      tags: [],
      local_forwards: [],
      remote_forwards: [],
      autoReconnect: false,
    });

    expect(created.proxy_type).toEqual({
      Socks5: {
        host: 'proxy.example.com',
        port: 1080,
        username: 'proxy-user',
        password: null,
        has_password: true,
      },
    });
  });

  it('clears stored proxy password on edit when explicitly requested', async () => {
    const created = await createBackendConfig({
      id: 'conn-1',
      name: 'demo',
      protocol: 'Ssh',
      host: '127.0.0.1',
      port: 22,
      username: 'root',
      authMethod: 'password',
      password: 'secret',
      savePassword: false,
      proxyType: 'http',
      proxyHost: 'proxy.example.com',
      proxyPort: 8080,
      proxyUsername: 'proxy-user',
      proxyPassword: '',
      proxyHasStoredPassword: false,
      proxyPasswordDirty: true,
      clearStoredProxyPassword: true,
      tags: [],
      local_forwards: [],
      remote_forwards: [],
      autoReconnect: false,
    });

    expect(created.proxy_type).toEqual({
      Http: {
        host: 'proxy.example.com',
        port: 8080,
        username: 'proxy-user',
        password: null,
        has_password: false,
      },
    });
  });
});
