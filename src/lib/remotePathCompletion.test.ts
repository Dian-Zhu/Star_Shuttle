import { describe, expect, it } from 'vitest';
import {
  buildRemotePathCompletion,
  getRemotePathCompletionContext,
} from './remotePathCompletion';

describe('remote path completion', () => {
  it('completes a name relative to the current directory', () => {
    expect(getRemotePathCompletionContext('doc')).toEqual({
      directoryInput: '.',
      typedPrefix: 'doc',
      valuePrefix: '',
    });
    expect(buildRemotePathCompletion('', 'documents')).toBe('documents/');
  });

  it('preserves absolute and home-relative prefixes', () => {
    expect(getRemotePathCompletionContext('/usr/lo')).toEqual({
      directoryInput: '/usr',
      typedPrefix: 'lo',
      valuePrefix: '/usr/',
    });
    expect(getRemotePathCompletionContext('~/Do')).toEqual({
      directoryInput: '~',
      typedPrefix: 'Do',
      valuePrefix: '~/',
    });
  });

  it('lists children when the input ends with a slash', () => {
    expect(getRemotePathCompletionContext('/')).toEqual({
      directoryInput: '/',
      typedPrefix: '',
      valuePrefix: '/',
    });
    expect(getRemotePathCompletionContext('var/log/')).toEqual({
      directoryInput: 'var/log',
      typedPrefix: '',
      valuePrefix: 'var/log/',
    });
  });
});
