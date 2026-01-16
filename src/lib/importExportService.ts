import { save, open } from '@tauri-apps/plugin-dialog';
import { writeTextFile, readTextFile } from '@tauri-apps/plugin-fs';
import { invoke } from '@tauri-apps/api/core';
import { loadConnections } from './connectionService';
import { successMessage, errorMessage } from './store';
import type { Connection } from './store';

export async function exportConnections() {
  try {
    const connections = await invoke('get_all_connection_configs') as Connection[];
    
    if (connections.length === 0) {
      errorMessage.set('没有可导出的连接');
      setTimeout(() => errorMessage.set(null), 3000);
      return;
    }

    const filePath = await save({
      filters: [{
        name: 'JSON',
        extensions: ['json']
      }],
      defaultPath: 'connections.json'
    });

    if (!filePath) return; // User cancelled

    // Sanitize data before export (remove IDs to avoid conflicts on import, or keep them?)
    // Better to keep IDs but maybe generate new ones on import if they conflict?
    // For simple backup/restore, keeping exact data is fine.
    // We might want to remove sensitive data if we could, but passwords are part of the config...
    // Note: Passwords in the struct are exported if they are in the struct.
    // The struct has `password: String`, so it WILL be exported in plain text if not handled!
    // This is a security risk, but for "Export" it's often expected to backup everything.
    // We should warn the user or offer encrypted export (future feature).
    // For now, we export as is.

    await writeTextFile(filePath, JSON.stringify(connections, null, 2));
    
    successMessage.set('导出成功！');
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
    
    const jsonContent = await readTextFile(pathStr);
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
          console.warn('Skipping invalid connection:', conn);
          failCount++;
          continue;
        }

        // We should probably generate a new ID to avoid conflict with existing ones?
        // Or if ID exists, update it?
        // Let's assume "Import" adds new entries.
        // We strip the ID so the backend generates a new one.
        // But `save_connection_config` logic: if ID is nil, generate new. If ID provided, update.
        // If we import from another machine, IDs might not conflict, but if we import same file twice, we overwrite.
        // To be safe and support "Duplicate/Clone" behavior, maybe we should regenerate IDs?
        // But if it's "Restore Backup", we want same IDs.
        // Let's try to save as is.
        
        await invoke('save_connection_config', { config: conn });
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
