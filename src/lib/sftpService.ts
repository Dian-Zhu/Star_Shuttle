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

function normalizeRemotePath(path: string): string {
  if (!path) return '/';
  if ([...path].every((ch) => ch === '/')) return '/';
  return path.replace(/\/+/g, '/').replace(/\/+$/g, '');
}

function buildRemoteChildPath(parentPath: string, name: string): string {
  const normalizedParent = normalizeRemotePath(parentPath);
  return normalizedParent === '/'
    ? `/${name}`
    : `${normalizedParent}/${name}`;
}

function encodeHeaderPath(path: string): string {
  return encodeURIComponent(path);
}

export class SftpService {
  private decodeBinaryPayload(data: ArrayBuffer | number[]): Uint8Array {
    return data instanceof ArrayBuffer ? new Uint8Array(data) : Uint8Array.from(data);
  }

  private normalizeWriteOptions(
    options: boolean | { append?: boolean; offset?: number; truncate?: boolean }
  ): { append: boolean; offset?: number; truncate?: boolean } {
    if (typeof options === 'boolean') {
      return { append: options };
    }
    return {
      append: options.append ?? false,
      offset: options.offset,
      truncate: options.truncate,
    };
  }

  async listDirectory(sessionId: string, path: string): Promise<FileEntry[]> {
    const entries = await invoke<BackendFileEntry[]>('sftp_ls', { sessionId, path });
    return entries.map(e => ({
      name: e.name,
      path: buildRemoteChildPath(path, e.name),
      isDirectory: e.is_dir,
      size: e.size,
      modified: new Date(e.modified * 1000),
      permissions: (e.permissions & 0o7777).toString(8),
      owner: e.owner,
      group: e.group
    }));
  }

  async readFile(sessionId: string, path: string): Promise<Uint8Array> {
    const data = await invoke<ArrayBuffer | number[]>('sftp_read', undefined, {
      headers: {
        'session-id': sessionId,
        path: encodeHeaderPath(path)
      }
    });
    return this.decodeBinaryPayload(data);
  }

  async readChunk(sessionId: string, path: string, offset: number, length: number): Promise<Uint8Array> {
    const data = await invoke<ArrayBuffer | number[]>('sftp_read_chunk', undefined, {
      headers: {
        'session-id': sessionId,
        path: encodeHeaderPath(path),
        offset: String(offset),
        length: String(length)
      }
    });
    return this.decodeBinaryPayload(data);
  }

  async writeFile(
    sessionId: string,
    path: string,
    content: Uint8Array,
    options: boolean | { append?: boolean; offset?: number; truncate?: boolean } = false
  ): Promise<void> {
    const normalized = this.normalizeWriteOptions(options);
    await invoke('sftp_write', content, {
      headers: {
        'session-id': sessionId,
        path: encodeHeaderPath(path),
        append: String(normalized.append),
        ...(normalized.offset !== undefined ? { offset: String(normalized.offset) } : {}),
        ...(normalized.truncate !== undefined ? { truncate: String(normalized.truncate) } : {}),
      }
    });
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

  async scpUpload(sessionId: string, remotePath: string, content: Uint8Array): Promise<void> {
    await invoke('scp_upload', content, {
      headers: {
        'session-id': sessionId,
        'remote-path': encodeHeaderPath(remotePath)
      }
    });
  }

  async scpDownload(sessionId: string, remotePath: string): Promise<Uint8Array> {
    const data = await invoke<ArrayBuffer | number[]>('scp_download', undefined, {
      headers: {
        'session-id': sessionId,
        'remote-path': encodeHeaderPath(remotePath)
      }
    });
    return this.decodeBinaryPayload(data);
  }
}

export const __sftpServiceTestHooks = {
  normalizeRemotePath,
  buildRemoteChildPath,
};

export const sftpService = new SftpService();
