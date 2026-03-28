import { invoke } from '@tauri-apps/api/core';
import { loadConnections } from './connectionService';
import { localFsService } from './localFsService';
import { successMessage, errorMessage } from './store';
import type { Connection } from './store';

const IS_DEV = import.meta.env.DEV;
const MAX_IMPORT_JSON_BYTES = 8 * 1024 * 1024;
const LOCAL_FS_TEXT_TOO_LARGE_ERR = 'LOCAL_FS_TEXT_TOO_LARGE|';

function formatMiB(bytes: number): string {
  return `${Math.ceil(bytes / (1024 * 1024))} MB`;
}

function parseLocalFsTextTooLarge(error: unknown): { sizeBytes: number; maxBytes: number } | null {
  const raw = error instanceof Error ? error.message : String(error);
  const idx = raw.indexOf(LOCAL_FS_TEXT_TOO_LARGE_ERR);
  if (idx === -1) return null;
  const payload = raw.slice(idx + LOCAL_FS_TEXT_TOO_LARGE_ERR.length);
  try {
    const obj = JSON.parse(payload);
    const sizeBytes = typeof obj.size_bytes === 'number' ? obj.size_bytes : null;
    const maxBytes = typeof obj.max_bytes === 'number' ? obj.max_bytes : null;
    if (sizeBytes === null || maxBytes === null) return null;
    return { sizeBytes, maxBytes };
  } catch {
    return null;
  }
}

function sanitizeAuthMethodForExport(auth: Connection['auth_method']): Connection['auth_method'] {
  if (auth.Password) {
    return {
      Password: {
        ...auth.Password,
        password: '',
        save_password: false,
      }
    };
  }

  if (auth.PrivateKey) {
    const rest: Omit<typeof auth.PrivateKey, 'passphrase'> & { passphrase?: string } = { ...auth.PrivateKey };
    delete rest.passphrase;
    return {
      PrivateKey: {
        ...rest,
        save_passphrase: false,
      }
    };
  }

  if (auth.Certificate) {
    const rest: Omit<typeof auth.Certificate, 'passphrase'> & { passphrase?: string } = { ...auth.Certificate };
    delete rest.passphrase;
    return {
      Certificate: {
        ...rest,
        save_passphrase: false,
      }
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

function hasValue(value: unknown): boolean {
  return typeof value === 'string' && value.trim().length > 0;
}

function sanitizeAuthMethodForImport(auth: Connection['auth_method']): Connection['auth_method'] {
  if (auth.Password) {
    const password = auth.Password.password ?? '';
    const savePassword = hasValue(password) ? Boolean(auth.Password.save_password) : false;
    return {
      Password: {
        ...auth.Password,
        password,
        save_password: savePassword,
      }
    };
  }

  if (auth.PrivateKey) {
    const passphrase = auth.PrivateKey.passphrase;
    const hasPassphrase = hasValue(passphrase);
    const base: Omit<typeof auth.PrivateKey, 'passphrase'> & { passphrase?: string } = {
      ...auth.PrivateKey,
      save_passphrase: hasPassphrase ? Boolean(auth.PrivateKey.save_passphrase) : false,
    };
    if (hasPassphrase) {
      base.passphrase = passphrase;
    } else {
      delete base.passphrase;
    }
    return { PrivateKey: base };
  }

  if (auth.Certificate) {
    const passphrase = auth.Certificate.passphrase;
    const hasPassphrase = hasValue(passphrase);
    const base: Omit<typeof auth.Certificate, 'passphrase'> & { passphrase?: string } = {
      ...auth.Certificate,
      save_passphrase: hasPassphrase ? Boolean(auth.Certificate.save_passphrase) : false,
    };
    if (hasPassphrase) {
      base.passphrase = passphrase;
    } else {
      delete base.passphrase;
    }
    return { Certificate: base };
  }

  if (auth.Agent) {
    return { Agent: { ...auth.Agent } };
  }

  return { KeyboardInteractive: {} };
}

function sanitizeProxyTypeForImport(proxyType: any): any {
  if (!proxyType || typeof proxyType !== 'object') {
    return proxyType;
  }

  if (proxyType.JumpHost) {
    const jumpHost = proxyType.JumpHost;
    return {
      JumpHost: {
        ...jumpHost,
        auth_method: sanitizeAuthMethodForImport(jumpHost.auth_method ?? { KeyboardInteractive: {} }),
      }
    };
  }

  return proxyType;
}

function sanitizeConnectionForImport(connection: Connection): Connection {
  return {
    ...connection,
    auth_method: sanitizeAuthMethodForImport(connection.auth_method),
    proxy_type: sanitizeProxyTypeForImport(connection.proxy_type),
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

    const grant = await localFsService.pickFileForWrite('connections.json', [
      {
        name: 'JSON',
        extensions: ['json'],
      },
    ]);
    if (!grant) return; // User cancelled

    // Connection data from backend is already sanitized; we still sanitize defensively here.
    const exportData = connections.map(sanitizeConnectionForExport);
    await localFsService.writeTextFile(
      grant.path,
      JSON.stringify(exportData, null, 2),
      grant.accessToken,
    );
    
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
    const grant = await localFsService.pickFileForRead([
      {
        name: 'JSON',
        extensions: ['json'],
      },
    ]);
    if (!grant) return; // User cancelled

    // Avoid reading huge files into memory. Backend also enforces this limit, but we can fail fast
    // with a clearer message using the dialog-provided size.
    if (typeof grant.size === 'number' && grant.size > MAX_IMPORT_JSON_BYTES) {
      errorMessage.set(`文件过大，无法导入（上限 ${formatMiB(MAX_IMPORT_JSON_BYTES)}）`);
      setTimeout(() => errorMessage.set(null), 5000);
      return;
    }

    let jsonContent = '';
    try {
      jsonContent = await localFsService.readTextFile(grant.path, grant.accessToken);
    } catch (error) {
      const parsed = parseLocalFsTextTooLarge(error);
      if (parsed) {
        errorMessage.set(
          `文件过大，无法导入（大小 ${formatMiB(parsed.sizeBytes)}，上限 ${formatMiB(parsed.maxBytes)}）`,
        );
        setTimeout(() => errorMessage.set(null), 5000);
        return;
      }
      throw error;
    }
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
        const configToSave: Connection = sanitizeConnectionForImport({
          ...conn,
          id: crypto.randomUUID(),
        });

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
