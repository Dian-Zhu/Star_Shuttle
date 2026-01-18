import { writable, derived } from 'svelte/store';
import type { Terminal } from 'xterm';
import type { FitAddon } from 'xterm-addon-fit';
import type { SearchAddon } from 'xterm-addon-search';

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
    KeyboardInteractive?: Record<string, never>;
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
  local_forwards?: { local_host: string; local_port: number; remote_host: string; remote_port: number }[];
  remote_forwards?: { remote_host: string; remote_port: number; local_host: string; local_port: number }[];
  proxy_type?: any;
  socks_proxy_port?: number | null;
}

export interface ConnectionGroup {
  id: string;
  name: string;
  createdAt: number;
}

export interface HistoryItem {
  connection: Connection;
  lastConnected: number; // timestamp
}

export interface ActiveTerminal {
  sessionId: string;
  connection: Connection;
  terminal: Terminal;
  fitAddon: FitAddon;
  searchAddon: SearchAddon;
}

// Stores
export const connections = writable<Connection[]>([]);
export const activeTerminals = writable<ActiveTerminal[]>([]);
export const selectedTerminalIndex = writable<number>(0);
export const broadcastInputEnabled = writable<boolean>(false);
export const broadcastSessionIds = writable<string[]>([]);
export const showConnectionForm = writable<boolean>(false);
export const editingConnection = writable<Connection | null>(null);
export const showSettings = writable<boolean>(false);
export const showAdvancedModal = writable<boolean>(false);
export const showCommandPalette = writable<boolean>(false);
export const isLocked = writable<boolean>(false);
export const hasAppLock = writable<boolean>(false);
export const loading = writable<boolean>(false);
export const errorMessage = writable<string | null>(null);
export const successMessage = writable<string | null>(null);

// History Store
const loadHistory = (): HistoryItem[] => {
  if (typeof localStorage === 'undefined') return [];
  try {
    const stored = localStorage.getItem('connectionHistory');
    return stored ? JSON.parse(stored) : [];
  } catch (e) {
    console.error('Failed to parse history:', e);
    return [];
  }
};

export const connectionHistory = writable<HistoryItem[]>(loadHistory());

connectionHistory.subscribe(value => {
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('connectionHistory', JSON.stringify(value));
  }
});

const loadGroups = (): ConnectionGroup[] => {
  if (typeof localStorage === 'undefined') return [];
  try {
    const stored = localStorage.getItem('connectionGroups');
    const parsed = stored ? (JSON.parse(stored) as ConnectionGroup[]) : [];
    if (Array.isArray(parsed) && parsed.length > 0) {
      return parsed;
    }
  } catch (e) {
    console.error('Failed to parse connection groups:', e);
  }
  return [{ id: '00000000-0000-0000-0000-000000000000', name: '默认', createdAt: Date.now() }];
};

export const connectionGroups = writable<ConnectionGroup[]>(loadGroups());

connectionGroups.subscribe(value => {
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('connectionGroups', JSON.stringify(value));
  }
});

// Global Settings Store
export interface AppSettings {
  theme: 'dark' | 'light';
  ui: {
    sidebarCollapsed: boolean;
  };
  terminal: {
    fontSize: number;
    fontFamily: string;
    cursorBlink: boolean;
  };
  connection: {
    autoReconnect: boolean;
  };
  shortcuts: {
    commandPalette: string;
    newConnection: string;
    settings: string;
    closeTerminal: string;
    prevTab: string;
    nextTab: string;
  };
  security: {
    autoLockMinutes: number; // 0 = disabled
    lockOnBlur: boolean;
  };
}

const defaultSettings: AppSettings = {
  theme: 'dark',
  ui: {
    sidebarCollapsed: false,
  },
  terminal: {
    fontSize: 14,
    fontFamily: 'Menlo, Monaco, "Courier New", monospace',
    cursorBlink: true,
  },
  connection: {
    autoReconnect: false,
  },
  shortcuts: {
    commandPalette: 'Ctrl+Shift+P',
    newConnection: 'Ctrl+Shift+N',
    settings: 'Ctrl+Shift+S',
    closeTerminal: 'Ctrl+Shift+W',
    prevTab: 'Ctrl+Shift+[',
    nextTab: 'Ctrl+Shift+]',
  },
  security: {
    autoLockMinutes: 0,
    lockOnBlur: false,
  }
};

// Load settings from localStorage with fallback
const loadSettings = (): AppSettings => {
  if (typeof localStorage === 'undefined') return defaultSettings;
  
  const stored = localStorage.getItem('appSettings');
  if (!stored) return defaultSettings;

  try {
    const parsed = JSON.parse(stored);
    // Merge with defaults to ensure all fields exist
    const merged: AppSettings = {
      ...defaultSettings,
      ...parsed,
      ui: {
        ...defaultSettings.ui,
        ...(parsed.ui || {})
      },
      terminal: {
        ...defaultSettings.terminal,
        ...(parsed.terminal || {})
      },
      connection: {
        ...defaultSettings.connection,
        ...(parsed.connection || {})
      },
      shortcuts: {
        ...defaultSettings.shortcuts,
        ...(parsed.shortcuts || {})
      },
      security: {
        ...defaultSettings.security,
        ...(parsed.security || {})
      }
    };

    const shortcutOrder: Array<keyof AppSettings['shortcuts']> = [
      'commandPalette',
      'newConnection',
      'settings',
      'closeTerminal',
      'prevTab',
      'nextTab'
    ];
    const seen = new Map<string, keyof AppSettings['shortcuts']>();
    const sanitizedShortcuts = { ...merged.shortcuts };
    for (const key of shortcutOrder) {
      const raw = sanitizedShortcuts[key];
      const value = typeof raw === 'string' ? raw.trim() : '';
      if (!value) {
        sanitizedShortcuts[key] = '';
        continue;
      }
      const normalized = value.toLowerCase().replace(/\s+/g, '');
      const existing = seen.get(normalized);
      if (existing) {
        sanitizedShortcuts[key] = '';
        continue;
      }
      sanitizedShortcuts[key] = value;
      seen.set(normalized, key);
    }
    merged.shortcuts = sanitizedShortcuts;
    return merged;
  } catch (e) {
    console.error('Failed to parse settings:', e);
    return defaultSettings;
  }
};

export const settings = writable<AppSettings>(loadSettings());

// Create a derived store for backward compatibility or ease of use if needed, 
// but we should prefer using settings directly.
// For now, let's keep isSidebarCollapsed as a derived store that can also be set? 
// No, let's replace usages of isSidebarCollapsed with settings.
export const isSidebarCollapsed = {
    subscribe: (run: (value: boolean) => void) => {
        return settings.subscribe(s => run(s.ui.sidebarCollapsed));
    },
    update: (fn: (value: boolean) => boolean) => {
        settings.update(s => ({
            ...s,
            ui: { ...s.ui, sidebarCollapsed: fn(s.ui.sidebarCollapsed) }
        }));
    },
    set: (value: boolean) => {
        settings.update(s => ({
            ...s,
            ui: { ...s.ui, sidebarCollapsed: value }
        }));
    }
};

// Persist settings changes
settings.subscribe(value => {
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('appSettings', JSON.stringify(value));
  }
});

// Derived store for selected terminal
export const selectedTerminal = derived(
  [activeTerminals, selectedTerminalIndex],
  ([$activeTerminals, $selectedTerminalIndex]) => {
    return $activeTerminals[$selectedTerminalIndex] || null;
  }
);
