import { invoke } from '@tauri-apps/api/core';
import type { CommandHistoryEntry } from '../types';

export class CommandHistoryService {
  async getRecent(limit = 500): Promise<CommandHistoryEntry[]> {
    try {
      return await invoke<CommandHistoryEntry[]>('get_command_history', { limit });
    } catch (error) {
      console.error('Failed to fetch command history:', error);
      throw error;
    }
  }

  async add(entry: CommandHistoryEntry): Promise<void> {
    try {
      await invoke('add_command_history', { entry });
    } catch (error) {
      // 记录历史失败不应影响正常终端输入，静默失败即可。
      console.error('Failed to add command history:', error);
    }
  }

  async clear(): Promise<void> {
    try {
      await invoke('clear_command_history');
    } catch (error) {
      console.error('Failed to clear command history:', error);
      throw error;
    }
  }

  async delete(id: string): Promise<void> {
    try {
      await invoke('delete_command_history', { id });
    } catch (error) {
      console.error(`Failed to delete command history ${id}:`, error);
      throw error;
    }
  }
}

export const commandHistoryService = new CommandHistoryService();
