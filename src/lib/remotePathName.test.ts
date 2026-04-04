import { describe, expect, it } from 'vitest';

import { validateRemoteLeafName } from './remotePathName';

describe('remotePathName', () => {
  it('accepts a regular file name', () => {
    expect(validateRemoteLeafName('normal.txt')).toBeNull();
  });

  it('preserves leading and trailing spaces in otherwise valid names', () => {
    expect(validateRemoteLeafName('  normal.txt  ')).toBeNull();
    expect(validateRemoteLeafName(' . ')).toBeNull();
    expect(validateRemoteLeafName(' .. ')).toBeNull();
  });

  it('rejects empty and special names', () => {
    expect(validateRemoteLeafName('   ')).toBe('名称不能为空');
    expect(validateRemoteLeafName('.')).toBe('名称不能为 . 或 ..');
    expect(validateRemoteLeafName('..')).toBe('名称不能为 . 或 ..');
  });

  it('rejects path separators and control characters', () => {
    expect(validateRemoteLeafName('dir/file')).toBe('名称不能包含 /');
    expect(validateRemoteLeafName('bad\nname')).toBe('名称不能包含控制字符');
  });
});
