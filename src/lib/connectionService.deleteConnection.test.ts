import { beforeEach, describe, expect, it, vi } from 'vitest';

const mockedStore = vi.hoisted(() => {
  const connectionsStore = {
    __value: [] as Array<Record<string, unknown>>,
    set: vi.fn((value: Array<Record<string, unknown>>) => {
      connectionsStore.__value = value;
    }),
    update: vi.fn((updater: (items: Array<Record<string, unknown>>) => Array<Record<string, unknown>>) => {
      connectionsStore.__value = updater(connectionsStore.__value);
      return connectionsStore.__value;
    }),
  };

  return {
    connectionsStore,
    showErrorMessage: vi.fn(),
    showSuccessMessage: vi.fn(),
  };
});

vi.mock('./store', () => ({
  connectionGroups: {
    update: vi.fn(),
    set: vi.fn(),
  },
  connections: mockedStore.connectionsStore,
  loading: {
    set: vi.fn(),
  },
  getGroupIdByPath: vi.fn(() => null),
  clearErrorMessage: vi.fn(),
  showErrorMessage: mockedStore.showErrorMessage,
  showSuccessMessage: mockedStore.showSuccessMessage,
}));

vi.mock('./devLogger', () => ({
  devWarn: vi.fn(),
}));

vi.mock('svelte/store', () => ({
  get: (store: { __value: unknown }) => store.__value,
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';
import { deleteConnection } from './connectionService';

describe('deleteConnection safeguards', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedStore.connectionsStore.__value = [{ id: 'conn-1', name: 'demo' }];
  });

  it('blocks deletion when active sessions exist', async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'get_all_sessions') {
        return [{ id: 'session-1', connection_id: 'conn-1', status: 'Connected' }];
      }
      throw new Error(`unexpected invoke: ${command}`);
    });

    await deleteConnection('conn-1');

    expect(invoke).toHaveBeenCalledWith('get_all_sessions');
    expect(invoke).not.toHaveBeenCalledWith('delete_connection_config', { connectionId: 'conn-1' });
    expect(mockedStore.connectionsStore.__value).toHaveLength(1);
    expect(mockedStore.showErrorMessage).toHaveBeenCalledWith(
      '该连接仍有活动会话，请先断开会话后再删除。',
      5000
    );
  });

  it('rolls back optimistic removal when backend rejects deletion', async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === 'get_all_sessions') {
        return [];
      }
      if (command === 'delete_connection_config') {
        throw new Error('Cannot delete connection config while sessions are still active');
      }
      throw new Error(`unexpected invoke: ${command}`);
    });

    await deleteConnection('conn-1');

    expect(invoke).toHaveBeenCalledWith('delete_connection_config', { connectionId: 'conn-1' });
    expect(mockedStore.connectionsStore.__value).toHaveLength(1);
    expect(mockedStore.showErrorMessage).toHaveBeenCalledWith(
      '该连接仍有活动会话，请先断开会话后再删除。',
      5000
    );
  });
});
