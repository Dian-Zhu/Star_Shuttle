import { save, open } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';
import { loadConnections } from './connectionService';
import { localFsService } from './localFsService';
import { successMessage, errorMessage } from './store';
import type { Connection } from './store';

const IS_DEV = import.meta.env.DEV;

function sanitizeAuthMethodForExport(auth: Connection['auth_method']): Connection['auth_method'] {
  if (auth.Password) {
    return {
      Password: {
        ...auth.Password,
        password: ''
      }
    };
  }

  if (auth.PrivateKey) {
    const rest: Omit<typeof auth.PrivateKey, 'passphrase'> & { passphrase?: string } = { ...auth.PrivateKey };
    delete rest.passphrase;
    return {
      PrivateKey: rest
    };
  }

  if (auth.Certificate) {
    const rest: Omit<typeof auth.Certificate, 'passphrase'> & { passphrase?: string } = { ...auth.Certificate };
    delete rest.passphrase;
    return {
      Certificate: rest
    };
  }

  if (auth.Agent) {
    return { Agent: { ...auth.Agent } };
  }

  return { KeyboardInteractive: {} };
}

function sanitizeProxyTypeForExport(proxyType: any): any {
  if (!proxyType || typeof proxyType !== 'object') {
    return proxyType;
  }

  if (proxyType.Socks5) {
    return {
      Socks5: {
        ...proxyType.Socks5,
        password: null
      }
    };
  }

  if (proxyType.Http) {
    return {
      Http: {
        ...proxyType.Http,
        password: null
      }
    };
  }

  if (proxyType.JumpHost) {
    const jumpHost = proxyType.JumpHost;
    return {
      JumpHost: {
        ...jumpHost,
        auth_method: sanitizeAuthMethodForExport(jumpHost.auth_method ?? { KeyboardInteractive: {} })
      }
    };
  }

  // Some imported data may use flattened shape; sanitize common password-like fields defensively.
  const sanitized = { ...proxyType };
  if ('password' in sanitized) {
    sanitized.password = null;
  }
  return sanitized;
}

function sanitizeConnectionForExport(connection: Connection): Connection {
  return {
    ...connection,
    auth_method: sanitizeAuthMethodForExport(connection.auth_method),
    proxy_type: sanitizeProxyTypeForExport(connection.proxy_type),
  };
}

export async function exportConnections(options?: { includeSensitive?: boolean }) {
  try {
    const connections = await invoke('get_all_connection_configs') as Connection[];
    
    if (connections.length === 0) {
      errorMessage.set('没有可导出的连接');
      setTimeout(() => errorMessage.set(null), 3000);
      return;
    }

    if (options?.includeSensitive === true && IS_DEV) {
      console.warn('[exportConnections] includeSensitive is ignored because credentials in secure storage cannot be exported.');
    }

    const filePath = await save({
      filters: [{
        name: 'JSON',
        extensions: ['json']
      }],
      defaultPath: 'connections.json'
    });

    if (!filePath) return; // User cancelled

    // Connection data from backend is already sanitized; we still sanitize defensively here.
    const exportData = connections.map(sanitizeConnectionForExport);

    await localFsService.writeTextFile(filePath, JSON.stringify(exportData, null, 2));
    
    successMessage.set('导出成功（不含密码/口令）');
    setTimeout(() => successMessage.set(null), 3000);
  } catch (error) {
    console.error('Export failed:', error);
    errorMessage.set(`导出失败: ${error}`);
    setTimeout(() => errorMessage.set(null), 5000);
  }
}

export async function importConnections() {
  try {
    const filePath = await open({
      filters: [{
        name: 'JSON',
        extensions: ['json']
      }]
    });

    if (!filePath) return; // User cancelled

    const pathStr = typeof filePath === 'string' ? filePath : filePath[0];
    
    const jsonContent = await localFsService.readTextFile(pathStr);
    let importedConnections: Connection[];
    
    try {
      importedConnections = JSON.parse(jsonContent);
    } catch (e) {
      throw new Error('文件格式错误，必须是有效的 JSON');
    }

    if (!Array.isArray(importedConnections)) {
      throw new Error('文件格式错误，根元素必须是数组');
    }

    let successCount = 0;
    let failCount = 0;

    for (const conn of importedConnections) {
      try {
        // Validate basic fields
        if (!conn.name || !conn.host) {
          if (IS_DEV) console.warn('Skipping invalid connection:', conn);
          failCount++;
          continue;
        }

        // Import defaults to append semantics: regenerate ID to avoid silent overwrite.
        const configToSave: Connection = {
          ...conn,
          id: crypto.randomUUID(),
        };

        await invoke('save_connection_config', { config: configToSave });
        successCount++;
      } catch (e) {
        console.error('Failed to save imported connection:', e);
        failCount++;
      }
    }

    await loadConnections();
    
    if (failCount > 0) {
      successMessage.set(`导入完成: ${successCount} 个成功, ${failCount} 个失败`);
    } else {
      successMessage.set(`成功导入 ${successCount} 个连接`);
    }
    setTimeout(() => successMessage.set(null), 3000);

  } catch (error) {
    console.error('Import failed:', error);
    errorMessage.set(`导入失败: ${error}`);
    setTimeout(() => errorMessage.set(null), 5000);
  }
}
