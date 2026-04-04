import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { __sftpServiceTestHooks } from './sftpService';

describe('sftpService path helpers', () => {
  it('normalizes remote paths before building child paths', () => {
    expect(__sftpServiceTestHooks.buildRemoteChildPath('/tmp/demo/', 'file.txt')).toBe('/tmp/demo/file.txt');
    expect(__sftpServiceTestHooks.buildRemoteChildPath('demo/', 'file.txt')).toBe('demo/file.txt');
    expect(__sftpServiceTestHooks.buildRemoteChildPath('/', 'file.txt')).toBe('/file.txt');
  });

  it('collapses duplicate slashes in parent paths', () => {
    expect(__sftpServiceTestHooks.normalizeRemotePath('/tmp//demo///')).toBe('/tmp/demo');
  });

  it('preserves significant spaces in remote parent paths', () => {
    expect(__sftpServiceTestHooks.normalizeRemotePath('  spaced dir  /')).toBe('  spaced dir  ');
    expect(__sftpServiceTestHooks.buildRemoteChildPath('  spaced dir  /', 'file.txt')).toBe('  spaced dir  /file.txt');
  });
});
