import { describe, expect, it } from 'vitest';

import { summarizeAuditedCommand } from './commandAuditSanitizer';

describe('commandAuditSanitizer', () => {
  it('redacts variable and long option secrets', () => {
    const summary = summarizeAuditedCommand(
      'DB_PASSWORD=secret curl --token abc123 --password "p@ss"',
      'LOW'
    );
    expect(summary).toContain('DB_PASSWORD=<redacted>');
    expect(summary).toContain('--token <redacted>');
    expect(summary).toContain('--password <redacted>');
  });

  it('redacts short secret flags in both compact and spaced forms', () => {
    const summary = summarizeAuditedCommand(
      'sshpass -psecret ssh user@host && sshpass -p "new secret" ssh user@host',
      'LOW'
    );
    expect(summary).toContain('sshpass -p<redacted>');
    expect(summary).toContain('-p <redacted>');
    expect(summary).not.toContain('-psecret');
  });

  it('redacts url credentials and bearer tokens', () => {
    const summary = summarizeAuditedCommand(
      'curl -H "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.sig" https://user:pass@example.com/path',
      'LOW'
    );
    expect(summary).toContain('Bearer <redacted>');
    expect(summary).toContain('https://user:<redacted>@example.com/path');
  });

  it('redacts standalone jwt strings', () => {
    const summary = summarizeAuditedCommand(
      'echo eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.sig',
      'LOW'
    );
    expect(summary).toContain('<redacted-jwt>');
  });

  it('uses compact summary format for high risk commands', () => {
    const summary = summarizeAuditedCommand(
      'rm -rf /tmp/somewhere && echo done',
      'HIGH'
    );
    expect(summary).toBe('[HIGH] rm -rf /tmp/somewhere && [summary]');
  });
});
