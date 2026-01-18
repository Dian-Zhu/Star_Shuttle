import { invoke } from '@tauri-apps/api/core';
import { connections, errorMessage, successMessage, loading, type Connection } from './store';
import { v4 as uuidv4 } from 'uuid';

export async function loadConnections() {
  try {
    loading.set(true);
    errorMessage.set(null);
    
    const result = await invoke('get_all_connection_configs');
    connections.set(result as Connection[]);
  } catch (error) {
    console.error('Error loading connections:', error);
    errorMessage.set(`加载连接失败: ${error}`);
    setTimeout(() => errorMessage.set(null), 5000);
  } finally {
    loading.set(false);
  }
}

export async function deleteConnection(connectionId: string) {
  try {
    await invoke('delete_connection_config', { connectionId });
    
    successMessage.set('连接删除成功！');
    await loadConnections();
    
    setTimeout(() => successMessage.set(null), 5000);
  } catch (error) {
    console.error('Error deleting connection:', error);
    errorMessage.set(`删除连接失败：${error}`);
    setTimeout(() => errorMessage.set(null), 5000);
  }
}

function toBackendConnectionConfig(connection: Connection, overrides?: Record<string, unknown>) {
  return {
    id: connection.id,
    name: connection.name,
    host: connection.host,
    port: Number(connection.port),
    username: connection.username,
    auth_method: connection.auth_method,
    description: connection.description ?? null,
    tags: connection.tags ?? [],
    group_id: null,
    local_forwards: connection.local_forwards ?? [],
    remote_forwards: connection.remote_forwards ?? [],
    proxy_type: (connection as any).proxy_type ?? 'None',
    socks_proxy_port: (connection as any).socks_proxy_port ?? null,
    ...(overrides ?? {}),
  };
}

export async function updateConnectionConfig(connection: Connection) {
  await invoke('save_connection_config', { config: toBackendConnectionConfig(connection) });
}

export async function saveConnection(connectionData: any): Promise<{ connectionId: string; connectConfig: any } | null> {
  try {
    errorMessage.set(null);
    const isEditing = Boolean(connectionData.id);
    
    // Validate form data
    if (!connectionData.name.trim()) throw new Error('连接名称不能为空');
    if (!connectionData.host.trim()) throw new Error('主机地址不能为空');
    if (connectionData.port < 1 || connectionData.port > 65535) throw new Error('端口号必须在1-65535之间');
    if (!connectionData.username.trim()) throw new Error('用户名不能为空');
    
    // Construct auth method
    let backendAuthMethod;
    switch (connectionData.authMethod) {
      case 'password':
        if (!connectionData.password) {
          if (connectionData.savePassword) {
            if (!isEditing) throw new Error('新建连接时，勾选保存密码需要先输入密码');
          } else {
            throw new Error('密码不能为空');
          }
        }
        backendAuthMethod = {
          Password: {
            password: connectionData.password,
            save_password: connectionData.savePassword,
          },
        };
        break;
      case 'keyboardInteractive':
        backendAuthMethod = {
          KeyboardInteractive: {},
        };
        break;
      case 'privateKey':
        if (!connectionData.keyPath) throw new Error('私钥路径不能为空');
        backendAuthMethod = {
          PrivateKey: {
            key_path: connectionData.keyPath,
            passphrase: connectionData.passphrase || undefined,
            save_passphrase: connectionData.savePassphrase,
          },
        };
        break;
      case 'agent':
        backendAuthMethod = {
          Agent: {
            agent_path: connectionData.agentPath || undefined,
          },
        };
        break;
      case 'certificate':
        if (!connectionData.certificatePath) throw new Error('证书路径不能为空');
        if (!connectionData.privateKeyPath) throw new Error('证书对应的私钥路径不能为空');
        backendAuthMethod = {
          Certificate: {
            certificate_path: connectionData.certificatePath,
            private_key_path: connectionData.privateKeyPath,
            passphrase: connectionData.passphrase || undefined,
            save_passphrase: connectionData.savePassphrase,
          },
        };
        break;
      default:
        throw new Error('不支持的认证方式');
    }

    // Parse tags
    const tagsArray = connectionData.tags
      ? connectionData.tags.split(',').map((tag: string) => tag.trim()).filter(Boolean)
      : [];

    const connectionId = connectionData.id || uuidv4();

    // Construct proxy configuration
    let backendProxyType;
    switch (connectionData.proxyType) {
      case 'none':
        backendProxyType = 'None';
        break;
      case 'socks5':
        if (!connectionData.proxyHost.trim()) throw new Error('SOCKS5代理主机不能为空');
        backendProxyType = {
          Socks5: {
            host: connectionData.proxyHost,
            port: Number(connectionData.proxyPort || 1080),
            username: connectionData.proxyUsername?.trim() || null,
            password: connectionData.proxyPassword?.trim() || null,
          },
        };
        break;
      case 'http':
        if (!connectionData.proxyHost.trim()) throw new Error('HTTP代理主机不能为空');
        backendProxyType = {
          Http: {
            host: connectionData.proxyHost,
            port: Number(connectionData.proxyPort || 8080),
            username: connectionData.proxyUsername?.trim() || null,
            password: connectionData.proxyPassword?.trim() || null,
          },
        };
        break;
      case 'jumpHost': {
        if (!connectionData.proxyHost.trim()) throw new Error('跳板主机不能为空');
        if (!connectionData.jumpHostUsername.trim()) throw new Error('跳板用户名不能为空');
        let jumpAuthMethod;
        switch (connectionData.jumpHostAuthMethod) {
          case 'password':
            if (!connectionData.jumpHostPassword) {
              if (connectionData.jumpHostSavePassword) {
                if (!isEditing) throw new Error('新建连接时，勾选保存跳板密码需要先输入跳板密码');
              } else {
                throw new Error('跳板密码不能为空');
              }
            }
            jumpAuthMethod = {
              Password: {
                password: connectionData.jumpHostPassword,
                save_password: connectionData.jumpHostSavePassword,
              },
            };
            break;
          case 'keyboardInteractive':
            jumpAuthMethod = {
              KeyboardInteractive: {},
            };
            break;
          case 'privateKey':
            if (!connectionData.jumpHostKeyPath) throw new Error('跳板私钥路径不能为空');
            jumpAuthMethod = {
              PrivateKey: {
                key_path: connectionData.jumpHostKeyPath,
                passphrase: connectionData.jumpHostPassphrase || undefined,
                save_passphrase: connectionData.jumpHostSavePassphrase,
              },
            };
            break;
          case 'agent':
            jumpAuthMethod = {
              Agent: {
                agent_path: connectionData.jumpHostAgentPath || undefined,
              },
            };
            break;
          case 'certificate':
            if (!connectionData.jumpHostCertificatePath) throw new Error('跳板证书路径不能为空');
            if (!connectionData.jumpHostPrivateKeyPath) throw new Error('跳板证书对应的私钥路径不能为空');
            jumpAuthMethod = {
              Certificate: {
                certificate_path: connectionData.jumpHostCertificatePath,
                private_key_path: connectionData.jumpHostPrivateKeyPath,
                passphrase: connectionData.jumpHostPassphrase || undefined,
                save_passphrase: connectionData.jumpHostSavePassphrase,
              },
            };
            break;
          default:
            throw new Error('不支持的跳板认证方式');
        }
        backendProxyType = {
          JumpHost: {
            host: connectionData.proxyHost,
            port: Number(connectionData.proxyPort || 22),
            username: connectionData.jumpHostUsername,
            auth_method: jumpAuthMethod,
          },
        };
        break;
      }
      default:
        backendProxyType = 'None';
        break;
    }

    const connectConfig = {
      id: connectionId,
      name: connectionData.name,
      host: connectionData.host,
      port: Number(connectionData.port),
      username: connectionData.username,
      auth_method: backendAuthMethod,
      description: connectionData.description || null,
      tags: tagsArray,
      group_id: null,
      local_forwards: connectionData.local_forwards || [],
      remote_forwards: connectionData.remote_forwards || [],
      proxy_type: backendProxyType,
      socks_proxy_port: connectionData.socksProxyPort || null,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };

    await invoke('save_connection_config', { config: connectConfig });

    successMessage.set('连接保存成功！');
    await loadConnections();
    setTimeout(() => successMessage.set(null), 5000);
    return { connectionId, connectConfig };
  } catch (error) {
    console.error('Error saving connection:', error);
    errorMessage.set(`保存连接失败：${error instanceof Error ? error.message : error}`);
    setTimeout(() => errorMessage.set(null), 5000);
    return null;
  }
}
