import { readDir, readFile, writeFile, create, remove, rename, open } from '@tauri-apps/plugin-fs';
import { homeDir } from '@tauri-apps/api/path';
import type { FileEntry } from '../types';

export class LocalFsService {
  private normalizePath(path: string): string {
    return path.replace(/\\/g, '/');
  }

  /**
   * Open a file for reading
   */
  async openFile(path: string): Promise<any> {
    try {
      return await open(this.normalizePath(path), { read: true, write: false });
    } catch (error: any) {
      throw new Error(`Failed to open file: ${error.message}`);
    }
  }

  async openWriteFile(path: string, truncate: boolean): Promise<any> {
    try {
      return await open(this.normalizePath(path), {
        read: false,
        write: true,
        create: true,
        truncate
      });
    } catch (error: any) {
      throw new Error(`Failed to open file for writing: ${error.message}`);
    }
  }

  async readChunk(handle: any, length: number): Promise<Uint8Array> {
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

  async writeChunk(handle: any, content: Uint8Array): Promise<number> {
    try {
      return await handle.write(content);
    } catch (error: any) {
      throw new Error(`Failed to write chunk: ${error.message}`);
    }
  }

  async closeFile(handle: any): Promise<void> {
    try {
      await handle.close();
    } catch (error: any) {
      console.warn(`Failed to close file: ${error.message}`);
    }
  }

  /**
   * Get file size
   */
  async getFileSize(path: string): Promise<number> {
    try {
        const handle = await open(this.normalizePath(path), { read: true });
        const stat = await handle.stat();
        await handle.close();
        return stat.size;
    } catch (error: any) {
        // Fallback or rethrow
         throw new Error(`Failed to get file size: ${error.message}`);
    }
  }

  /**
   * List directory contents
   */
  async listDirectory(path: string): Promise<FileEntry[]> {
    try {
      // Handle special paths
      let actualPath = path;
      if (path === '~') {
        try {
          actualPath = await homeDir();
        } catch {
          actualPath = '.';
        }
      } else if (path === '' || path === '.') {
        actualPath = '.'; // Current directory
      }
      if (!actualPath) actualPath = '.';

      actualPath = this.normalizePath(actualPath);
      const normalizedActualPath = actualPath.replace(/\/+$/, '');

      const entries = await readDir(actualPath);
      
      return await Promise.all(entries.map(async (entry) => {
        // Tauri's DirEntry has .path and .name
        const name = entry.name || '';
        const path = this.normalizePath((entry as any).path || (actualPath === '.' ? name : `${normalizedActualPath}/${name}`));
        const isDirectory = (entry as any).isDirectory || false;
        // Size and modified time may not be available directly
        // We'll use defaults for now
        const size = 0;
        const modified = Date.now() / 1000;

        return {
          name,
          path,
          isDirectory,
          size,
          modified: new Date(modified * 1000),
          permissions: '644',
          owner: '',
          group: ''
        };
      }));
    } catch (error: any) {
      const message = error?.message ?? String(error);
      throw new Error(`Failed to list directory: ${message}`);
    }
  }

  /**
   * Read file content
   */
  async readFile(path: string): Promise<Uint8Array> {
    try {
      const content = await readFile(this.normalizePath(path));
      return new Uint8Array(content);
    } catch (error: any) {
      throw new Error(`Failed to read file: ${error.message}`);
    }
  }

  /**
   * Write file content
   */
  async writeFile(path: string, content: Uint8Array, append: boolean = false): Promise<void> {
    try {
      const normalizedPath = this.normalizePath(path);
      if (append) {
        // For append, we need to read existing content first
        const existing = await this.readFile(normalizedPath).catch(() => new Uint8Array(0));
        const combined = new Uint8Array(existing.length + content.length);
        combined.set(existing);
        combined.set(content, existing.length);
        await writeFile(normalizedPath, combined);
      } else {
        await writeFile(normalizedPath, content);
      }
    } catch (error: any) {
      throw new Error(`Failed to write file: ${error.message}`);
    }
  }

  /**
   * Create directory
   */
  async createDirectory(path: string): Promise<void> {
    try {
      await create(this.normalizePath(path));
    } catch (error: any) {
      throw new Error(`Failed to create directory: ${error.message}`);
    }
  }

  /**
   * Remove file
   */
  async removeFile(path: string): Promise<void> {
    try {
      await remove(this.normalizePath(path));
    } catch (error: any) {
      throw new Error(`Failed to remove file: ${error.message}`);
    }
  }

  /**
   * Remove directory
   */
  async removeDirectory(path: string): Promise<void> {
    try {
      await remove(this.normalizePath(path), { recursive: true });
    } catch (error: any) {
      throw new Error(`Failed to remove directory: ${error.message}`);
    }
  }

  /**
   * Rename file or directory
   */
  async rename(oldPath: string, newPath: string): Promise<void> {
    try {
      await rename(this.normalizePath(oldPath), this.normalizePath(newPath));
    } catch (error: any) {
      throw new Error(`Failed to rename: ${error.message}`);
    }
  }

  /**
   * Get home directory path
   */
  async getHomeDir(): Promise<string> {
    return this.normalizePath(await homeDir());
  }
}

export const localFsService = new LocalFsService();
