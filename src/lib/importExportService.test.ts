import { beforeEach, describe, expect, it, vi } from 'vitest';

const mocks = vi.hoisted(() => ({
  save: vi.fn(),
  open: vi.fn(),
  writeTextFile: vi.fn(),
  readTextFile: vi.fn(),
  invoke: vi.fn(),
  loadConnections: vi.fn(),
  successSet: vi.fn(),
  errorSet: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  save: mocks.save,
  open: mocks.open,
}));

vi.mock('./localFsService', () => ({
  localFsService: {
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

describe('importExportService exportConnections', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.save.mockResolvedValue('/tmp/connections.json');
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

    expect(mocks.writeTextFile).toHaveBeenCalledTimes(1);
    const [, payload] = mocks.writeTextFile.mock.calls[0] as [string, string];
    const exported = JSON.parse(payload);

    expect(exported[0].auth_method.Password.password).toBe('');
    expect(
      exported[0].proxy_type.JumpHost.auth_method.Password.password
    ).toBe('');
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

    expect(mocks.writeTextFile).toHaveBeenCalledTimes(1);
    const [, payload] = mocks.writeTextFile.mock.calls[0] as [string, string];
    const exported = JSON.parse(payload);

    expect(exported[0].auth_method.Password.password).toBe('');
    expect(exported[0].proxy_type.Socks5.password).toBeNull();
  });
});

describe('importExportService importConnections', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.open.mockResolvedValue('/tmp/import-connections.json');
    mocks.loadConnections.mockResolvedValue(undefined);
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
});
