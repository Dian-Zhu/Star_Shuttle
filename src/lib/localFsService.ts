import { invoke } from '@tauri-apps/api/core';

type BackendOpenHandle = {
  handle_id: string;
  size: number;
};

type BackendFileStat = {
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

  async openFile(path: string): Promise<LocalReadHandle> {
    try {
      const result = await invoke<BackendOpenHandle>('local_fs_open_read', {
        path: this.normalizePath(path),
      });
      return new LocalReadHandle(result.handle_id, result.size);
    } catch (error: any) {
      throw new Error(`Failed to open file: ${error.message}`);
    }
  }

  async openWriteFile(path: string, truncate: boolean): Promise<LocalWriteHandle> {
    try {
      const handleId = await invoke<string>('local_fs_open_write', {
        path: this.normalizePath(path),
        truncate,
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

  async getFileSize(path: string): Promise<number> {
    try {
      const stat = await invoke<BackendFileStat>('local_fs_stat', {
        path: this.normalizePath(path),
      });
      return stat.size;
    } catch (error: any) {
      throw new Error(`Failed to get file size: ${error.message}`);
    }
  }

  async readFile(path: string): Promise<Uint8Array> {
    const handle = await this.openFile(path);
    try {
      const stat = await handle.stat();
      const size = stat.size;
      const chunks: Uint8Array[] = [];
      let remaining = size;
      const chunkSize = 128 * 1024;

      while (remaining > 0) {
        const chunk = await this.readChunk(handle, Math.min(chunkSize, remaining));
        if (chunk.length === 0) break;
        chunks.push(chunk);
        remaining -= chunk.length;
      }

      const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
      const content = new Uint8Array(total);
      let offset = 0;
      for (const chunk of chunks) {
        content.set(chunk, offset);
        offset += chunk.length;
      }
      return content;
    } catch (error: any) {
      throw new Error(`Failed to read file: ${error.message}`);
    } finally {
      await this.closeFile(handle);
    }
  }

  async writeFile(path: string, content: Uint8Array, append: boolean = false): Promise<void> {
    try {
      const normalizedPath = this.normalizePath(path);
      const handle = await this.openWriteFile(normalizedPath, !append);
      try {
        if (append) {
          await handle.seek(0, 2);
        }
        await this.writeChunk(handle, content);
      } finally {
        await this.closeFile(handle);
      }
    } catch (error: any) {
      throw new Error(`Failed to write file: ${error.message}`);
    }
  }

  async readTextFile(path: string): Promise<string> {
    try {
      return await invoke<string>('local_fs_read_text', {
        path: this.normalizePath(path),
      });
    } catch (error: any) {
      throw new Error(`Failed to read text file: ${error.message}`);
    }
  }

  async writeTextFile(path: string, content: string): Promise<void> {
    try {
      await invoke('local_fs_write_text', {
        path: this.normalizePath(path),
        content,
      });
    } catch (error: any) {
      throw new Error(`Failed to write text file: ${error.message}`);
    }
  }

}

export const localFsService = new LocalFsService();
