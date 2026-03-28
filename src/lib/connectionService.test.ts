import { describe, expect, it, vi } from 'vitest';

vi.mock('./store', () => ({
  connectionGroups: {
    update: vi.fn(),
    set: vi.fn(),
  },
  connections: {
    set: vi.fn(),
    update: vi.fn(),
  },
  loading: {
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

const { normalizeConnectionsPayload, toBackendConnectionConfig, parseBackendAuthMethod, parseBackendProxyType } =
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
});
