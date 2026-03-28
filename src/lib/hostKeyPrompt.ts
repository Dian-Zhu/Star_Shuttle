import { invoke } from '@tauri-apps/api/core';

export type HostKeyPromptType = 'unknown' | 'mismatch' | 'unavailable';

export interface HostKeyPromptPayload {
  host: string;
  port: number;
  fingerprint: string;
  key_type: string;
  key_base64: string;
  challenge_token: string;
  reason?: string;
}

export interface HostKeyPrompt {
  type: HostKeyPromptType;
  payload: HostKeyPromptPayload;
}

const HOST_KEY_MARKERS: Array<{ marker: string; type: HostKeyPromptType }> = [
  { marker: 'HOST_KEY_UNKNOWN|', type: 'unknown' },
  { marker: 'HOST_KEY_MISMATCH|', type: 'mismatch' },
  { marker: 'HOST_KEY_UNAVAILABLE|', type: 'unavailable' },
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object';
}

function asString(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

function asNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function normalizePayload(value: unknown): HostKeyPromptPayload | null {
  if (!isRecord(value)) return null;
  const host = asString(value.host);
  const port = asNumber(value.port);
  const fingerprint = asString(value.fingerprint);
  const keyType = asString(value.key_type);
  const keyBase64 = asString(value.key_base64);
  const challengeToken = asString(value.challenge_token);
  const reason = asString(value.reason) ?? undefined;
  if (!host || !fingerprint || !keyType || !keyBase64 || !challengeToken || port === null || port <= 0) {
    return null;
  }
  return {
    host,
    port,
    fingerprint,
    key_type: keyType,
    key_base64: keyBase64,
    challenge_token: challengeToken,
    reason,
  };
}

export function parseHostKeyPrompt(error: unknown): HostKeyPrompt | null {
  const str = String(error);
  for (const { marker, type } of HOST_KEY_MARKERS) {
    const idx = str.lastIndexOf(marker);
    if (idx === -1) continue;
    const jsonPart = str.slice(idx + marker.length).trim();
    try {
      const payload = normalizePayload(JSON.parse(jsonPart));
      if (!payload) return null;
      return { type, payload };
    } catch {
      return null;
    }
  }
  return null;
}

export function getHostKeyPromptTitle(type: HostKeyPromptType): string {
  if (type === 'mismatch') return '主机密钥已变更';
  if (type === 'unavailable') return '无法校验主机密钥';
  return '未知的主机密钥';
}

export function getHostKeyPromptHint(type: HostKeyPromptType): string {
  if (type === 'mismatch') return '这可能是中间人攻击或服务器重装导致。请谨慎确认后再信任。';
  if (type === 'unavailable') return '应用信任库当前不可用，无法自动校验服务器身份。';
  return '首次连接该服务器，请确认指纹后再继续。';
}

export function buildHostKeyConfirmMessage(prompt: HostKeyPrompt): string {
  const { type, payload } = prompt;
  const lines = [
    `${getHostKeyPromptTitle(type)}: ${payload.host}:${payload.port}`,
    `Key Type: ${payload.key_type}`,
    `Fingerprint: ${payload.fingerprint}`,
    payload.reason ? `Reason: ${payload.reason}` : null,
    '',
    type === 'unknown'
      ? '是否信任该主机并保存到应用信任库？'
      : type === 'mismatch'
        ? '这可能是中间人攻击或服务器重装导致。仍要信任并替换应用信任库记录吗？'
        : '应用信任库当前不可用。仍要信任并保存吗？',
  ].filter(Boolean);
  return lines.join('\n');
}

export async function saveHostKeyPrompt(prompt: HostKeyPrompt): Promise<void> {
  const { type, payload } = prompt;
  await invoke('known_hosts_save_host_key', {
    challengeToken: payload.challenge_token,
    host: payload.host,
    port: payload.port,
    keyType: payload.key_type,
    keyBase64: payload.key_base64,
    replace: type === 'mismatch',
  });
}
