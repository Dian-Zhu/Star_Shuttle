import { invoke } from '@tauri-apps/api/core';
import type { FileEntry } from '../types';

// Backend response type matching Rust struct
interface BackendFileEntry {
  name: string;
  is_dir: boolean;
  size: number;
  modified: number;
  permissions: number;
}

export class SftpService {
  async listDirectory(sessionId: string, path: string): Promise<FileEntry[]> {
    const entries = await invoke<BackendFileEntry[]>('sftp_ls', { sessionId, path });
    return entries.map(e => ({
      name: e.name,
      path: path === '/' || path === '' ? `/${e.name}` : `${path}/${e.name}`.replace('//', '/'),
      isDirectory: e.is_dir,
      size: e.size,
      modified: new Date(e.modified * 1000),
      permissions: e.permissions.toString(8), // Convert to octal string representation
      owner: '', // Not provided by backend yet
      group: ''  // Not provided by backend yet
    }));
  }

  async readFile(sessionId: string, path: string): Promise<Uint8Array> {
    const data = await invoke<number[]>('sftp_read', { sessionId, path });
    return new Uint8Array(data);
  }

  async writeFile(sessionId: string, path: string, content: Uint8Array, append: boolean = false): Promise<void> {
    await invoke('sftp_write', { sessionId, path, content: Array.from(content), append });
  }

  async createDirectory(sessionId: string, path: string): Promise<void> {
    await invoke('sftp_mkdir', { sessionId, path });
  }

  async removeFile(sessionId: string, path: string): Promise<void> {
    await invoke('sftp_rm', { sessionId, path });
  }

  async removeDirectory(sessionId: string, path: string): Promise<void> {
    await invoke('sftp_rmdir', { sessionId, path });
  }

  async rename(sessionId: string, oldPath: string, newPath: string): Promise<void> {
    await invoke('sftp_rename', { sessionId, oldPath, newPath });
  }
}

export const sftpService = new SftpService();
