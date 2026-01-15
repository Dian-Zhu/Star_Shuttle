import { writable, derived } from 'svelte/store';
import type { Terminal } from 'xterm';
import type { FitAddon } from 'xterm-addon-fit';

// 定义连接类型 (与后端结构匹配)
export interface Connection {
  id: string;
  name: string;
  host: string;
  port: number;
  username: string;
  auth_method: {
    Password?: {
      password: string;
      save_password: boolean;
    };
    PrivateKey?: {
      key_path: string;
      passphrase?: string;
      save_passphrase: boolean;
    };
    Agent?: {
      agent_path?: string;
    };
    Certificate?: {
      certificate_path: string;
      private_key_path: string;
      passphrase?: string;
      save_passphrase: boolean;
    };
  };
  description: string | null;
  tags: string[];
  created_at: string;
  updated_at: string;
  group_id: string | null;
}

export interface ActiveTerminal {
  sessionId: string;
  connection: Connection;
  terminal: Terminal;
  fitAddon: FitAddon;
}

// Stores
export const connections = writable<Connection[]>([]);
export const activeTerminals = writable<ActiveTerminal[]>([]);
export const selectedTerminalIndex = writable<number>(0);
export const showConnectionForm = writable<boolean>(false);
export const loading = writable<boolean>(false);
export const errorMessage = writable<string | null>(null);
export const successMessage = writable<string | null>(null);

// Derived store for selected terminal
export const selectedTerminal = derived(
  [activeTerminals, selectedTerminalIndex],
  ([$activeTerminals, $selectedTerminalIndex]) => {
    return $activeTerminals[$selectedTerminalIndex] || null;
  }
);
