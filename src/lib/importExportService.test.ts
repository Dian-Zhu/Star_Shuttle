import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  pickFileForWrite: vi.fn(),
  pickFileForRead: vi.fn(),
  writeTextFile: vi.fn(),
  readTextFile: vi.fn(),
  invoke: vi.fn(),
  loadConnections: vi.fn(),
  successSet: vi.fn(),
  errorSet: vi.fn(),
}));

vi.mock('./localFsService', () => ({
  localFsService: {
    pickFileForWrite: mocks.pickFileForWrite,
    pickFileForRead: mocks.pickFileForRead,
    writeTextFile: mocks.writeTextFile,
    readTextFile: mocks.readTextFile,
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: mocks.invoke,
}));

vi.mock('./connectionService', () => ({
  loadConnections: mocks.loadConnections,
}));

vi.mock('./store', () => ({
  successMessage: { set: mocks.successSet },
  errorMessage: { set: mocks.errorSet },
}));

function createBaseConnection(overrides: Record<string, unknown> = {}) {
  return {
    id: 'conn-base',
    name: 'Base',
    protocol: 'Ssh',
    host: '10.0.0.1',
    port: 22,
    username: 'root',
    auth_method: { KeyboardInteractive: {} },
    description: null,
    tags: [],
    created_at: '2026-03-26T00:00:00.000Z',
    updated_at: '2026-03-26T00:00:00.000Z',
    group_id: null,
    proxy_type: 'None',
    ...overrides,
  };
}

type RoundTripCase = {
  name: string;
  source: Record<string, unknown>;
  assertExported: (item: any) => void;
  assertImported: (item: any) => void;
};

function assertNoEmptySavedSecrets(config: any) {
  const auth = config?.auth_method ?? {};
  if (auth.Password && auth.Password.save_password && !String(auth.Password.password ?? '')) {
    throw new Error('save_password must be false when password is empty');
  }
  if (
    auth.PrivateKey &&
    auth.PrivateKey.save_passphrase &&
    !String(auth.PrivateKey.passphrase ?? '')
  ) {
    throw new Error('save_passphrase must be false when private key passphrase is empty');
  }
  if (
    auth.Certificate &&
    auth.Certificate.save_passphrase &&
    !String(auth.Certificate.passphrase ?? '')
  ) {
    throw new Error('save_passphrase must be false when certificate passphrase is empty');
  }

  const jumpAuth = config?.proxy_type?.JumpHost?.auth_method;
  if (!jumpAuth) return;
  if (jumpAuth.Password && jumpAuth.Password.save_password && !String(jumpAuth.Password.password ?? '')) {
    throw new Error('jump host save_password must be false when password is empty');
  }
  if (
    jumpAuth.PrivateKey &&
    jumpAuth.PrivateKey.save_passphrase &&
    !String(jumpAuth.PrivateKey.passphrase ?? '')
  ) {
    throw new Error('jump host private key save_passphrase must be false when passphrase is empty');
  }
  if (
    jumpAuth.Certificate &&
    jumpAuth.Certificate.save_passphrase &&
    !String(jumpAuth.Certificate.passphrase ?? '')
  ) {
    throw new Error('jump host certificate save_passphrase must be false when passphrase is empty');
  }
}

describe('importExportService exportConnections', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.pickFileForWrite.mockResolvedValue({
      path: '/tmp/connections.json',
      accessToken: 'access-token',
      size: 0,
    });
  });

  it('sanitizes auth and proxy secrets in non-sensitive export', async () => {
    const source = {
      id: 'conn-1',
      name: 'Prod',
      protocol: 'Ssh',
      host: '10.0.0.1',
      port: 22,
      username: 'root',
      auth_method: {
        Password: { password: 'secret-password', save_password: true },
      },
      description: null,
      tags: [],
      created_at: '2026-03-26T00:00:00.000Z',
      updated_at: '2026-03-26T00:00:00.000Z',
      group_id: null,
      proxy_type: {
        JumpHost: {
          host: 'jump.example.com',
          port: 22,
          username: 'jump-user',
          auth_method: {
            Password: {
              password: 'jump-secret',
              save_password: true,
            },
          },
        },
      },
    };

    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_all_connection_configs') return [source];
      throw new Error(`unexpected command: ${command}`);
    });

    const { exportConnections } = await import('./importExportService');
    await exportConnections();

    expect(mocks.pickFileForWrite).toHaveBeenCalledWith('connections.json', [
      { name: 'JSON', extensions: ['json'] },
    ]);
    expect(mocks.writeTextFile).toHaveBeenCalledTimes(1);
    const [, payload, token] = mocks.writeTextFile.mock.calls[0] as [string, string, string];
    const exported = JSON.parse(payload);
    expect(token).toBe('access-token');

    expect(exported[0].auth_method.Password.password).toBe('');
    expect(exported[0].auth_method.Password.save_password).toBe(false);
    expect(
      exported[0].proxy_type.JumpHost.auth_method.Password.password
    ).toBe('');
    expect(exported[0].proxy_type.JumpHost.auth_method.Password.save_password).toBe(false);
  });

  it('still exports sanitized data when includeSensitive is requested', async () => {
    const source = {
      id: 'conn-2',
      name: 'Stage',
      protocol: 'Ssh',
      host: '10.0.0.2',
      port: 22,
      username: 'root',
      auth_method: {
        Password: { password: 'keep-me', save_password: true },
      },
      description: null,
      tags: [],
      created_at: '2026-03-26T00:00:00.000Z',
      updated_at: '2026-03-26T00:00:00.000Z',
      group_id: null,
      proxy_type: {
        Socks5: {
          host: 'proxy',
          port: 1080,
          username: 'u',
          password: 'proxy-pass',
        },
      },
    };

    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'get_all_connection_configs') return [source];
      throw new Error(`unexpected command: ${command}`);
    });

    const { exportConnections } = await import('./importExportService');
    await exportConnections({ includeSensitive: true });

    expect(mocks.pickFileForWrite).toHaveBeenCalledWith('connections.json', [
      { name: 'JSON', extensions: ['json'] },
    ]);
    expect(mocks.writeTextFile).toHaveBeenCalledTimes(1);
    const [, payload, token] = mocks.writeTextFile.mock.calls[0] as [string, string, string];
    const exported = JSON.parse(payload);
    expect(token).toBe('access-token');

    expect(exported[0].auth_method.Password.password).toBe('');
    expect(exported[0].auth_method.Password.save_password).toBe(false);
    expect(exported[0].proxy_type.Socks5.password).toBeNull();
  });
});

describe('importExportService importConnections', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.loadConnections.mockResolvedValue(undefined);
    mocks.pickFileForRead.mockResolvedValue({
      path: '/tmp/import-connections.json',
      accessToken: 'read-token',
      size: 12,
    });
  });

  it('regenerates ids on import to avoid overwriting existing connections', async () => {
    const originalId = 'existing-connection-id';
    const imported = [
      {
        id: originalId,
        name: 'Imported',
        protocol: 'Ssh',
        host: '10.0.0.9',
        port: 22,
        username: 'root',
        auth_method: {
          Password: { password: '', save_password: false },
        },
        description: null,
        tags: [],
        created_at: '2026-03-26T00:00:00.000Z',
        updated_at: '2026-03-26T00:00:00.000Z',
        group_id: null,
        proxy_type: 'None',
      },
    ];

    mocks.readTextFile.mockResolvedValue(JSON.stringify(imported));

    const generatedId = '11111111-1111-4111-8111-111111111111';
    const randomUuidSpy = vi
      .spyOn(globalThis.crypto, 'randomUUID')
      .mockReturnValue(generatedId);

    mocks.invoke.mockImplementation(async (command: string, payload?: any) => {
      if (command === 'save_connection_config') {
        expect(payload.config.id).toBe(generatedId);
        expect(payload.config.id).not.toBe(originalId);
        return;
      }
      throw new Error(`unexpected command: ${command}`);
    });

    const { importConnections } = await import('./importExportService');
    await importConnections();

    expect(mocks.pickFileForRead).toHaveBeenCalledWith([
      { name: 'JSON', extensions: ['json'] },
    ]);
    expect(mocks.readTextFile).toHaveBeenCalledWith('/tmp/import-connections.json', 'read-token');
    expect(mocks.invoke).toHaveBeenCalledWith(
      'save_connection_config',
      expect.objectContaining({
        config: expect.objectContaining({
          id: generatedId,
          name: 'Imported',
        }),
      }),
    );
    expect(mocks.loadConnections).toHaveBeenCalledTimes(1);
    expect(mocks.successSet).toHaveBeenCalledWith('成功导入 1 个连接');

    randomUuidSpy.mockRestore();
  });

  const roundTripCases: RoundTripCase[] = [
    {
      name: 'Password',
      source: createBaseConnection({
        id: 'conn-password',
        name: 'Password',
        auth_method: {
          Password: {
            password: 'secret',
            save_password: true,
          },
        },
      }),
      assertExported: (item: any) => {
        expect(item.auth_method.Password.password).toBe('');
        expect(item.auth_method.Password.save_password).toBe(false);
      },
      assertImported: (item: any) => {
        expect(item.auth_method.Password.password).toBe('');
        expect(item.auth_method.Password.save_password).toBe(false);
      },
    },
    {
      name: 'PrivateKey',
      source: createBaseConnection({
        id: 'conn-private-key',
        name: 'PrivateKey',
        auth_method: {
          PrivateKey: {
            key_path: '/tmp/id_ed25519',
            passphrase: 'pk-secret',
            save_passphrase: true,
          },
        },
      }),
      assertExported: (item: any) => {
        expect(item.auth_method.PrivateKey.passphrase).toBeUndefined();
        expect(item.auth_method.PrivateKey.save_passphrase).toBe(false);
      },
      assertImported: (item: any) => {
        expect(item.auth_method.PrivateKey.passphrase).toBeUndefined();
        expect(item.auth_method.PrivateKey.save_passphrase).toBe(false);
      },
    },
    {
      name: 'Certificate',
      source: createBaseConnection({
        id: 'conn-cert',
        name: 'Certificate',
        auth_method: {
          Certificate: {
            certificate_path: '/tmp/client.crt',
            private_key_path: '/tmp/client.key',
            passphrase: 'cert-secret',
            save_passphrase: true,
          },
        },
      }),
      assertExported: (item: any) => {
        expect(item.auth_method.Certificate.passphrase).toBeUndefined();
        expect(item.auth_method.Certificate.save_passphrase).toBe(false);
      },
      assertImported: (item: any) => {
        expect(item.auth_method.Certificate.passphrase).toBeUndefined();
        expect(item.auth_method.Certificate.save_passphrase).toBe(false);
      },
    },
    {
      name: 'JumpHost',
      source: createBaseConnection({
        id: 'conn-jump',
        name: 'JumpHost',
        proxy_type: {
          JumpHost: {
            host: 'jump.example.com',
            port: 22,
            username: 'jump',
            auth_method: {
              Password: {
                password: 'jump-secret',
                save_password: true,
              },
            },
          },
        },
      }),
      assertExported: (item: any) => {
        expect(item.proxy_type.JumpHost.auth_method.Password.password).toBe('');
        expect(item.proxy_type.JumpHost.auth_method.Password.save_password).toBe(false);
      },
      assertImported: (item: any) => {
        expect(item.proxy_type.JumpHost.auth_method.Password.password).toBe('');
        expect(item.proxy_type.JumpHost.auth_method.Password.save_password).toBe(false);
      },
    },
  ];

  it.each(roundTripCases)(
    'round-trip keeps %s sanitized and import-safe',
    async ({ source, assertExported, assertImported }: RoundTripCase) => {
      mocks.pickFileForWrite.mockResolvedValue({
        path: '/tmp/roundtrip-export.json',
        accessToken: 'write-token',
        size: 0,
      });
      mocks.pickFileForRead.mockResolvedValue({
        path: '/tmp/roundtrip-import.json',
        accessToken: 'read-token',
        size: 0,
      });

      const savedConfigs: any[] = [];
      mocks.invoke.mockImplementation(async (command: string, payload?: any) => {
        if (command === 'get_all_connection_configs') return [source];
        if (command === 'save_connection_config') {
          assertNoEmptySavedSecrets(payload.config);
          savedConfigs.push(payload.config);
          return;
        }
        throw new Error(`unexpected command: ${command}`);
      });

      const { exportConnections, importConnections } = await import('./importExportService');
      await exportConnections();

      expect(mocks.writeTextFile).toHaveBeenCalledTimes(1);
      const [, exportedPayload] = mocks.writeTextFile.mock.calls[0] as [string, string, string];
      const exported = JSON.parse(exportedPayload);
      expect(exported).toHaveLength(1);
      assertExported(exported[0]);

      mocks.readTextFile.mockResolvedValue(exportedPayload);
      await importConnections();

      expect(savedConfigs).toHaveLength(1);
      assertImported(savedConfigs[0]);
    }
  );

  it('sanitizes stale save flags during import for legacy exported JSON', async () => {
    const staleImported = [
      createBaseConnection({
        id: 'legacy-1',
        name: 'LegacyPassword',
        auth_method: {
          Password: { password: '', save_password: true },
        },
      }),
      createBaseConnection({
        id: 'legacy-2',
        name: 'LegacyPrivateKey',
        auth_method: {
          PrivateKey: { key_path: '/tmp/id_ed25519', save_passphrase: true },
        },
      }),
      createBaseConnection({
        id: 'legacy-3',
        name: 'LegacyCertificate',
        auth_method: {
          Certificate: {
            certificate_path: '/tmp/client.crt',
            private_key_path: '/tmp/client.key',
            save_passphrase: true,
          },
        },
      }),
      createBaseConnection({
        id: 'legacy-4',
        name: 'LegacyJumpHost',
        proxy_type: {
          JumpHost: {
            host: 'jump.example.com',
            port: 22,
            username: 'jump',
            auth_method: {
              Password: { password: '', save_password: true },
            },
          },
        },
      }),
    ];
    mocks.readTextFile.mockResolvedValue(JSON.stringify(staleImported));

    const savedConfigs: any[] = [];
    mocks.invoke.mockImplementation(async (command: string, payload?: any) => {
      if (command === 'save_connection_config') {
        assertNoEmptySavedSecrets(payload.config);
        savedConfigs.push(payload.config);
        return;
      }
      throw new Error(`unexpected command: ${command}`);
    });

    const { importConnections } = await import('./importExportService');
    await importConnections();

    expect(savedConfigs).toHaveLength(4);
    expect(savedConfigs[0].auth_method.Password.save_password).toBe(false);
    expect(savedConfigs[1].auth_method.PrivateKey.save_passphrase).toBe(false);
    expect(savedConfigs[2].auth_method.Certificate.save_passphrase).toBe(false);
    expect(savedConfigs[3].proxy_type.JumpHost.auth_method.Password.save_password).toBe(false);
  });
});
