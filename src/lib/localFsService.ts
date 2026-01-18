import { readDir, readFile, writeFile, create, remove, rename } from '@tauri-apps/plugin-fs';
import { homeDir } from '@tauri-apps/api/path';
import type { FileEntry } from '../types';



export class LocalFsService {
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

      const entries = await readDir(actualPath);
      
      return await Promise.all(entries.map(async (entry) => {
        // Tauri's DirEntry has .path and .name
        const name = entry.name || '';
        const path = (entry as any).path || (actualPath === '.' ? name : `${actualPath}/${name}`);
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
      const content = await readFile(path);
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
      if (append) {
        // For append, we need to read existing content first
        const existing = await this.readFile(path).catch(() => new Uint8Array(0));
        const combined = new Uint8Array(existing.length + content.length);
        combined.set(existing);
        combined.set(content, existing.length);
        await writeFile(path, combined);
      } else {
        await writeFile(path, content);
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
      await create(path);
    } catch (error: any) {
      throw new Error(`Failed to create directory: ${error.message}`);
    }
  }

  /**
   * Remove file
   */
  async removeFile(path: string): Promise<void> {
    try {
      await remove(path);
    } catch (error: any) {
      throw new Error(`Failed to remove file: ${error.message}`);
    }
  }

  /**
   * Remove directory
   */
  async removeDirectory(path: string): Promise<void> {
    try {
      await remove(path, { recursive: true });
    } catch (error: any) {
      throw new Error(`Failed to remove directory: ${error.message}`);
    }
  }

  /**
   * Rename file or directory
   */
  async rename(oldPath: string, newPath: string): Promise<void> {
    try {
      await rename(oldPath, newPath);
    } catch (error: any) {
      throw new Error(`Failed to rename: ${error.message}`);
    }
  }

  /**
   * Get home directory path
   */
  async getHomeDir(): Promise<string> {
    return await homeDir();
  }
}

export const localFsService = new LocalFsService();
