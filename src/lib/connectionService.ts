import { invoke } from '@tauri-apps/api/core';
import { connections, errorMessage, successMessage, loading, type Connection } from './store';
import { v4 as uuidv4 } from 'uuid';

export async function loadConnections() {
  try {
    loading.set(true);
    errorMessage.set(null);
    
    const result = await invoke('get_all_connection_configs');
    connections.set(result as Connection[]);
    console.log('Connections loaded:', result);
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
    await invoke('delete_connection_config', { connection_id: connectionId });
    
    successMessage.set('连接删除成功！');
    await loadConnections();
    
    setTimeout(() => successMessage.set(null), 5000);
  } catch (error) {
    console.error('Error deleting connection:', error);
    errorMessage.set(`删除连接失败：${error}`);
    setTimeout(() => errorMessage.set(null), 5000);
  }
}

export async function saveConnection(connectionData: any) {
  try {
    errorMessage.set(null);
    
    // Validate form data
    if (!connectionData.name.trim()) throw new Error('连接名称不能为空');
    if (!connectionData.host.trim()) throw new Error('主机地址不能为空');
    if (connectionData.port < 1 || connectionData.port > 65535) throw new Error('端口号必须在1-65535之间');
    if (!connectionData.username.trim()) throw new Error('用户名不能为空');
    
    // Construct auth method
    let backendAuthMethod;
    switch (connectionData.authMethod) {
      case 'password':
        if (!connectionData.password) throw new Error('密码不能为空');
        backendAuthMethod = {
          Password: {
            password: connectionData.password,
            save_password: connectionData.savePassword,
          },
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

    await invoke('save_connection_config', {
      config: {
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
      },
    });

    successMessage.set('连接保存成功！');
    await loadConnections();
    setTimeout(() => successMessage.set(null), 5000);
    return true;
  } catch (error) {
    console.error('Error saving connection:', error);
    errorMessage.set(`保存连接失败：${error instanceof Error ? error.message : error}`);
    setTimeout(() => errorMessage.set(null), 5000);
    return false;
  }
}
