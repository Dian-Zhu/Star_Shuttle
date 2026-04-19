import { writable, derived } from 'svelte/store';
import type { ITheme, Terminal } from '@xterm/xterm';
import type { FitAddon } from '@xterm/addon-fit';
import type { SearchAddon } from '@xterm/addon-search';
import { normalizeShortcut } from './shortcuts';
import {
  mergeTerminalSessionState,
  type TerminalSessionState,
  type TerminalSessionStatePatch,
} from './terminalSessionModel';

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
        has_password?: boolean;
      };
    }
  | {
      Http: {
        host: string;
        port: number;
        username: string | null;
        password: string | null;
        has_password?: boolean;
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

export interface HistoryConnectionSnapshot {
  id: string;
  name: string;
  protocol?: 'Ssh' | 'Rdp' | 'Telnet';
  host: string;
  port: number;
  username: string;
  description: string | null;
  tags: string[];
  group_id: string | null;
}

export interface HistoryItem {
  connection: HistoryConnectionSnapshot;
  lastConnected: number; // timestamp
}

export interface ActiveTerminal {
  sessionId: string;
  connection: Connection;
  terminal: Terminal;
  fitAddon: FitAddon;
  searchAddon: SearchAddon;
  currentDirectory?: string;
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
export const activeTerminalSessionIds = derived(activeTerminals, ($activeTerminals) => {
  return new Set($activeTerminals.map((item) => item.sessionId));
});
export const selectedTerminalIndex = writable<number>(0);
export const broadcastInputEnabled = writable<boolean>(false);
export const broadcastSessionIds = writable<string[]>([]);
export const showConnectionForm = writable<boolean>(false);
export const editingConnection = writable<Connection | null>(null);
export const showSettings = writable<boolean>(false);
export const showAdvancedModal = writable<boolean>(false);
export const showCommandPalette = writable<boolean>(false);
export const isLocked = writable<boolean>(false);

// Secure password prompt modal (replaces window.prompt for password collection)
export interface PasswordPromptRequest {
  title: string;
  resolve: (password: string | null) => void;
}
export const passwordPromptRequest = writable<PasswordPromptRequest | null>(null);

export function requestPasswordPrompt(title: string): Promise<string | null> {
  return new Promise((resolve) => {
    passwordPromptRequest.set({ title, resolve });
  });
}
export const hasAppLock = writable<boolean>(false);
export const loading = writable<boolean>(false);
export const connectingConnections = writable<Set<string>>(new Set());
export const errorMessage = writable<string | null>(null);
export const successMessage = writable<string | null>(null);
export const fileClipboard = writable<FileClipboardItem | null>(null);
export const terminalSplitConfigs = writable<Map<string, SplitConfig>>(new Map());
// Map root sessionId -> Set of all child sessionIds (including root itself)
export const terminalSessionMap = writable<Map<string, Set<string>>>(new Map());
export const terminalSessionStates = writable<Map<string, TerminalSessionState>>(new Map());
export const closeSplitRequest = writable<string | null>(null);

export function setTerminalSessionState(
  sessionId: string,
  patch: TerminalSessionStatePatch & {
    connectionPhase?: TerminalSessionState['connectionPhase'];
    terminalPhase?: TerminalSessionState['terminalPhase'];
  }
): void {
  terminalSessionStates.update((current) => {
    const next = new Map(current);
    next.set(sessionId, mergeTerminalSessionState(current.get(sessionId), patch));
    return next;
  });
}

export function removeTerminalSessionState(sessionId: string): void {
  terminalSessionStates.update((current) => {
    if (!current.has(sessionId)) return current;
    const next = new Map(current);
    next.delete(sessionId);
    return next;
  });
}

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

function toHistoryConnectionSnapshot(value: unknown): HistoryConnectionSnapshot | null {
  if (!value || typeof value !== 'object') return null;
  const source = value as Record<string, unknown>;
  const id = typeof source.id === 'string' ? source.id : null;
  const name = typeof source.name === 'string' ? source.name : null;
  const host = typeof source.host === 'string' ? source.host : null;
  const port = typeof source.port === 'number' && Number.isFinite(source.port) ? source.port : null;
  const username = typeof source.username === 'string' ? source.username : null;
  if (!id || !name || !host || port === null || !username) return null;

  const protocol = source.protocol === 'Ssh' || source.protocol === 'Rdp' || source.protocol === 'Telnet'
    ? source.protocol
    : undefined;
  const description = source.description === null || typeof source.description === 'string'
    ? source.description ?? null
    : null;
  const tags = Array.isArray(source.tags) ? source.tags.filter((tag): tag is string => typeof tag === 'string') : [];
  const group_id = source.group_id === null || typeof source.group_id === 'string'
    ? source.group_id ?? null
    : null;

  return {
    id,
    name,
    protocol,
    host,
    port,
    username,
    description,
    tags,
    group_id,
  };
}

function normalizeHistoryItem(value: unknown): HistoryItem | null {
  if (!value || typeof value !== 'object') return null;
  const source = value as Record<string, unknown>;
  const connection = toHistoryConnectionSnapshot(source.connection);
  const lastConnected = typeof source.lastConnected === 'number' && Number.isFinite(source.lastConnected)
    ? source.lastConnected
    : null;
  if (!connection || lastConnected === null) return null;
  return { connection, lastConnected };
}

// History Store
const loadHistory = (): HistoryItem[] => {
  if (typeof localStorage === 'undefined') return [];
  try {
    const stored = localStorage.getItem('connectionHistory');
    if (!stored) return [];
    const parsed = JSON.parse(stored);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .map((item) => normalizeHistoryItem(item))
      .filter((item): item is HistoryItem => item !== null);
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
    ansiColorPreset: 'classic' | 'standard-light' | 'solarized' | 'solarized-light' | 'github-light' | 'monokai' | 'gruvbox' | 'dracula' | 'one-dark' | 'tokyo-night' | 'custom';
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
    scrollback: 3000,
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
    if ((parsed.appearance?.ansiColorPreset as string | undefined) === 'smart') {
      const terminalTheme = merged.appearance.terminalTheme;
      const isLightPreference =
        terminalTheme === 'solarized-light' ||
        (terminalTheme === 'auto' && merged.theme === 'light');
      merged.appearance.ansiColorPreset = isLightPreference ? 'standard-light' : 'classic';
    }
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
  dracula: {
    foreground: '#f8f8f2',
    background: '#282a36',
    black: '#21222c',
    red: '#ff5555',
    green: '#50fa7b',
    yellow: '#f1fa8c',
    blue: '#bd93f9',
    magenta: '#ff79c6',
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
  },
  'one-dark': {
    foreground: '#abb2bf',
    background: '#282c34',
    black: '#282c34',
    red: '#e06c75',
    green: '#98c379',
    yellow: '#e5c07b',
    blue: '#61afef',
    magenta: '#c678dd',
    cyan: '#56b6c2',
    white: '#dcdfe4',
    brightBlack: '#5c6370',
    brightRed: '#e06c75',
    brightGreen: '#98c379',
    brightYellow: '#e5c07b',
    brightBlue: '#61afef',
    brightMagenta: '#c678dd',
    brightCyan: '#56b6c2',
    brightWhite: '#ffffff',
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
  'tokyo-night': {
    foreground: '#c0caf5',
    background: '#1a1b26',
    black: '#15161e',
    red: '#f7768e',
    green: '#9ece6a',
    yellow: '#e0af68',
    blue: '#7aa2f7',
    magenta: '#bb9af7',
    cyan: '#7dcfff',
    white: '#a9b1d6',
    brightBlack: '#414868',
    brightRed: '#f7768e',
    brightGreen: '#9ece6a',
    brightYellow: '#e0af68',
    brightBlue: '#7aa2f7',
    brightMagenta: '#bb9af7',
    brightCyan: '#7dcfff',
    brightWhite: '#c0caf5',
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

  // Align stale dark/light ANSI presets with the active terminal background brightness.
  const bgHex = baseTheme.background || '#000000';
  // Calculate brightness inline to avoid circular dependency with terminalService
  const r = parseInt(bgHex.slice(1, 3), 16);
  const g = parseInt(bgHex.slice(3, 5), 16);
  const b = parseInt(bgHex.slice(5, 7), 16);
  const brightness = (r * 299 + g * 587 + b * 114) / 1000;
  
  const isLightBackground = brightness > 128;
  const isDarkPreset = ['classic', 'solarized', 'monokai', 'gruvbox', 'dracula', 'one-dark', 'tokyo-night'].includes(preset);
  const isLightPreset = ['standard-light', 'solarized-light', 'github-light'].includes(preset);

  if (isLightBackground && isDarkPreset) {
    preset = 'standard-light';
  } else if (!isLightBackground && isLightPreset) {
    preset = 'classic';
  }

  // Get ANSI colors from preset or custom
  const ansiColors = preset === 'custom' && appSettings.appearance?.customAnsiColors
    ? generateAnsiColorsFromCustom(appSettings.appearance.customAnsiColors)
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

// Legacy writable facade kept for existing call sites. New code should use `settings` directly.
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

function normalizeGroupPath(value: string): string {
  const normalized = value
    .split('/')
    .map(part => part.trim())
    .filter(Boolean)
    .join('/');
  return normalized || '未分组';
}

// Helper function to find group_id by folder path (from tags[0])
export function getGroupIdByPath(groups: ConnectionGroup[], folderPath: string): string | null {
  const normalizedFolderPath = normalizeGroupPath(folderPath);
  if (normalizedFolderPath === '未分组') return null;

  const exactMatch = groups.find(g => normalizeGroupPath(g.name) === normalizedFolderPath);
  return exactMatch?.id ?? null;
}
