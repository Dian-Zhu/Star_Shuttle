import { invoke } from '@tauri-apps/api/core';

type BackendOpenHandle = {
  handle_id: string;
  size: number;
};

type BackendFileStat = {
  size: number;
  access_token?: string | null;
};

type LocalFsDialogFilter = {
  name: string;
  extensions: string[];
};

type BackendDialogGrant = {
  path: string;
  access_token: string;
  size: number;
};

export type LocalFsDialogGrant = {
  path: string;
  accessToken: string;
  size: number;
};

class LocalReadHandle {
  constructor(
    private readonly handleId: string,
    private readonly sizeValue: number
  ) {}

  async read(buffer: Uint8Array): Promise<number> {
    const data = await invoke<number[]>('local_fs_read_chunk', {
      handleId: this.handleId,
      length: buffer.length,
    });
    const bytes = Uint8Array.from(data);
    buffer.set(bytes);
    return bytes.length;
  }

  async seek(offset: number, whence: number): Promise<number> {
    return invoke<number>('local_fs_seek', {
      handleId: this.handleId,
      offset,
      whence,
    });
  }

  async stat(): Promise<BackendFileStat> {
    return { size: this.sizeValue };
  }

  async close(): Promise<void> {
    await invoke('local_fs_close', { handleId: this.handleId });
  }
}

class LocalWriteHandle {
  constructor(private readonly handleId: string) {}

  async write(content: Uint8Array): Promise<number> {
    return invoke<number>('local_fs_write_chunk', {
      handleId: this.handleId,
      content: Array.from(content),
    });
  }

  async seek(offset: number, whence: number): Promise<number> {
    return invoke<number>('local_fs_seek', {
      handleId: this.handleId,
      offset,
      whence,
    });
  }

  async close(): Promise<void> {
    await invoke('local_fs_close', { handleId: this.handleId });
  }
}

export class LocalFsService {
  private normalizePath(path: string): string {
    return path.replace(/\\/g, '/');
  }

  async pickFileForRead(filters: LocalFsDialogFilter[] = []): Promise<LocalFsDialogGrant | null> {
    const grant = await invoke<BackendDialogGrant | null>('local_fs_pick_file_for_read', {
      filters,
    });
    if (!grant) return null;
    return {
      path: this.normalizePath(grant.path),
      accessToken: grant.access_token,
      size: grant.size,
    };
  }

  async pickFileForWrite(
    defaultFileName?: string,
    filters: LocalFsDialogFilter[] = [],
  ): Promise<LocalFsDialogGrant | null> {
    const grant = await invoke<BackendDialogGrant | null>('local_fs_pick_file_for_write', {
      defaultFileName: defaultFileName ?? null,
      filters,
    });
    if (!grant) return null;
    return {
      path: this.normalizePath(grant.path),
      accessToken: grant.access_token,
      size: grant.size,
    };
  }

  async openFile(path: string, accessToken?: string): Promise<LocalReadHandle> {
    try {
      const result = await invoke<BackendOpenHandle>('local_fs_open_read', {
        path: this.normalizePath(path),
        accessToken,
      });
      return new LocalReadHandle(result.handle_id, result.size);
    } catch (error: any) {
      throw new Error(`Failed to open file: ${error.message}`);
    }
  }

  async openWriteFile(path: string, truncate: boolean, accessToken?: string): Promise<LocalWriteHandle> {
    try {
      const handleId = await invoke<string>('local_fs_open_write', {
        path: this.normalizePath(path),
        truncate,
        accessToken,
      });
      return new LocalWriteHandle(handleId);
    } catch (error: any) {
      throw new Error(`Failed to open file for writing: ${error.message}`);
    }
  }

  async readChunk(handle: LocalReadHandle, length: number): Promise<Uint8Array> {
    try {
      const buffer = new Uint8Array(length);
      const bytesRead = await handle.read(buffer);
      if (bytesRead === 0) return new Uint8Array(0);
      if (bytesRead < length) {
        return buffer.slice(0, bytesRead);
      }
      return buffer;
    } catch (error: any) {
      throw new Error(`Failed to read chunk: ${error.message}`);
    }
  }

  async writeChunk(handle: LocalWriteHandle, content: Uint8Array): Promise<number> {
    try {
      return await handle.write(content);
    } catch (error: any) {
      throw new Error(`Failed to write chunk: ${error.message}`);
    }
  }

  async closeFile(handle: { close: () => Promise<void> }): Promise<void> {
    try {
      await handle.close();
    } catch (error: any) {
      console.warn(`Failed to close file: ${error.message}`);
    }
  }

  async readTextFile(path: string, accessToken?: string): Promise<string> {
    try {
      return await invoke<string>('local_fs_read_text', {
        path: this.normalizePath(path),
        accessToken,
      });
    } catch (error: any) {
      throw new Error(`Failed to read text file: ${error.message}`);
    }
  }

  async writeTextFile(path: string, content: string, accessToken?: string): Promise<void> {
    try {
      await invoke('local_fs_write_text', {
        path: this.normalizePath(path),
        content,
        accessToken,
      });
    } catch (error: any) {
      throw new Error(`Failed to write text file: ${error.message}`);
    }
  }

}

export const localFsService = new LocalFsService();
