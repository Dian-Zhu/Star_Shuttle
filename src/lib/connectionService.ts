import { invoke } from '@tauri-apps/api/core';
import {
  connectionGroups,
  connections,
  loading,
  type Connection,
  getGroupIdByPath,
  clearErrorMessage,
  showErrorMessage,
  showSuccessMessage,
} from './store';
import { v4 as uuidv4 } from 'uuid';
import { get } from 'svelte/store';
import { devWarn } from './devLogger';

type ConnectionProtocol = 'Ssh' | 'Rdp' | 'Telnet';
type AuthMethodKind = 'password' | 'keyboardInteractive' | 'privateKey' | 'agent' | 'certificate';
type ProxyTypeKind = 'none' | 'socks5' | 'http' | 'jumpHost';

type BackendPasswordAuth = {
  Password: {
    password: string;
    save_password: boolean;
  };
};

type BackendKeyboardInteractiveAuth = {
  KeyboardInteractive: Record<string, never>;
};

type BackendPrivateKeyAuth = {
  PrivateKey: {
    key_path: string;
    passphrase?: string;
    save_passphrase: boolean;
  };
};

type BackendAgentAuth = {
  Agent: {
    agent_path?: string;
  };
};

type BackendCertificateAuth = {
  Certificate: {
    certificate_path: string;
    private_key_path: string;
    passphrase?: string;
    save_passphrase: boolean;
  };
};

type BackendAuthMethod =
  | BackendPasswordAuth
  | BackendKeyboardInteractiveAuth
  | BackendPrivateKeyAuth
  | BackendAgentAuth
  | BackendCertificateAuth;

type ParseResult<T> =
  | {
      ok: true;
      value: T;
    }
  | {
      ok: false;
      reason: string;
    };

type BackendProxyType =
  | 'None'
  | {
      Socks5: {
        host: string;
        port: number;
        username: string | null;
        password: string | null;
      };
    }
  | {
      Http: {
        host: string;
        port: number;
        username: string | null;
        password: string | null;
      };
    }
  | {
      JumpHost: {
        host: string;
        port: number;
        username: string;
        auth_method: BackendAuthMethod;
      };
    };

type LocalForward = {
  local_host: string;
  local_port: number;
  remote_host: string;
  remote_port: number;
};

type RemoteForward = {
  remote_host: string;
  remote_port: number;
  local_host: string;
  local_port: number;
};

export interface BackendConnectionConfig {
  id: string;
  name: string;
  protocol: ConnectionProtocol;
  host: string;
  port: number;
  username: string;
  auth_method: BackendAuthMethod;
  description: string | null;
  tags: string[];
  group_id: string | null;
  local_forwards: LocalForward[];
  remote_forwards: RemoteForward[];
  proxy_type: BackendProxyType;
  socks_proxy_port: number | null;
  auto_reconnect: boolean | null;
  created_at: string;
  updated_at: string;
}

export interface ConnectionFormData {
  id?: string;
  name: string;
  protocol?: ConnectionProtocol;
  host: string;
  port: number;
  username?: string;
  authMethod?: AuthMethodKind;
  password?: string;
  savePassword?: boolean;
  keyPath?: string;
  passphrase?: string;
  savePassphrase?: boolean;
  agentPath?: string;
  certificatePath?: string;
  privateKeyPath?: string;
  description?: string | null;
  tags?: string[] | string;
  local_forwards?: LocalForward[];
  remote_forwards?: RemoteForward[];
  proxyType?: ProxyTypeKind;
  proxyHost?: string;
  proxyPort?: number;
  proxyUsername?: string;
  proxyPassword?: string;
  jumpHostUsername?: string;
  jumpHostAuthMethod?: AuthMethodKind;
  jumpHostPassword?: string;
  jumpHostSavePassword?: boolean;
  jumpHostKeyPath?: string;
  jumpHostPassphrase?: string;
  jumpHostSavePassphrase?: boolean;
  jumpHostAgentPath?: string;
  jumpHostCertificatePath?: string;
  jumpHostPrivateKeyPath?: string;
  socksProxyPort?: number | null;
  autoReconnect?: boolean | null;
  created_at?: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object';
}

function asString(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

function asNullableString(value: unknown): string | null {
  if (value === null || value === undefined) return null;
  return typeof value === 'string' ? value : null;
}

function asNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function asBoolean(value: unknown): boolean | null {
  return typeof value === 'boolean' ? value : null;
}

function normalizeProtocol(value: unknown): ConnectionProtocol {
  if (value === 'Rdp' || value === 'Telnet') return value;
  return 'Ssh';
}

function parseBackendAuthMethod(value: unknown): ParseResult<BackendAuthMethod> {
  if (!isRecord(value)) {
    return { ok: false, reason: 'auth_method must be an object' };
  }

  const hasPassword = value.Password !== undefined;
  const hasKeyboardInteractive = value.KeyboardInteractive !== undefined;
  const hasPrivateKey = value.PrivateKey !== undefined;
  const hasAgent = value.Agent !== undefined;
  const hasCertificate = value.Certificate !== undefined;
  const variantCount =
    Number(hasPassword) +
    Number(hasKeyboardInteractive) +
    Number(hasPrivateKey) +
    Number(hasAgent) +
    Number(hasCertificate);

  if (variantCount !== 1) {
    return {
      ok: false,
      reason: 'auth_method must contain exactly one variant',
    };
  }

  if (hasPassword) {
    const password = value.Password;
    if (!isRecord(password)) {
      return { ok: false, reason: 'auth_method.Password must be an object' };
    }
    const rawPassword = asString(password.password);
    const rawSavePassword = asBoolean(password.save_password);
    if (rawPassword === null || rawSavePassword === null) {
      return {
        ok: false,
        reason: 'auth_method.Password.password/save_password have invalid types',
      };
    }
    return {
      ok: true,
      value: {
        Password: {
          password: rawPassword,
          save_password: rawSavePassword,
        },
      },
    };
  }

  if (hasKeyboardInteractive) {
    if (!isRecord(value.KeyboardInteractive)) {
      return {
        ok: false,
        reason: 'auth_method.KeyboardInteractive must be an object',
      };
    }
    return { ok: true, value: { KeyboardInteractive: {} } };
  }

  if (hasPrivateKey) {
    const privateKey = value.PrivateKey;
    if (!isRecord(privateKey)) {
      return { ok: false, reason: 'auth_method.PrivateKey must be an object' };
    }
    const keyPath = asString(privateKey.key_path);
    const savePassphrase = asBoolean(privateKey.save_passphrase);
    if (keyPath === null || savePassphrase === null) {
      return {
        ok: false,
        reason: 'auth_method.PrivateKey.key_path/save_passphrase have invalid types',
      };
    }
    const passphraseRaw = privateKey.passphrase;
    if (passphraseRaw !== undefined && passphraseRaw !== null && typeof passphraseRaw !== 'string') {
      return {
        ok: false,
        reason: 'auth_method.PrivateKey.passphrase must be string/null/undefined',
      };
    }
    return {
      ok: true,
      value: {
        PrivateKey: {
          key_path: keyPath,
          passphrase: asNullableString(passphraseRaw) ?? undefined,
          save_passphrase: savePassphrase,
        },
      },
    };
  }

  if (hasAgent) {
    const agent = value.Agent;
    if (!isRecord(agent)) {
      return { ok: false, reason: 'auth_method.Agent must be an object' };
    }
    const agentPathRaw = agent.agent_path;
    if (agentPathRaw !== undefined && agentPathRaw !== null && typeof agentPathRaw !== 'string') {
      return {
        ok: false,
        reason: 'auth_method.Agent.agent_path must be string/null/undefined',
      };
    }
    return {
      ok: true,
      value: {
        Agent: {
          agent_path: asNullableString(agentPathRaw) ?? undefined,
        },
      },
    };
  }

  const certificate = value.Certificate;
  if (!isRecord(certificate)) {
    return { ok: false, reason: 'auth_method.Certificate must be an object' };
  }
  const certificatePath = asString(certificate.certificate_path);
  const privateKeyPath = asString(certificate.private_key_path);
  const savePassphrase = asBoolean(certificate.save_passphrase);
  if (certificatePath === null || privateKeyPath === null || savePassphrase === null) {
    return {
      ok: false,
      reason: 'auth_method.Certificate.certificate_path/private_key_path/save_passphrase have invalid types',
    };
  }
  const passphraseRaw = certificate.passphrase;
  if (passphraseRaw !== undefined && passphraseRaw !== null && typeof passphraseRaw !== 'string') {
    return {
      ok: false,
      reason: 'auth_method.Certificate.passphrase must be string/null/undefined',
    };
  }
  return {
    ok: true,
    value: {
      Certificate: {
        certificate_path: certificatePath,
        private_key_path: privateKeyPath,
        passphrase: asNullableString(passphraseRaw) ?? undefined,
        save_passphrase: savePassphrase,
      },
    },
  };
}

function normalizeBackendAuthMethod(value: unknown): BackendAuthMethod {
  const parsed = parseBackendAuthMethod(value);
  if (!parsed.ok) {
    throw new Error(`Invalid auth_method: ${parsed.reason}`);
  }
  return parsed.value;
}

function normalizeConnectionAuthMethod(value: Connection['auth_method']): BackendAuthMethod {
  return normalizeBackendAuthMethod(value);
}

function toConnectionAuthMethod(auth: BackendAuthMethod): Connection['auth_method'] {
  if ('Password' in auth) {
    return {
      Password: {
        password: auth.Password.password,
        save_password: auth.Password.save_password,
      },
    };
  }
  if ('PrivateKey' in auth) {
    return {
      PrivateKey: {
        key_path: auth.PrivateKey.key_path,
        passphrase: auth.PrivateKey.passphrase,
        save_passphrase: auth.PrivateKey.save_passphrase,
      },
    };
  }
  if ('Agent' in auth) {
    return {
      Agent: {
        agent_path: auth.Agent.agent_path,
      },
    };
  }
  if ('Certificate' in auth) {
    return {
      Certificate: {
        certificate_path: auth.Certificate.certificate_path,
        private_key_path: auth.Certificate.private_key_path,
        passphrase: auth.Certificate.passphrase,
        save_passphrase: auth.Certificate.save_passphrase,
      },
    };
  }
  return { KeyboardInteractive: {} };
}

function normalizeLocalForwards(value: unknown): LocalForward[] {
  if (!Array.isArray(value)) return [];
  return value
    .map(entry => {
      if (!isRecord(entry)) return null;
      const localPort = asNumber(entry.local_port);
      const remotePort = asNumber(entry.remote_port);
      if (localPort === null || remotePort === null) return null;
      return {
        local_host: asString(entry.local_host) ?? 'localhost',
        local_port: localPort,
        remote_host: asString(entry.remote_host) ?? 'localhost',
        remote_port: remotePort,
      };
    })
    .filter((entry): entry is LocalForward => entry !== null);
}

function normalizeRemoteForwards(value: unknown): RemoteForward[] {
  if (!Array.isArray(value)) return [];
  return value
    .map(entry => {
      if (!isRecord(entry)) return null;
      const localPort = asNumber(entry.local_port);
      const remotePort = asNumber(entry.remote_port);
      if (localPort === null || remotePort === null) return null;
      return {
        remote_host: asString(entry.remote_host) ?? 'localhost',
        remote_port: remotePort,
        local_host: asString(entry.local_host) ?? 'localhost',
        local_port: localPort,
      };
    })
    .filter((entry): entry is RemoteForward => entry !== null);
}

function normalizeConnectionFromBackend(value: unknown): Connection | null {
  if (!isRecord(value)) return null;

  const id = asString(value.id);
  const name = asString(value.name);
  const host = asString(value.host);
  const port = asNumber(value.port);

  if (!id || !name || !host || port === null) {
    return null;
  }

  const parsedAuthMethod = parseBackendAuthMethod(value.auth_method);
  if (!parsedAuthMethod.ok) {
    devWarn(
      'connectionService',
      `Skipping connection ${id}: invalid auth_method (${parsedAuthMethod.reason})`
    );
    return null;
  }
  const parsedProxyType = parseBackendProxyType(value.proxy_type);
  if (!parsedProxyType.ok) {
    devWarn(
      'connectionService',
      `Skipping connection ${id}: invalid proxy_type (${parsedProxyType.reason})`
    );
    return null;
  }

  return {
    id,
    name,
    protocol: normalizeProtocol(value.protocol),
    host,
    port,
    username: asString(value.username) ?? '',
    auth_method: toConnectionAuthMethod(parsedAuthMethod.value),
    description: asNullableString(value.description),
    tags: parseTags(value.tags),
    created_at: asString(value.created_at) ?? new Date().toISOString(),
    updated_at: asString(value.updated_at) ?? new Date().toISOString(),
    group_id: asNullableString(value.group_id),
    local_forwards: normalizeLocalForwards(value.local_forwards),
    remote_forwards: normalizeRemoteForwards(value.remote_forwards),
    proxy_type: parsedProxyType.value,
    socks_proxy_port: asNumber(value.socks_proxy_port),
    auto_reconnect: asBoolean(value.auto_reconnect) ?? undefined,
  };
}

function normalizeConnectionsPayload(payload: unknown): Connection[] {
  if (!Array.isArray(payload)) {
    devWarn('connectionService', 'Expected array from get_all_connection_configs, got non-array payload');
    return [];
  }

  let skipped = 0;
  const normalized: Connection[] = [];

  for (const item of payload) {
    const connection = normalizeConnectionFromBackend(item);
    if (!connection) {
      skipped += 1;
      continue;
    }
    normalized.push(connection);
  }

  if (skipped > 0) {
    devWarn('connectionService', `Skipped ${skipped} invalid connection entries from backend payload`);
  }

  return normalized;
}

export async function loadConnections() {
  try {
    loading.set(true);
    clearErrorMessage();
    
    const result = await invoke<unknown>('get_all_connection_configs');
    connections.set(normalizeConnectionsPayload(result));
  } catch (error) {
    console.error('Error loading connections:', error);
    showErrorMessage(`加载连接失败: ${error}`, 5000);
  } finally {
    loading.set(false);
  }
}

export async function deleteConnection(connectionId: string) {
  const currentConnections = get(connections);
  const restoreIndex = currentConnections.findIndex(c => c.id === connectionId);
  const removedConnection = restoreIndex >= 0 ? currentConnections[restoreIndex] : null;
  connections.update(items => items.filter(c => c.id !== connectionId));

  try {
    await invoke('delete_connection_config', { connectionId });
    showSuccessMessage('连接删除成功！', 5000);
  } catch (error) {
    console.error('Error deleting connection:', error);
    if (removedConnection) {
      connections.update(items => {
        if (items.some(item => item.id === connectionId)) {
          return items;
        }
        const next = [...items];
        const insertAt = Math.max(0, Math.min(restoreIndex, next.length));
        next.splice(insertAt, 0, removedConnection);
        return next;
      });
    }
    showErrorMessage(`删除连接失败：${error}`, 5000);
  }
}

function parseBackendProxyType(proxyType: unknown): ParseResult<BackendProxyType> {
  if (proxyType === null || proxyType === undefined || proxyType === 'None') {
    return { ok: true, value: 'None' };
  }
  if (!isRecord(proxyType)) {
    return { ok: false, reason: 'proxy_type must be "None" or an object variant' };
  }

  const hasSocks5 = proxyType.Socks5 !== undefined;
  const hasHttp = proxyType.Http !== undefined;
  const hasJumpHost = proxyType.JumpHost !== undefined;
  const variantCount = Number(hasSocks5) + Number(hasHttp) + Number(hasJumpHost);

  if (variantCount !== 1) {
    return {
      ok: false,
      reason: 'proxy_type must contain exactly one variant',
    };
  }

  if (hasSocks5) {
    const socks = proxyType.Socks5;
    if (!isRecord(socks)) {
      return { ok: false, reason: 'proxy_type.Socks5 must be an object' };
    }
    const host = asString(socks.host);
    const port = asNumber(socks.port);
    if (host === null || port === null) {
      return {
        ok: false,
        reason: 'proxy_type.Socks5.host/port have invalid types',
      };
    }
    const usernameRaw = socks.username;
    const passwordRaw = socks.password;
    if (usernameRaw !== undefined && usernameRaw !== null && typeof usernameRaw !== 'string') {
      return { ok: false, reason: 'proxy_type.Socks5.username must be string/null/undefined' };
    }
    if (passwordRaw !== undefined && passwordRaw !== null && typeof passwordRaw !== 'string') {
      return { ok: false, reason: 'proxy_type.Socks5.password must be string/null/undefined' };
    }
    return {
      ok: true,
      value: {
        Socks5: {
          host,
          port,
          username: asNullableString(usernameRaw),
          password: asNullableString(passwordRaw),
        },
      },
    };
  }

  if (hasHttp) {
    const http = proxyType.Http;
    if (!isRecord(http)) {
      return { ok: false, reason: 'proxy_type.Http must be an object' };
    }
    const host = asString(http.host);
    const port = asNumber(http.port);
    if (host === null || port === null) {
      return {
        ok: false,
        reason: 'proxy_type.Http.host/port have invalid types',
      };
    }
    const usernameRaw = http.username;
    const passwordRaw = http.password;
    if (usernameRaw !== undefined && usernameRaw !== null && typeof usernameRaw !== 'string') {
      return { ok: false, reason: 'proxy_type.Http.username must be string/null/undefined' };
    }
    if (passwordRaw !== undefined && passwordRaw !== null && typeof passwordRaw !== 'string') {
      return { ok: false, reason: 'proxy_type.Http.password must be string/null/undefined' };
    }
    return {
      ok: true,
      value: {
        Http: {
          host,
          port,
          username: asNullableString(usernameRaw),
          password: asNullableString(passwordRaw),
        },
      },
    };
  }

  const jumpHost = proxyType.JumpHost;
  if (!isRecord(jumpHost)) {
    return { ok: false, reason: 'proxy_type.JumpHost must be an object' };
  }
  const host = asString(jumpHost.host);
  const port = asNumber(jumpHost.port);
  const username = asString(jumpHost.username);
  if (host === null || port === null || username === null) {
    return {
      ok: false,
      reason: 'proxy_type.JumpHost.host/port/username have invalid types',
    };
  }
  const parsedAuthMethod = parseBackendAuthMethod(jumpHost.auth_method);
  if (!parsedAuthMethod.ok) {
    return {
      ok: false,
      reason: `proxy_type.JumpHost.auth_method is invalid (${parsedAuthMethod.reason})`,
    };
  }
  return {
    ok: true,
    value: {
      JumpHost: {
        host,
        port,
        username,
        auth_method: parsedAuthMethod.value,
      },
    },
  };
}

function normalizeBackendProxyType(proxyType: unknown): BackendProxyType {
  const parsed = parseBackendProxyType(proxyType);
  if (!parsed.ok) {
    throw new Error(`Invalid proxy_type: ${parsed.reason}`);
  }
  return parsed.value;
}

function parseTags(value: unknown): string[] {
  if (Array.isArray(value)) {
    return value.map(v => String(v).trim()).filter(Boolean);
  }
  if (typeof value === 'string') {
    return value
      .split(/[,\uFF0C;\uFF1B\n]+/g)
      .map(tag => tag.trim())
      .filter(Boolean);
  }
  return [];
}

function toBackendConnectionConfig(
  connection: Connection,
  overrides?: Partial<BackendConnectionConfig>
): BackendConnectionConfig {
  const protocol: ConnectionProtocol = connection.protocol ?? 'Ssh';
  const proxyType: BackendProxyType = normalizeBackendProxyType(connection.proxy_type);

  return {
    id: connection.id,
    name: connection.name,
    protocol,
    host: connection.host,
    port: Number(connection.port),
    username: connection.username,
    auth_method: normalizeConnectionAuthMethod(connection.auth_method),
    description: connection.description ?? null,
    tags: connection.tags ?? [],
    group_id: connection.group_id ?? null,
    local_forwards: connection.local_forwards ?? [],
    remote_forwards: connection.remote_forwards ?? [],
    proxy_type: proxyType,
    socks_proxy_port: connection.socks_proxy_port ?? null,
    auto_reconnect: connection.auto_reconnect ?? null,
    created_at: connection.created_at,
    updated_at: connection.updated_at,
    ...(overrides ?? {}),
  };
}

export async function updateConnectionConfig(connection: Connection) {
  await invoke('save_connection_config', { config: toBackendConnectionConfig(connection) });
}

export async function createBackendConfig(connectionData: ConnectionFormData): Promise<BackendConnectionConfig> {
  const isEditing = Boolean(connectionData.id);
  const protocol: ConnectionProtocol =
    connectionData.protocol === 'Rdp'
      ? 'Rdp'
      : connectionData.protocol === 'Telnet'
        ? 'Telnet'
        : 'Ssh';
  
  // Validate form data
  const trimmedName = String(connectionData.name ?? '').trim();
  if (!trimmedName) throw new Error('连接名称不能为空');
  const trimmedHost = String(connectionData.host || '').trim();
  if (!trimmedHost) throw new Error('主机地址不能为空');
  const normalizedPort = Number(connectionData.port);
  if (normalizedPort < 1 || normalizedPort > 65535) throw new Error('端口号必须在1-65535之间');
  const effectiveUsername = protocol === 'Ssh'
    ? (String(connectionData.username || '').trim() || 'root')
    : String(connectionData.username || '').trim();
  
  // Construct auth method
  let backendAuthMethod: BackendAuthMethod;
  if (protocol === 'Rdp' || protocol === 'Telnet') {
    backendAuthMethod = {
      Password: {
        password: '',
        save_password: false,
      },
    };
  } else {
    switch (connectionData.authMethod) {
    case 'password':
      {
        const password = String(connectionData.password ?? '');
        const savePassword = Boolean(connectionData.savePassword);
        if (!password) {
          if (savePassword) {
            if (!isEditing) throw new Error('新建连接时，勾选保存密码需要先输入密码');
          } else {
            throw new Error('密码不能为空');
          }
        }
        backendAuthMethod = {
          Password: {
            password,
            save_password: savePassword,
          },
        };
      }
      break;
    case 'keyboardInteractive':
      backendAuthMethod = {
        KeyboardInteractive: {},
      };
      break;
    case 'privateKey': {
      const keyPath = String(connectionData.keyPath ?? '').trim();
      if (!keyPath) throw new Error('私钥路径不能为空');
      backendAuthMethod = {
        PrivateKey: {
          key_path: keyPath,
          passphrase: connectionData.passphrase || undefined,
          save_passphrase: Boolean(connectionData.savePassphrase),
        },
      };
      break;
    }
    case 'agent':
      backendAuthMethod = {
        Agent: {
          agent_path: connectionData.agentPath || undefined,
        },
      };
      break;
    case 'certificate': {
      const certificatePath = String(connectionData.certificatePath ?? '').trim();
      const privateKeyPath = String(connectionData.privateKeyPath ?? '').trim();
      if (!certificatePath) throw new Error('证书路径不能为空');
      if (!privateKeyPath) throw new Error('证书对应的私钥路径不能为空');
      backendAuthMethod = {
        Certificate: {
          certificate_path: certificatePath,
          private_key_path: privateKeyPath,
          passphrase: connectionData.passphrase || undefined,
          save_passphrase: Boolean(connectionData.savePassphrase),
        },
      };
      break;
    }
    default:
      throw new Error('不支持的认证方式');
    }
  }

  const tagsArray = parseTags(connectionData.tags);

  // Load groups to find the corresponding group_id based on tags[0]
  const groups = get(connectionGroups);
  const folderTag = tagsArray[0] || '未分组';

  // Auto-create group if it doesn't exist in connectionGroups
  // This ensures groups created through tag input don't disappear when connections are moved
  let finalGroups = groups;
  if (folderTag !== '未分组' && !groups.some(g => g.name === folderTag)) {
    const newGroup = {
      id: uuidv4(),
      name: folderTag,
      createdAt: Date.now()
    };
    connectionGroups.update(groupList => [...groupList, newGroup]);
    finalGroups = [...groups, newGroup];
  }

  const groupId = getGroupIdByPath(finalGroups, folderTag);

  const connectionId = connectionData.id || uuidv4();
  const proxyHost = String(connectionData.proxyHost ?? '').trim();
  const proxyPort = Number(connectionData.proxyPort ?? 0);

  // Construct proxy configuration
  let backendProxyType: BackendProxyType;
  switch (connectionData.proxyType) {
    case 'none':
      backendProxyType = 'None';
      break;
    case 'socks5':
      if (!proxyHost) throw new Error('SOCKS5代理主机不能为空');
      backendProxyType = {
        Socks5: {
          host: proxyHost,
          port: Number(connectionData.proxyPort || 1080),
          username: connectionData.proxyUsername?.trim() || null,
          password: connectionData.proxyPassword?.trim() || null,
        },
      };
      break;
    case 'http':
      if (!proxyHost) throw new Error('HTTP代理主机不能为空');
      backendProxyType = {
        Http: {
          host: proxyHost,
          port: Number(connectionData.proxyPort || 8080),
          username: connectionData.proxyUsername?.trim() || null,
          password: connectionData.proxyPassword?.trim() || null,
        },
      };
      break;
    case 'jumpHost': {
      const jumpHostUsername = String(connectionData.jumpHostUsername ?? '').trim();
      if (!proxyHost) throw new Error('跳板主机不能为空');
      if (!jumpHostUsername) throw new Error('跳板用户名不能为空');
      let jumpAuthMethod: BackendAuthMethod;
      switch (connectionData.jumpHostAuthMethod) {
        case 'password': {
          const jumpHostPassword = String(connectionData.jumpHostPassword ?? '');
          const jumpHostSavePassword = Boolean(connectionData.jumpHostSavePassword);
          if (!jumpHostPassword) {
            if (jumpHostSavePassword) {
              if (!isEditing) throw new Error('新建连接时，勾选保存跳板密码需要先输入跳板密码');
            } else {
              throw new Error('跳板密码不能为空');
            }
          }
          jumpAuthMethod = {
            Password: {
              password: jumpHostPassword,
              save_password: jumpHostSavePassword,
            },
          };
          break;
        }
        case 'keyboardInteractive':
          jumpAuthMethod = {
            KeyboardInteractive: {},
          };
          break;
        case 'privateKey': {
          const jumpHostKeyPath = String(connectionData.jumpHostKeyPath ?? '').trim();
          if (!jumpHostKeyPath) throw new Error('跳板私钥路径不能为空');
          jumpAuthMethod = {
            PrivateKey: {
              key_path: jumpHostKeyPath,
              passphrase: connectionData.jumpHostPassphrase || undefined,
              save_passphrase: Boolean(connectionData.jumpHostSavePassphrase),
            },
          };
          break;
        }
        case 'agent':
          jumpAuthMethod = {
            Agent: {
              agent_path: connectionData.jumpHostAgentPath || undefined,
            },
          };
          break;
        case 'certificate': {
          const jumpHostCertificatePath = String(connectionData.jumpHostCertificatePath ?? '').trim();
          const jumpHostPrivateKeyPath = String(connectionData.jumpHostPrivateKeyPath ?? '').trim();
          if (!jumpHostCertificatePath) throw new Error('跳板证书路径不能为空');
          if (!jumpHostPrivateKeyPath) throw new Error('跳板证书对应的私钥路径不能为空');
          jumpAuthMethod = {
            Certificate: {
              certificate_path: jumpHostCertificatePath,
              private_key_path: jumpHostPrivateKeyPath,
              passphrase: connectionData.jumpHostPassphrase || undefined,
              save_passphrase: Boolean(connectionData.jumpHostSavePassphrase),
            },
          };
          break;
        }
        default:
          throw new Error('不支持的跳板认证方式');
      }
      backendProxyType = {
        JumpHost: {
          host: proxyHost,
          port: proxyPort > 0 ? proxyPort : 22,
          username: jumpHostUsername,
          auth_method: jumpAuthMethod,
        },
      };
      break;
    }
    default:
      backendProxyType = 'None';
      break;
  }

  const nonSsh = protocol !== 'Ssh';
  const effectiveProxyType: BackendProxyType = nonSsh ? 'None' : backendProxyType;
  const effectiveLocalForwards = nonSsh ? [] : (connectionData.local_forwards || []);
  const effectiveRemoteForwards = nonSsh ? [] : (connectionData.remote_forwards || []);
  const effectiveSocksProxyPort = nonSsh ? null : (connectionData.socksProxyPort || null);

  return {
    id: connectionId,
    name: trimmedName,
    protocol,
    host: trimmedHost,
    port: normalizedPort,
    username: effectiveUsername,
    auth_method: backendAuthMethod,
    description: connectionData.description || null,
    tags: tagsArray,
    group_id: groupId,
    local_forwards: effectiveLocalForwards,
    remote_forwards: effectiveRemoteForwards,
    proxy_type: effectiveProxyType,
    socks_proxy_port: effectiveSocksProxyPort,
    auto_reconnect: connectionData.autoReconnect ?? null,
    created_at: connectionData.created_at || new Date().toISOString(),
    updated_at: new Date().toISOString(),
  };
}

export async function saveConnection(
  connectionData: ConnectionFormData
): Promise<{ connectionId: string; connectConfig: BackendConnectionConfig } | null> {
  try {
    clearErrorMessage();
    const connectConfig = await createBackendConfig(connectionData);
    
    await invoke('save_connection_config', { config: connectConfig });
    
    showSuccessMessage('连接保存成功！', 5000);
    await loadConnections();
    return { connectionId: connectConfig.id, connectConfig };
  } catch (error) {
    console.error('Error saving connection:', error);
    showErrorMessage(`保存连接失败：${error instanceof Error ? error.message : error}`, 5000);
    return null;
  }
}

export const __connectionServiceTestHooks = {
  parseBackendAuthMethod,
  parseBackendProxyType,
  normalizeConnectionsPayload,
  toBackendConnectionConfig,
};
