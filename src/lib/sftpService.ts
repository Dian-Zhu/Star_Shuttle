import { invoke } from '@tauri-apps/api/core';
import type { FileEntry } from '../types';

// Backend response type matching Rust struct
interface BackendFileEntry {
  name: string;
  is_dir: boolean;
  size: number;
  modified: number;
  permissions: number;
  owner: string;
  group: string;
}

export class SftpService {
  async listDirectory(sessionId: string, path: string): Promise<FileEntry[]> {
    const entries = await invoke<BackendFileEntry[]>('sftp_ls', { session_id: sessionId, path });
    return entries.map(e => ({
      name: e.name,
      path: path === '/' || path === '' ? `/${e.name}` : `${path}/${e.name}`.replace('//', '/'),
      isDirectory: e.is_dir,
      size: e.size,
      modified: new Date(e.modified * 1000),
      permissions: (e.permissions & 0o7777).toString(8),
      owner: e.owner,
      group: e.group
    }));
  }

  async readFile(sessionId: string, path: string): Promise<Uint8Array> {
    const data = await invoke<number[]>('sftp_read', { session_id: sessionId, path });
    return new Uint8Array(data);
  }

  async writeFile(sessionId: string, path: string, content: Uint8Array, append: boolean = false): Promise<void> {
    await invoke('sftp_write', { session_id: sessionId, path, content: Array.from(content), append });
  }

  async createDirectory(sessionId: string, path: string): Promise<void> {
    await invoke('sftp_mkdir', { session_id: sessionId, path });
  }

  async removeFile(sessionId: string, path: string): Promise<void> {
    await invoke('sftp_rm', { session_id: sessionId, path });
  }

  async removeDirectory(sessionId: string, path: string): Promise<void> {
    await invoke('sftp_rmdir', { session_id: sessionId, path });
  }

  async rename(sessionId: string, oldPath: string, newPath: string): Promise<void> {
    await invoke('sftp_rename', { session_id: sessionId, old_path: oldPath, new_path: newPath });
  }

  async scpUpload(sessionId: string, remotePath: string, content: Uint8Array): Promise<void> {
    await invoke('scp_upload', {
      session_id: sessionId,
      remote_path: remotePath,
      content: Array.from(content),
    });
  }

  async scpDownload(sessionId: string, remotePath: string): Promise<Uint8Array> {
    const data = await invoke<number[]>('scp_download', { session_id: sessionId, remote_path: remotePath });
    return new Uint8Array(data);
  }
}

export const sftpService = new SftpService();
