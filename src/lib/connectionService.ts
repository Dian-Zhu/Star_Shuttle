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

export async function loadConnections() {
  try {
    loading.set(true);
    clearErrorMessage();
    
    const result = await invoke('get_all_connection_configs');
    connections.set(result as Connection[]);
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

function normalizeBackendProxyType(proxyType: Connection['proxy_type']): BackendProxyType {
  if (!proxyType || proxyType === 'None') {
    return 'None';
  }
  if (typeof proxyType !== 'object') {
    return 'None';
  }
  const record = proxyType as Record<string, unknown>;
  const asRecord = (value: unknown): Record<string, unknown> | null =>
    value && typeof value === 'object' ? (value as Record<string, unknown>) : null;
  const asString = (value: unknown): string | null =>
    typeof value === 'string' ? value : null;
  const asNumber = (value: unknown): number | null =>
    typeof value === 'number' && Number.isFinite(value) ? value : null;

  const socks = asRecord(record.Socks5);
  if (socks) {
    return {
      Socks5: {
        host: asString(socks.host) ?? '',
        port: asNumber(socks.port) ?? 1080,
        username: asString(socks.username),
        password: asString(socks.password),
      },
    };
  }

  const http = asRecord(record.Http);
  if (http) {
    return {
      Http: {
        host: asString(http.host) ?? '',
        port: asNumber(http.port) ?? 8080,
        username: asString(http.username),
        password: asString(http.password),
      },
    };
  }

  const jumpHost = asRecord(record.JumpHost);
  if (jumpHost) {
    const authMethod = asRecord(jumpHost.auth_method);
    if (!authMethod) {
      return 'None';
    }
    return {
      JumpHost: {
        host: asString(jumpHost.host) ?? '',
        port: asNumber(jumpHost.port) ?? 22,
        username: asString(jumpHost.username) ?? '',
        auth_method: authMethod as BackendAuthMethod,
      },
    };
  }
  return 'None';
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
    auth_method: connection.auth_method as BackendAuthMethod,
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
