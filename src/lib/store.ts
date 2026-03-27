import { writable, derived } from 'svelte/store';
import type { ITheme, Terminal } from 'xterm';
import type { FitAddon } from 'xterm-addon-fit';
import type { SearchAddon } from 'xterm-addon-search';
import { normalizeShortcut } from './shortcuts';

export type ConnectionAuthMethod = {
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

export type ConnectionProxyType =
  | 'None'
  | {
      Socks5: {
        host: string;
        port: number;
        username: string | null;
        password: string | null;
      };
    }
  | {
      Http: {
        host: string;
        port: number;
        username: string | null;
        password: string | null;
      };
    }
  | {
      JumpHost: {
        host: string;
        port: number;
        username: string;
        auth_method: ConnectionAuthMethod;
      };
    };

// 定义连接类型 (与后端结构匹配)
export interface Connection {
  id: string;
  name: string;
  protocol?: 'Ssh' | 'Rdp' | 'Telnet';
  host: string;
  port: number;
  username: string;
  auth_method: ConnectionAuthMethod;
  description: string | null;
  tags: string[];
  created_at: string;
  updated_at: string;
  group_id: string | null;
  local_forwards?: { local_host: string; local_port: number; remote_host: string; remote_port: number }[];
  remote_forwards?: { remote_host: string; remote_port: number; local_host: string; local_port: number }[];
  proxy_type?: ConnectionProxyType | Record<string, unknown>;
  socks_proxy_port?: number | null;
  auto_reconnect?: boolean;
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
  fileExplorerPath?: string; // Add this line
  parentId?: string; // ID of the root session if this is a split pane
}

export interface SplitConfig {
  sessionId: string;
  mode: 'none' | 'horizontal' | 'vertical';
  splitRatio: number; // 0-1, 主面板占比
}

export type FileClipboardItem = {
  source: 'remote';
  sessionId?: string;
  entries: { path: string; name: string; isDirectory: boolean }[];
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
export const connectingConnections = writable<Set<string>>(new Set());
export const errorMessage = writable<string | null>(null);
export const successMessage = writable<string | null>(null);
export const fileClipboard = writable<FileClipboardItem | null>(null);
export const terminalSplitConfigs = writable<Map<string, SplitConfig>>(new Map());
// Map root sessionId -> Set of all child sessionIds (including root itself)
export const terminalSessionMap = writable<Map<string, Set<string>>>(new Map());
export const closeSplitRequest = writable<string | null>(null);

let errorMessageTimer: ReturnType<typeof setTimeout> | null = null;
let successMessageTimer: ReturnType<typeof setTimeout> | null = null;
let errorMessageVersion = 0;
let successMessageVersion = 0;

export function clearErrorMessage(): void {
  errorMessageVersion += 1;
  if (errorMessageTimer) {
    clearTimeout(errorMessageTimer);
    errorMessageTimer = null;
  }
  errorMessage.set(null);
}

export function clearSuccessMessage(): void {
  successMessageVersion += 1;
  if (successMessageTimer) {
    clearTimeout(successMessageTimer);
    successMessageTimer = null;
  }
  successMessage.set(null);
}

export function showErrorMessage(message: string, timeoutMs = 5000): void {
  errorMessageVersion += 1;
  const currentVersion = errorMessageVersion;
  if (errorMessageTimer) {
    clearTimeout(errorMessageTimer);
    errorMessageTimer = null;
  }
  errorMessage.set(message);
  if (timeoutMs > 0) {
    errorMessageTimer = setTimeout(() => {
      if (errorMessageVersion === currentVersion) {
        errorMessage.set(null);
        errorMessageTimer = null;
      }
    }, timeoutMs);
  }
}

export function showSuccessMessage(message: string, timeoutMs = 3000): void {
  successMessageVersion += 1;
  const currentVersion = successMessageVersion;
  if (successMessageTimer) {
    clearTimeout(successMessageTimer);
    successMessageTimer = null;
  }
  successMessage.set(message);
  if (timeoutMs > 0) {
    successMessageTimer = setTimeout(() => {
      if (successMessageVersion === currentVersion) {
        successMessage.set(null);
        successMessageTimer = null;
      }
    }, timeoutMs);
  }
}

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
  theme: 'dark' | 'light' | 'system' | 'custom';
  ui: {
    sidebarCollapsed: boolean;
    rightSidebarOpen: boolean;
    rightSidebarWidth: number;
  };
  appearance: {
    terminalTheme: 'auto' | 'dracula' | 'nord' | 'solarized-dark' | 'solarized-light' | 'monokai' | 'one-dark' | 'github-dark' | 'tokyo-night' | 'catppuccin' | 'custom';
    customTheme?: ITheme;
    customUITheme?: {
      backgroundColor: string;
      surfaceColor: string;
      statusBarColor: string;
      surfaceLightColor: string;
      textColor: string;
      secondaryTextColor: string;
      borderColor: string;
      borderLightColor: string;
    };
    accentColor?: string;
    backgroundImage?: string | null;
    backgroundOpacity?: number; // 0-1, 默认 0.5
    backgroundBlur?: number; // 0-20, 默认 0
    ansiColorPreset: 'classic' | 'standard-light' | 'solarized' | 'nord-light' | 'monokai' | 'gruvbox' | 'github-light' | 'solarized-light' | 'tango-light' | 'smart' | 'custom';
    customAnsiColors?: {
      foreground: string;
      red: string;
      green: string;
      yellow: string;
      blue: string;
      magenta: string;
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
    toggleFileBrowser: string;
    newConnection: string;
    settings: string;
    closeTerminal: string;
    prevTab: string;
    nextTab: string;
    copy: string;
    paste: string;
    fileBrowserRefresh: string;
    fileBrowserNewFolder: string;
    fileBrowserNewFile: string;
    fileBrowserRename: string;
    fileBrowserDelete: string;
    fileBrowserSelectAll: string;
    fileBrowserOpen: string;
    fileBrowserBack: string;
  };
  security: {
    autoLockMinutes: number; // 0 = disabled
    lockOnBlur: boolean;
    disableDevToolsShortcuts: boolean;
  };
}

const defaultSettings: AppSettings = {
  theme: 'dark',
  ui: {
    sidebarCollapsed: false,
    rightSidebarOpen: false,
    rightSidebarWidth: 400,
  },
  appearance: {
    terminalTheme: 'auto',
    accentColor: 'blue',
    ansiColorPreset: 'classic',
    customUITheme: {
      backgroundColor: '#0f172a',
      surfaceColor: '#1e293b',
      statusBarColor: '#1e293b',
      surfaceLightColor: '#334155',
      textColor: '#f8fafc',
      secondaryTextColor: '#94a3b8',
      borderColor: '#334155',
      borderLightColor: '#475569',
    }
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
    toggleFileBrowser: 'Ctrl+Shift+B',
    newConnection: 'Ctrl+Shift+N',
    settings: 'Ctrl+Shift+S',
    closeTerminal: 'Ctrl+Shift+W',
    prevTab: 'Ctrl+Shift+[',
    nextTab: 'Ctrl+Shift+]',
    copy: 'Ctrl+Shift+C',
    paste: 'Ctrl+Shift+V',
    fileBrowserRefresh: 'F5',
    fileBrowserNewFolder: 'F7',
    fileBrowserNewFile: 'Ctrl+Alt+N',
    fileBrowserRename: 'F2',
    fileBrowserDelete: 'Delete',
    fileBrowserSelectAll: 'Ctrl+A',
    fileBrowserOpen: 'Enter',
    fileBrowserBack: 'Backspace',
  },
  security: {
    autoLockMinutes: 0,
    lockOnBlur: false,
    disableDevToolsShortcuts: true,
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
    'toggleFileBrowser',
    'newConnection',
      'settings',
      'closeTerminal',
      'prevTab',
      'nextTab',
      'copy',
      'paste',
      'fileBrowserRefresh',
      'fileBrowserNewFolder',
      'fileBrowserNewFile',
      'fileBrowserRename',
      'fileBrowserDelete',
      'fileBrowserSelectAll',
      'fileBrowserOpen',
      'fileBrowserBack'
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
      const parsed = normalizeShortcut(value);
      if ('error' in parsed || !parsed.value) {
        sanitizedShortcuts[key] = '';
        continue;
      }
      const normalized = parsed.value.toLowerCase().replace(/\s+/g, '');
      const existing = seen.get(normalized);
      if (existing) {
        sanitizedShortcuts[key] = '';
        continue;
      }
      sanitizedShortcuts[key] = parsed.value;
      seen.set(normalized, key);
    }
    merged.shortcuts = sanitizedShortcuts;
    return merged;
  } catch (e) {
    console.error('Failed to parse settings:', e);
    return defaultSettings;
  }
};

export function getBaseXtermTheme(appSettings: AppSettings): ITheme {
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

// ANSI Color Presets
const ansiColorPresets: Record<AppSettings['appearance']['ansiColorPreset'], {
  foreground: string;
  background: string;
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
}> = {
  classic: {
    foreground: '#ffffff',
    background: '#000000',
    black: '#000000',
    red: '#cd0000',
    green: '#00cd00',
    yellow: '#cdcd00',
    blue: '#0000ee',
    magenta: '#cd00cd',
    cyan: '#00cdcd',
    white: '#f8fafc',
    brightBlack: '#7f7f7f',
    brightRed: '#ff0000',
    brightGreen: '#00ff00',
    brightYellow: '#ffff00',
    brightBlue: '#5c5cff',
    brightMagenta: '#ff00ff',
    brightCyan: '#00ffff',
    brightWhite: '#ffffff',
  },
  solarized: {
    foreground: '#839496',
    background: '#002b36',
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
  },
  'standard-light': {
    foreground: '#333333',
    background: '#ffffff',
    black: '#000000',
    red: '#cd3131',
    green: '#0dbc79',
    yellow: '#949800',
    blue: '#2472c8',
    magenta: '#bc3fbc',
    cyan: '#11a8cd',
    white: '#555555',
    brightBlack: '#666666',
    brightRed: '#f14c4c',
    brightGreen: '#23d18b',
    brightYellow: '#f5f543',
    brightBlue: '#3b8eea',
    brightMagenta: '#d670d6',
    brightCyan: '#29b8db',
    brightWhite: '#e5e5e5',
  },
  'nord-light': {
    foreground: '#d8dee9',
    background: '#2e3440',
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
  },
  monokai: {
    foreground: '#f8f8f2',
    background: '#272822',
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
  },
  gruvbox: {
    foreground: '#ebdbb2',
    background: '#282828',
    black: '#282828',
    red: '#cc241d',
    green: '#98971a',
    yellow: '#d79921',
    blue: '#458588',
    magenta: '#b16286',
    cyan: '#689d6a',
    white: '#a89984',
    brightBlack: '#928374',
    brightRed: '#fb4934',
    brightGreen: '#b8bb26',
    brightYellow: '#fabd2f',
    brightBlue: '#83a598',
    brightMagenta: '#d3869b',
    brightCyan: '#8ec07c',
    brightWhite: '#ebdbb2',
  },
  'github-light': {
    foreground: '#24292f',
    background: '#ffffff',
    black: '#24292f',
    red: '#cf222e',
    green: '#1a7f37',
    yellow: '#9a6700',
    blue: '#0969da',
    magenta: '#8250df',
    cyan: '#1b7c83',
    white: '#6e7781',
    brightBlack: '#57606a',
    brightRed: '#a40e26',
    brightGreen: '#1a7f37',
    brightYellow: '#633c01',
    brightBlue: '#218bff',
    brightMagenta: '#a475f9',
    brightCyan: '#3192aa',
    brightWhite: '#8c959f',
  },
  'solarized-light': {
    foreground: '#657b83',
    background: '#fdf6e3',
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
  },
  'tango-light': {
    foreground: '#2e3436',
    background: '#eeeeec',
    black: '#2e3436',
    red: '#cc0000',
    green: '#4e9a06',
    yellow: '#c4a000',
    blue: '#3465a4',
    magenta: '#75507b',
    cyan: '#06989a',
    white: '#d3d7cf',
    brightBlack: '#555753',
    brightRed: '#ef2929',
    brightGreen: '#8ae234',
    brightYellow: '#fce94f',
    brightBlue: '#729fcf',
    brightMagenta: '#ad7fa8',
    brightCyan: '#34e2e2',
    brightWhite: '#eeeeec',
  },
  smart: {
    foreground: '#e0e0e0',
    background: 'rgba(0,0,0,0)',
    black: '#1a1a1a',
    red: '#ff5252',
    green: '#69f0ae',
    yellow: '#ffd740',
    blue: '#448aff',
    magenta: '#e040fb',
    cyan: '#18ffff',
    white: '#ffffff',
    brightBlack: '#555555',
    brightRed: '#ff80ab',
    brightGreen: '#b9f6ca',
    brightYellow: '#ffe57f',
    brightBlue: '#82b1ff',
    brightMagenta: '#ea80fc',
    brightCyan: '#84ffff',
    brightWhite: '#ffffff',
  },
  custom: {
    foreground: '#ffffff',
    background: '#000000',
    black: '#000000',
    red: '#cd0000',
    green: '#00cd00',
    yellow: '#cdcd00',
    blue: '#0000ee',
    magenta: '#cd00cd',
    cyan: '#00cdcd',
    white: '#f8fafc',
    brightBlack: '#7f7f7f',
    brightRed: '#ff0000',
    brightGreen: '#00ff00',
    brightYellow: '#ffff00',
    brightBlue: '#5c5cff',
    brightMagenta: '#ff00ff',
    brightCyan: '#00ffff',
    brightWhite: '#ffffff',
  },
};

// Helper function to generate smart ANSI colors based on current settings
function generateSmartColors(appSettings: AppSettings) {
  const hasBackgroundImage = !!appSettings.appearance.backgroundImage;
  const terminalTheme = appSettings.appearance.terminalTheme;

  // If has background image, use image-optimized style
  if (hasBackgroundImage) {
    return {
      foreground: '#e0e0e0',
      background: 'rgba(0, 0, 0, 0)',
      black: '#0a0a14',
      red: '#ff5252',
      green: '#69f0ae',
      yellow: '#ffd740',
      blue: '#448aff',
      magenta: '#e040fb',
      cyan: '#18ffff',
      white: '#ffffff',
      brightBlack: '#37474f',
      brightRed: '#ff80ab',
      brightGreen: '#b9f6ca',
      brightYellow: '#ffe57f',
      brightBlue: '#82b1ff',
      brightMagenta: '#ea80fc',
      brightCyan: '#84ffff',
      brightWhite: '#ffffff',
    };
  }

  // Otherwise, match the terminal theme
  const themeMap: Record<string, { hue: number; saturation: number }> = {
    'auto': { hue: 220, saturation: 30 },           // dark blue-gray
    'dracula': { hue: 270, saturation: 60 },         // purple
    'nord': { hue: 220, saturation: 30 },            // arctic blue
    'solarized-dark': { hue: 195, saturation: 80 },   // deep blue
    'solarized-light': { hue: 45, saturation: 60 },   // warm cream
    'monokai': { hue: 40, saturation: 50 },         // warm gray
    'one-dark': { hue: 220, saturation: 25 },        // dark blue
    'github-dark': { hue: 210, saturation: 40 },      // deep dark
    'tokyo-night': { hue: 240, saturation: 30 },     // dark purple-blue
    'catppuccin': { hue: 240, saturation: 30 },      // mauve
    'custom': { hue: 220, saturation: 30 },         // default
  };

  const themeColors = themeMap[terminalTheme] || themeMap['auto'];
  const hue = themeColors.hue;

  // Determine if theme is light based on theme preset.
  const isLightTheme = terminalTheme.includes('light');

  // Generate colors based on theme
  const foreground = isLightTheme ? '#2c3e50' : '#e0e0e0';

  // Generate harmonious colors using color theory
  // Complementary colors for better contrast
  const colorSaturation = isLightTheme ? 70 : 80;
  const red = hslToHex(hue + 350, colorSaturation, 60);
  const green = hslToHex(hue + 120, colorSaturation, 55);
  const yellow = hslToHex(hue + 60, colorSaturation, 55);
  const blue = hslToHex(hue, colorSaturation, 55);
  const magenta = hslToHex(hue + 300, colorSaturation, 55);
  const cyan = hslToHex(hue + 180, colorSaturation, 55);

  // Bright versions
  const brightRed = hslToHex(hue + 350, colorSaturation, 70);
  const brightGreen = hslToHex(hue + 120, colorSaturation, 65);
  const brightYellow = hslToHex(hue + 60, colorSaturation, 65);
  const brightBlue = hslToHex(hue, colorSaturation, 65);
  const brightMagenta = hslToHex(hue + 300, colorSaturation, 65);
  const brightCyan = hslToHex(hue + 180, colorSaturation, 65);

  return {
    foreground,
    background: 'rgba(0,0,0,0)',
    black: isLightTheme ? '#e8e8e8' : '#1a1a1a',
    red,
    green,
    yellow,
    blue,
    magenta,
    cyan,
    white: isLightTheme ? '#1a1a1a' : '#ffffff',
    brightBlack: isLightTheme ? '#d0d0d0' : '#555555',
    brightRed,
    brightGreen,
    brightYellow,
    brightBlue,
    brightMagenta,
    brightCyan,
    brightWhite: isLightTheme ? '#000000' : '#ffffff',
  };
}

// Convert HSL to Hex
function hslToHex(h: number, s: number, l: number): string {
  h = ((h % 360) + 360) % 360;
  s /= 100;
  l /= 100;

  const a = s * Math.min(l, 1 - l);
  const f = (n: number) => {
    const k = (n + h / 30) % 12;
    return l - a * Math.max(Math.min(k - 3, 9 - k, 1), -1);
  };

  const r = Math.round(255 * f(0));
  const g = Math.round(255 * f(8));
  const b = Math.round(255 * f(4));

  return `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
}

// Helper function to generate complete ANSI color scheme from 6 core colors
function generateAnsiColorsFromCustom(customColors: NonNullable<AppSettings['appearance']['customAnsiColors']>) {
  const { foreground, red, green, yellow, blue, magenta } = customColors;

  // Calculate derived colors
  const black = '#1a1a1a';
  const white = foreground;
  const cyan = mixColors(red, green);
  const brightBlack = '#555555';
  const brightWhite = '#ffffff';
  const brightRed = brightenColor(red, 20);
  const brightGreen = brightenColor(green, 20);
  const brightYellow = brightenColor(yellow, 20);
  const brightBlue = brightenColor(blue, 20);
  const brightMagenta = brightenColor(magenta, 20);
  const brightCyan = brightenColor(cyan, 20);

  return {
    foreground,
    background: 'rgba(0,0,0,0)',
    black,
    red,
    green,
    yellow,
    blue,
    magenta,
    cyan,
    white,
    brightBlack,
    brightRed,
    brightGreen,
    brightYellow,
    brightBlue,
    brightMagenta,
    brightCyan,
    brightWhite,
  };
}

// Helper: Mix two hex colors
function mixColors(color1: string, color2: string): string {
  const r1 = parseInt(color1.slice(1, 3), 16);
  const g1 = parseInt(color1.slice(3, 5), 16);
  const b1 = parseInt(color1.slice(5, 7), 16);

  const r2 = parseInt(color2.slice(1, 3), 16);
  const g2 = parseInt(color2.slice(3, 5), 16);
  const b2 = parseInt(color2.slice(5, 7), 16);

  const r = Math.round((r1 + r2) / 2);
  const g = Math.round((g1 + g2) / 2);
  const b = Math.round((b1 + b2) / 2);

  return `#${r.toString(16).padStart(2, '0')}${g.toString(16).padStart(2, '0')}${b.toString(16).padStart(2, '0')}`;
}

// Helper: Brighten a hex color
function brightenColor(hex: string, percent: number): string {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);

  const factor = 1 + percent / 100;
  const newR = Math.min(255, Math.round(r * factor));
  const newG = Math.min(255, Math.round(g * factor));
  const newB = Math.min(255, Math.round(b * factor));

  return `#${newR.toString(16).padStart(2, '0')}${newG.toString(16).padStart(2, '0')}${newB.toString(16).padStart(2, '0')}`;
}

export function getXtermTheme(appSettings: AppSettings): ITheme {
  const baseTheme = getBaseXtermTheme(appSettings);
  let preset = appSettings.appearance?.ansiColorPreset || 'classic';

  // Smart preset selection for light themes
  // If the user has a light background (theme) but is using a dark-only preset (like classic),
  // we override it to a smart/light preset to ensure readability.
  const bgHex = baseTheme.background || '#000000';
  // Calculate brightness inline to avoid circular dependency with terminalService
  const r = parseInt(bgHex.slice(1, 3), 16);
  const g = parseInt(bgHex.slice(3, 5), 16);
  const b = parseInt(bgHex.slice(5, 7), 16);
  const brightness = (r * 299 + g * 587 + b * 114) / 1000;
  
  const isLightBackground = brightness > 128;
  const isDarkPreset = ['classic', 'high-contrast', 'neon-dark', 'matrix-green', 'night-owl'].includes(preset);

  if (isLightBackground && isDarkPreset) {
    preset = 'smart';
  }

  // Get ANSI colors from preset or custom
  const ansiColors = preset === 'custom' && appSettings.appearance?.customAnsiColors
    ? generateAnsiColorsFromCustom(appSettings.appearance.customAnsiColors)
    : preset === 'smart'
    ? generateSmartColors(appSettings)
    : ansiColorPresets[preset] || ansiColorPresets['classic'];

  // Apply ANSI colors to theme
  return {
    ...baseTheme,
    background: 'rgba(0,0,0,0)', // Keep background transparent
    foreground: ansiColors.foreground,
    black: ansiColors.black,
    red: ansiColors.red,
    green: ansiColors.green,
    yellow: ansiColors.yellow,
    blue: ansiColors.blue,
    magenta: ansiColors.magenta,
    cyan: ansiColors.cyan,
    white: ansiColors.white,
    brightBlack: ansiColors.brightBlack,
    brightRed: ansiColors.brightRed,
    brightGreen: ansiColors.brightGreen,
    brightYellow: ansiColors.brightYellow,
    brightBlue: ansiColors.brightBlue,
    brightMagenta: ansiColors.brightMagenta,
    brightCyan: ansiColors.brightCyan,
    brightWhite: ansiColors.brightWhite,
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

export const isRightSidebarOpen = {
    subscribe: (run: (value: boolean) => void) => {
        return settings.subscribe(s => run(s.ui.rightSidebarOpen));
    },
    update: (fn: (value: boolean) => boolean) => {
        settings.update(s => ({
            ...s,
            ui: { ...s.ui, rightSidebarOpen: fn(s.ui.rightSidebarOpen) }
        }));
    },
    set: (value: boolean) => {
        settings.update(s => ({
            ...s,
            ui: { ...s.ui, rightSidebarOpen: value }
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

// Helper function to find group_id by folder path (from tags[0])
export function getGroupIdByPath(groups: ConnectionGroup[], folderPath: string): string | null {
  if (!folderPath || folderPath === '未分组') return null;
  
  // Exact match
  const exactMatch = groups.find(g => g.name === folderPath);
  if (exactMatch) return exactMatch.id;
  
  // If folderPath contains '/', try to match the last segment
  const lastSegment = folderPath.split('/').pop();
  if (lastSegment) {
    const partialMatch = groups.find(g => g.name === lastSegment);
    if (partialMatch) return partialMatch.id;
  }
  
  return null;
}
