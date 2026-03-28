import { beforeEach, describe, expect, it, vi } from 'vitest';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

import { buildHostKeyConfirmMessage, parseHostKeyPrompt, saveHostKeyPrompt } from './hostKeyPrompt';

describe('hostKeyPrompt', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('parses host key prompt payload from marker error', () => {
    const parsed = parseHostKeyPrompt(
      'Error: HOST_KEY_UNKNOWN|{"host":"example.com","port":22,"fingerprint":"fp","key_type":"ssh-ed25519","key_base64":"AAAA","challenge_token":"tok"}'
    );
    expect(parsed).not.toBeNull();
    expect(parsed?.type).toBe('unknown');
    expect(parsed?.payload.host).toBe('example.com');
    expect(parsed?.payload.challenge_token).toBe('tok');
  });

  it('returns null when payload is incomplete', () => {
    const parsed = parseHostKeyPrompt(
      'HOST_KEY_UNKNOWN|{"host":"example.com","port":22,"fingerprint":"fp","key_type":"ssh-ed25519","key_base64":"AAAA"}'
    );
    expect(parsed).toBeNull();
  });

  it('builds confirmation message using normalized fields', () => {
    const parsed = parseHostKeyPrompt(
      'HOST_KEY_MISMATCH|{"host":"example.com","port":2222,"fingerprint":"fp","key_type":"rsa","key_base64":"BBBB","challenge_token":"tok","reason":"changed"}'
    );
    expect(parsed).not.toBeNull();
    const message = buildHostKeyConfirmMessage(parsed!);
    expect(message).toContain('example.com:2222');
    expect(message).toContain('Fingerprint: fp');
    expect(message).toContain('Reason: changed');
  });

  it('saves host key prompt with mapped invoke payload', async () => {
    const parsed = parseHostKeyPrompt(
      'HOST_KEY_MISMATCH|{"host":"example.com","port":22,"fingerprint":"fp","key_type":"ssh-ed25519","key_base64":"AAAA","challenge_token":"tok"}'
    );
    expect(parsed).not.toBeNull();
    await saveHostKeyPrompt(parsed!);
    expect(invokeMock).toHaveBeenCalledWith('known_hosts_save_host_key', {
      challengeToken: 'tok',
      host: 'example.com',
      port: 22,
      keyType: 'ssh-ed25519',
      keyBase64: 'AAAA',
      replace: true,
    });
  });
});
