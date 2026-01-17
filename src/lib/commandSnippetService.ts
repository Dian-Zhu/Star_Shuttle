import { invoke } from '@tauri-apps/api/core';
import type { CommandSnippet } from '../types';

export class CommandSnippetService {
  async getAll(): Promise<CommandSnippet[]> {
    try {
      const snippets = await invoke<CommandSnippet[]>('get_command_snippets');
      return snippets;
    } catch (error) {
      console.error('Failed to fetch command snippets:', error);
      throw error;
    }
  }

  async getById(id: string): Promise<CommandSnippet | null> {
    try {
      const snippet = await invoke<CommandSnippet | null>('get_command_snippet_by_id', { id });
      return snippet;
    } catch (error) {
      console.error(`Failed to fetch command snippet ${id}:`, error);
      throw error;
    }
  }

  async save(snippet: CommandSnippet): Promise<void> {
    try {
      await invoke('save_command_snippet', { snippet });
    } catch (error) {
      console.error('Failed to save command snippet:', error);
      throw error;
    }
  }

  async delete(id: string): Promise<void> {
    try {
      await invoke('delete_command_snippet', { id });
    } catch (error) {
      console.error(`Failed to delete command snippet ${id}:`, error);
      throw error;
    }
  }

  async incrementUsage(id: string): Promise<void> {
    try {
      await invoke('increment_command_snippet_usage', { id });
    } catch (error) {
      console.error(`Failed to increment usage count for ${id}:`, error);
      // Silent fail, not critical
    }
  }

  async search(query: string): Promise<CommandSnippet[]> {
    const all = await this.getAll();
    const q = query.toLowerCase();
    return all.filter(
      snippet =>
        snippet.name.toLowerCase().includes(q) ||
        snippet.command.toLowerCase().includes(q) ||
        snippet.description?.toLowerCase().includes(q) ||
        snippet.category?.toLowerCase().includes(q) ||
        snippet.tags?.toLowerCase().includes(q)
    );
  }

  async getCategories(): Promise<string[]> {
    const all = await this.getAll();
    const categories = all.map(s => s.category).filter(Boolean) as string[];
    return [...new Set(categories)];
  }
}

export const commandSnippetService = new CommandSnippetService();