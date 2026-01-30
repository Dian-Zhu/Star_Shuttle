import { writable, derived } from 'svelte/store';
import type { ITheme, Terminal } from '@xterm/xterm';
import type { FitAddon } from '@xterm/addon-fit';
import type { SearchAddon } from '@xterm/addon-search';

// 定义连接类型 (与后端结构匹配)
export interface Connection {
  id: string;
  name: string;
  protocol?: 'Ssh' | 'Rdp' | 'Telnet';
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

export interface SplitConfig {
  sessionId: string;
  mode: 'none' | 'horizontal' | 'vertical';
  splitRatio: number; // 0-1, 主面板占比
}

export type FileClipboardItem = {
  source: 'local' | 'remote';
  sessionId?: string;
  path: string;
  name: string;
  isDirectory: boolean;
  operation: 'copy';
};

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
export const fileClipboard = writable<FileClipboardItem | null>(null);
export const terminalSplitConfigs = writable<Map<string, SplitConfig>>(new Map());

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
  theme: 'dark' | 'light' | 'system';
  ui: {
    sidebarCollapsed: boolean;
  };
  appearance: {
    terminalTheme: 'auto' | 'dracula' | 'nord' | 'solarized-dark' | 'solarized-light' | 'monokai' | 'one-dark' | 'github-dark' | 'tokyo-night' | 'catppuccin' | 'custom';
    customTheme?: {
      background: string;
      foreground: string;
      cursor: string;
      selectionBackground: string;
      black: string;
      red: string;
      green: string;
      yellow: string;
      blue: string;
      magenta: string;
      cyan: string;
      white: string;
      brightBlack: string;
      brightRed: string;
      brightGreen: string;
      brightYellow: string;
      brightBlue: string;
      brightMagenta: string;
      brightCyan: string;
      brightWhite: string;
    };
  };
  terminal: {
    fontSize: number;
    fontFamily: string;
    cursorBlink: boolean;
    scrollback: number;
    cursorStyle: 'block' | 'underline' | 'bar';
  };
  connection: {
    autoReconnect: boolean;
  };
  shortcuts: {
    commandPalette: string;
    toggleSidebar: string;
    newConnection: string;
    settings: string;
    closeTerminal: string;
    prevTab: string;
    nextTab: string;
    copy: string;
    paste: string;
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
  appearance: {
    terminalTheme: 'auto',
  },
  terminal: {
    fontSize: 14,
    fontFamily: 'Menlo, Monaco, "Courier New", monospace',
    cursorBlink: true,
    scrollback: 5000,
    cursorStyle: 'block',
  },
  connection: {
    autoReconnect: false,
  },
  shortcuts: {
    commandPalette: 'Ctrl+Shift+P',
    toggleSidebar: 'Ctrl+B',
    newConnection: 'Ctrl+Shift+N',
    settings: 'Ctrl+Shift+S',
    closeTerminal: 'Ctrl+Shift+W',
    prevTab: 'Ctrl+Shift+[',
    nextTab: 'Ctrl+Shift+]',
    copy: 'Ctrl+Shift+C',
    paste: 'Ctrl+Shift+V',
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
      appearance: {
        ...defaultSettings.appearance,
        ...(parsed.appearance || {})
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
      'toggleSidebar',
      'newConnection',
      'settings',
      'closeTerminal',
      'prevTab',
      'nextTab',
      'copy',
      'paste'
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

export function getXtermTheme(appSettings: AppSettings): ITheme {
  const preset = appSettings.appearance?.terminalTheme ?? 'auto';

  // Custom theme
  if (preset === 'custom' && appSettings.appearance?.customTheme) {
    const custom = appSettings.appearance.customTheme;
    return {
      background: custom.background,
      foreground: custom.foreground,
      cursor: custom.cursor,
      selectionBackground: custom.selectionBackground,
      black: custom.black,
      red: custom.red,
      green: custom.green,
      yellow: custom.yellow,
      blue: custom.blue,
      magenta: custom.magenta,
      cyan: custom.cyan,
      white: custom.white,
      brightBlack: custom.brightBlack,
      brightRed: custom.brightRed,
      brightGreen: custom.brightGreen,
      brightYellow: custom.brightYellow,
      brightBlue: custom.brightBlue,
      brightMagenta: custom.brightMagenta,
      brightCyan: custom.brightCyan,
      brightWhite: custom.brightWhite,
    };
  }

  if (preset === 'dracula') {
    return {
      background: '#282a36',
      foreground: '#f8f8f2',
      cursor: '#bd93f9',
      selectionBackground: '#44475a',
      black: '#21222c',
      red: '#ff5555',
      green: '#50fa7b',
      yellow: '#f1fa8c',
      blue: '#6272a4',
      magenta: '#bd93f9',
      cyan: '#8be9fd',
      white: '#f8f8f2',
      brightBlack: '#6272a4',
      brightRed: '#ff6e6e',
      brightGreen: '#69ff94',
      brightYellow: '#ffffa5',
      brightBlue: '#d6acff',
      brightMagenta: '#ff92df',
      brightCyan: '#a4ffff',
      brightWhite: '#ffffff',
    };
  }

  if (preset === 'nord') {
    return {
      background: '#2e3440',
      foreground: '#d8dee9',
      cursor: '#88c0d0',
      selectionBackground: '#434c5e',
      black: '#3b4252',
      red: '#bf616a',
      green: '#a3be8c',
      yellow: '#ebcb8b',
      blue: '#81a1c1',
      magenta: '#b48ead',
      cyan: '#88c0d0',
      white: '#e5e9f0',
      brightBlack: '#4c566a',
      brightRed: '#bf616a',
      brightGreen: '#a3be8c',
      brightYellow: '#ebcb8b',
      brightBlue: '#81a1c1',
      brightMagenta: '#b48ead',
      brightCyan: '#8fbcbb',
      brightWhite: '#eceff4',
    };
  }

  if (preset === 'solarized-dark') {
    return {
      background: '#002b36',
      foreground: '#93a1a1',
      cursor: '#268bd2',
      selectionBackground: '#073642',
      black: '#073642',
      red: '#dc322f',
      green: '#859900',
      yellow: '#b58900',
      blue: '#268bd2',
      magenta: '#d33682',
      cyan: '#2aa198',
      white: '#eee8d5',
      brightBlack: '#002b36',
      brightRed: '#cb4b16',
      brightGreen: '#586e75',
      brightYellow: '#657b83',
      brightBlue: '#839496',
      brightMagenta: '#6c71c4',
      brightCyan: '#93a1a1',
      brightWhite: '#fdf6e3',
    };
  }

  if (preset === 'solarized-light') {
    return {
      background: '#fdf6e3',
      foreground: '#657b83',
      cursor: '#268bd2',
      selectionBackground: '#eee8d5',
      black: '#073642',
      red: '#dc322f',
      green: '#859900',
      yellow: '#b58900',
      blue: '#268bd2',
      magenta: '#d33682',
      cyan: '#2aa198',
      white: '#eee8d5',
      brightBlack: '#002b36',
      brightRed: '#cb4b16',
      brightGreen: '#586e75',
      brightYellow: '#657b83',
      brightBlue: '#839496',
      brightMagenta: '#6c71c4',
      brightCyan: '#93a1a1',
      brightWhite: '#fdf6e3',
    };
  }

  if (preset === 'monokai') {
    return {
      background: '#272822',
      foreground: '#f8f8f2',
      cursor: '#f92672',
      selectionBackground: '#49483e',
      black: '#1e1f1c',
      red: '#f92672',
      green: '#a6e22e',
      yellow: '#f4bf75',
      blue: '#66d9ef',
      magenta: '#ae81ff',
      cyan: '#a1efe4',
      white: '#f8f8f2',
      brightBlack: '#75715e',
      brightRed: '#f92672',
      brightGreen: '#a6e22e',
      brightYellow: '#f4bf75',
      brightBlue: '#66d9ef',
      brightMagenta: '#ae81ff',
      brightCyan: '#a1efe4',
      brightWhite: '#f9f8f5',
    };
  }

  if (preset === 'one-dark') {
    return {
      background: '#282c34',
      foreground: '#abb2bf',
      cursor: '#528bff',
      selectionBackground: '#3e4451',
      black: '#1e2127',
      red: '#e06c75',
      green: '#98c379',
      yellow: '#d19a66',
      blue: '#61afef',
      magenta: '#c678dd',
      cyan: '#56b6c2',
      white: '#abb2bf',
      brightBlack: '#5c6370',
      brightRed: '#e06c75',
      brightGreen: '#98c379',
      brightYellow: '#d19a66',
      brightBlue: '#61afef',
      brightMagenta: '#c678dd',
      brightCyan: '#56b6c2',
      brightWhite: '#ffffff',
    };
  }

  if (preset === 'github-dark') {
    return {
      background: '#0d1117',
      foreground: '#c9d1d9',
      cursor: '#58a6ff',
      selectionBackground: '#388bfd',
      black: '#484f58',
      red: '#ff7b72',
      green: '#3fb950',
      yellow: '#d29922',
      blue: '#58a6ff',
      magenta: '#bc8cff',
      cyan: '#39c5cf',
      white: '#b1bac4',
      brightBlack: '#6e7681',
      brightRed: '#ffa198',
      brightGreen: '#56d364',
      brightYellow: '#e3b341',
      brightBlue: '#79c0ff',
      brightMagenta: '#d2a8ff',
      brightCyan: '#56d4dd',
      brightWhite: '#f0f6fc',
    };
  }

  if (preset === 'tokyo-night') {
    return {
      background: '#1a1b26',
      foreground: '#a9b1d6',
      cursor: '#c0caf5',
      selectionBackground: '#2f354b',
      black: '#414868',
      red: '#f7768e',
      green: '#9ece6a',
      yellow: '#e0af68',
      blue: '#7aa2f7',
      magenta: '#bb9af7',
      cyan: '#7dcfff',
      white: '#c0caf5',
      brightBlack: '#565f89',
      brightRed: '#ff9e64',
      brightGreen: '#b9f27c',
      brightYellow: '#ff9e64',
      brightBlue: '#7aa2f7',
      brightMagenta: '#bb9af7',
      brightCyan: '#7dcfff',
      brightWhite: '#c0caf5',
    };
  }

  if (preset === 'catppuccin') {
    return {
      background: '#1e1e2e',
      foreground: '#cdd6f4',
      cursor: '#f5e0dc',
      selectionBackground: '#45475a',
      black: '#45475a',
      red: '#f38ba8',
      green: '#a6e3a1',
      yellow: '#f9e2af',
      blue: '#89b4fa',
      magenta: '#cba6f7',
      cyan: '#94e2d5',
      white: '#bac2de',
      brightBlack: '#585b70',
      brightRed: '#eba0ac',
      brightGreen: '#a6e3a1',
      brightYellow: '#f9e2af',
      brightBlue: '#89b4fa',
      brightMagenta: '#cba6f7',
      brightCyan: '#94e2d5',
      brightWhite: '#a6adc8',
    };
  }

  if (appSettings.theme === 'light') {
    return {
      background: '#ffffff',
      foreground: '#0f172a',
      cursor: '#2563eb',
      selectionBackground: '#e2e8f0',
      black: '#000000',
      red: '#ef4444',
      green: '#22c55e',
      yellow: '#eab308',
      blue: '#3b82f6',
      magenta: '#d946ef',
      cyan: '#06b6d4',
      white: '#64748b',
      brightBlack: '#94a3b8',
      brightRed: '#f87171',
      brightGreen: '#4ade80',
      brightYellow: '#facc15',
      brightBlue: '#60a5fa',
      brightMagenta: '#e879f9',
      brightCyan: '#22d3ee',
      brightWhite: '#f1f5f9',
    };
  }

  return {
    background: '#0f172a',
    foreground: '#e2e8f0',
    cursor: '#3b82f6',
    selectionBackground: '#334155',
  };
}

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

export type TerminalUiState = {
  order: string[];
  selectedSessionId: string | null;
};

function loadTerminalUiState(): TerminalUiState {
  if (typeof localStorage === 'undefined') return { order: [], selectedSessionId: null };
  try {
    const rawOrder = localStorage.getItem('terminalUi.order');
    const order = rawOrder ? (JSON.parse(rawOrder) as string[]) : [];
    const selected = localStorage.getItem('terminalUi.selectedSessionId');
    const selectedSessionId = selected ? selected : null;
    return {
      order: Array.isArray(order) ? order.map(String).filter(Boolean) : [],
      selectedSessionId: selectedSessionId ? String(selectedSessionId) : null,
    };
  } catch {
    return { order: [], selectedSessionId: null };
  }
}

export function getStoredTerminalUiState(): TerminalUiState {
  return loadTerminalUiState();
}

activeTerminals.subscribe(value => {
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('terminalUi.order', JSON.stringify(value.map(v => v.sessionId)));
  }
});

selectedTerminal.subscribe(value => {
  if (typeof localStorage !== 'undefined') {
    localStorage.setItem('terminalUi.selectedSessionId', value?.sessionId ?? '');
  }
});
