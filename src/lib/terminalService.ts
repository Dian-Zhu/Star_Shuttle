import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import { WebglAddon } from '@xterm/addon-webgl';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { get } from 'svelte/store';
import { activeTerminals, connections, selectedTerminalIndex, type Connection, type ActiveTerminal, type AppSettings, errorMessage, successMessage, settings, connectionHistory, broadcastInputEnabled, broadcastSessionIds, getStoredTerminalUiState, getXtermTheme, terminalSessionMap } from './store';
import { terminalPool } from './terminalPool';
import { TerminalInstance } from './terminalInstance';
import '@xterm/xterm/css/xterm.css';

const IS_DEV = import.meta.env.DEV;

/**
 * xterm 6.0: Unified logging with context-aware prefixes
 */
const log = {
  info: (component: string, message: string, ...args: any[]) => {
    if (IS_DEV) {
      console.log(`[${component}] ${message}`, ...args);
    }
  },
  warn: (component: string, message: string, ...args: any[]) => {
    if (IS_DEV) {
      console.warn(`[${component}] ${message}`, ...args);
    }
  },
  error: (component: string, message: string, error: any, context?: Record<string, any>) => {
    if (IS_DEV) {
      console.error(`[${component}] ${message}`, error, context ? { context } : '');
    }
  },
  perf: (component: string, operation: string, duration: number, details?: Record<string, any>) => {
    if (IS_DEV && duration > 10) {
      console.warn(`[PERF] [${component}] ${operation} took ${duration.toFixed(1)}ms`, details || '');
    }
  }
};

/**
 * xterm 6.0: Error handling with user-friendly messages
 */
function handleTerminalError(error: unknown, context: string, userMessage?: string) {
  const errorMsg = error instanceof Error ? error.message : String(error);

  if (IS_DEV) {
    log.error('TermError', `${context}: ${errorMsg}`, error);
  }

  // User-facing error message
  if (userMessage) {
    errorMessage.set(userMessage);
    setTimeout(() => errorMessage.set(null), 5000);
  }
}

/**
 * Output listeners storage - maps session IDs to cleanup functions
 * These listeners handle terminal output events from the backend
 */
const outputListeners = new Map<string, () => void>();

/**
 * Input listeners storage - maps session IDs to disposables
 * These listeners handle user input events
 */
const inputListeners = new Map<string, { dispose: () => void }>();

/**
 * Session status monitoring - maps session IDs to cleanup functions
 */
const sessionStatusListeners = new Map<string, () => void>();

/**
 * Reconnection tracking
 */
const reconnectAttempts = new Map<string, number>();
const reconnectTimers = new Map<string, number>();
const reconnectKeyListeners = new Map<string, { dispose: () => void }>();
const MAX_AUTO_RECONNECT_RETRIES = 5;
const BASE_AUTO_RECONNECT_DELAY_MS = 1500;
const MAX_AUTO_RECONNECT_DELAY_MS = 30000;

/**
 * Track which terminal sessions have been started
 */
const startedTerminalSessions = new Set<string>();

/**
 * Mark a terminal session as started
 * @param sessionId - The session ID to mark
 */
export function markTerminalStarted(sessionId: string) {
  startedTerminalSessions.add(sessionId);
}

/**
 * Mark a terminal session as stopped
 * @param sessionId - The session ID to unmark
 */
export function markTerminalStopped(sessionId: string) {
  startedTerminalSessions.delete(sessionId);
}

/**
 * Apply scrollbar color to match terminal theme background
 */
export function applyScrollbarColor(appSettings: AppSettings): void {
  log.info('Scrollbar', 'Updating scrollbar colors for terminal');
  const theme = getXtermTheme(appSettings);
  let terminalBg = theme.background || '#0f172a';
  
  // If background image is present, force terminal background to be transparent
  // This ensures the background image behind the terminal is visible
  if (appSettings.appearance?.backgroundImage) {
    terminalBg = 'rgba(0,0,0,0)';
  }

  // Update terminal background color
  document.documentElement.style.setProperty('--terminal-bg', terminalBg);

  // Calculate scrollbar colors based on terminal background brightness
  let brightness = 0;
  
  if (terminalBg === 'transparent') {
    // For transparent background, assume dark background (light scrollbars)
    // or we could try to analyze the image, but that's too complex.
    brightness = 0; 
  } else {
    brightness = calculateBrightness(terminalBg);
  }

  if (brightness < 128) {
    // Dark background
    document.documentElement.style.setProperty('--scrollbar-track', 'rgba(255, 255, 255, 0.05)');
    document.documentElement.style.setProperty('--scrollbar-thumb', 'rgba(255, 255, 255, 0.2)');
    document.documentElement.style.setProperty('--scrollbar-thumb-hover', 'rgba(255, 255, 255, 0.3)');
  } else {
    // Light background
    document.documentElement.style.setProperty('--scrollbar-track', 'rgba(0, 0, 0, 0.05)');
    document.documentElement.style.setProperty('--scrollbar-thumb', 'rgba(0, 0, 0, 0.15)');
    document.documentElement.style.setProperty('--scrollbar-thumb-hover', 'rgba(0, 0, 0, 0.25)');
  }

  log.info('Scrollbar', `Updated scrollbar colors`, {
    terminalBg,
    brightness,
    themePreset: appSettings.appearance?.terminalTheme,
  });
}

/**
 * Calculate brightness of a color (0-255)
 */
function calculateBrightness(color: string): number {
  const hex = color.replace('#', '');
  const r = parseInt(hex.substr(0, 2), 16);
  const g = parseInt(hex.substr(2, 2), 16);
  const b = parseInt(hex.substr(4, 2), 16);
  return (r * 299 + g * 587 + b * 114) / 1000;
}

/**
 * State for managing terminal output writes in xterm 6.0
 * @property chunks - Array of pending output chunks to write
 * @property chunkIndex - Current position in chunks array
 * @property scheduled - Scheduled write job (RAF or timeout)
 * @property writing - Whether a write is currently in progress
 * @property disposed - Whether the state has been disposed
 * @property chunkBudget - Adaptive budget for chunk size (optimizes performance)
 * @property lastWriteTime - Timing of the last write (for performance monitoring)
 * @property consecutiveSlowWrites - Count of consecutive slow writes (triggers budget reduction)
 */
type OutputWriteState = {
  chunks: string[];
  chunkIndex: number;
  scheduled: { id: number; kind: 'raf' | 'timeout' } | null;
  writing: boolean;
  disposed: boolean;
  chunkBudget: number;
  lastWriteTime: number;
  consecutiveSlowWrites: number;
};

/**
 * State for managing terminal input sends in xterm 6.0
 * @property buffer - Buffer of pending input to send
 * @property timer - Scheduled flush job (RAF or timeout)
 * @property sending - Whether a send is currently in progress
 * @property lastFlushTime - Timing of the last flush
 * @property pendingChunks - Number of chunks pending to be sent (adaptive throttling)
 */
type InputSendState = {
  buffer: string;
  timer: { id: number; kind: 'raf' | 'timeout' } | null;
  sending: boolean;
  lastFlushTime: number;
  pendingChunks: number;
};

const outputWriteStates = new Map<string, OutputWriteState>();
const inputSendStates = new Map<string, InputSendState>();

function nowMs(): number {
  if (typeof performance !== 'undefined' && typeof performance.now === 'function') return performance.now();
  return Date.now();
}

function scheduleNext(callback: () => void): { id: number; kind: 'raf' | 'timeout' } {
  const hidden = typeof document !== 'undefined' && document.hidden === true;
  if (!hidden && typeof requestAnimationFrame === 'function') {
    return { id: requestAnimationFrame(callback), kind: 'raf' };
  }
  const timeout = (typeof window !== 'undefined' ? window.setTimeout : setTimeout) as typeof setTimeout;
  return { id: timeout(callback, 0) as unknown as number, kind: 'timeout' };
}

function cancelScheduled(job: { id: number; kind: 'raf' | 'timeout' } | null) {
  if (!job) return;
  if (job.kind === 'raf') {
    cancelAnimationFrame(job.id);
    return;
  }
  const clear = (typeof window !== 'undefined' ? window.clearTimeout : clearTimeout) as typeof clearTimeout;
  clear(job.id as unknown as ReturnType<typeof setTimeout>);
}

export async function connectAndOpen(connection: Connection, connectConfig?: any) {
  try {
    errorMessage.set(null);

    const baseConfig = connectConfig ?? connection;
    const protocol = (baseConfig as any)?.protocol ?? 'Ssh';
    if (protocol === 'Rdp') {
      await invoke('launch_rdp', { config: baseConfig });

      connectionHistory.update(history => {
        const newHistory = history.filter(h => h.connection.id !== connection.id);
        newHistory.unshift({
          connection,
          lastConnected: Date.now()
        });
        return newHistory.slice(0, 50);
      });

      successMessage.set(`已启动 RDP: ${connection.name}`);
      setTimeout(() => successMessage.set(null), 3000);
      return;
    }

    if (protocol === 'Telnet') {
      const sessionId = await invoke('connect', { config: baseConfig }) as string;

      const terminals = get(activeTerminals);
      const existingIndex = terminals.findIndex(t => t.sessionId === sessionId);
      if (existingIndex !== -1) {
        selectedTerminalIndex.set(existingIndex);
        return;
      }

      const newIndex = terminals.length;
      selectedTerminalIndex.set(newIndex);
      activeTerminals.update(items => [
        ...items,
        {
          sessionId,
          connection,
          terminal: null as any,
          fitAddon: null as any,
          searchAddon: null as any
        }
      ]);

      connectionHistory.update(history => {
        const newHistory = history.filter(h => h.connection.id !== connection.id);
        newHistory.unshift({
          connection,
          lastConnected: Date.now()
        });
        return newHistory.slice(0, 50);
      });

      successMessage.set(`连接成功: ${connection.name}`);
      setTimeout(() => successMessage.set(null), 3000);
      return;
    }

    let config = await ensureConnectConfig(baseConfig, connection.name);

    let sessionId: string;
    try {
      sessionId = await connectWithKnownHostsPrompt(config);
    } catch (error) {
      if (shouldPromptForPasswordOnConnectError(error, config)) {
        const prompted = await promptPassword(connection.name);
        config = applyPromptedPassword(config, prompted);
        sessionId = await connectWithKnownHostsPrompt(config);
      } else {
        throw error;
      }
    }
    
    // Check if session already exists (shouldn't happen with unique sessionIds usually, but good to check)
    const terminals = get(activeTerminals);
    const existingIndex = terminals.findIndex(t => t.sessionId === sessionId);
    
    if (existingIndex !== -1) {
      selectedTerminalIndex.set(existingIndex);
      return;
    }

    const newIndex = terminals.length;
    selectedTerminalIndex.set(newIndex);
    // Add to active terminals store
    // The TerminalManager component will react to this and create a TerminalView
    // The TerminalView will call initTerminal
    activeTerminals.update(items => [
      ...items,
      {
        sessionId,
        connection,
        terminal: null as any, // Will be initialized by view
        fitAddon: null as any,
        searchAddon: null as any
      }
    ]);

    // Update history
    connectionHistory.update(history => {
      // Remove existing entry for this connection if any
      const newHistory = history.filter(h => h.connection.id !== connection.id);
      // Add to top
      newHistory.unshift({
        connection,
        lastConnected: Date.now()
      });
      // Limit to 50 items
      return newHistory.slice(0, 50);
    });

    successMessage.set(`连接成功: ${connection.name}`);
    setTimeout(() => successMessage.set(null), 3000);
    
  } catch (error) {
    log.error('Connection', `Failed to connect to ${connection.name}`, error);
    errorMessage.set(`连接失败：${error}`);
    setTimeout(() => errorMessage.set(null), 5000);
  }
}

async function ensureConnectConfig(config: any, connectionName: string): Promise<any> {
  const passwordAuth = config?.auth_method?.Password;
  if (!passwordAuth) return config;
  const existingPassword = typeof passwordAuth.password === 'string' ? passwordAuth.password : '';
  if (existingPassword.trim()) return config;
  if (passwordAuth.save_password === true) return config;

  const password = await promptPassword(connectionName);
  return applyPromptedPassword(config, password);
}

function normalizeErrorMessage(error: unknown): string {
  if (error instanceof Error && typeof error.message === 'string') return error.message;
  return String(error);
}

function shouldPromptForPasswordOnConnectError(error: unknown, config: any): boolean {
  const msg = normalizeErrorMessage(error);
  const needsPassword =
    msg.includes('Password is required') ||
    msg.includes('password is required') ||
    msg.includes('Password is required when save_password is enabled');

  if (!needsPassword) return false;
  const passwordAuth = config?.auth_method?.Password;
  if (!passwordAuth) return false;
  const existingPassword = typeof passwordAuth.password === 'string' ? passwordAuth.password : '';
  if (existingPassword.trim()) return false;
  return true;
}

async function promptPassword(connectionName: string): Promise<string> {
  const entered = window.prompt(`请输入连接「${connectionName}」的密码`, '');
  if (entered === null) {
    throw new Error('已取消输入密码');
  }
  const password = entered.trim();
  if (!password) {
    throw new Error('密码不能为空');
  }
  return password;
}

function applyPromptedPassword(config: any, password: string): any {
  const passwordAuth = config?.auth_method?.Password;
  if (!passwordAuth) return config;
  return {
    ...config,
    auth_method: {
      ...config.auth_method,
      Password: {
        ...passwordAuth,
        password,
        save_password: false
      }
    }
  };
}

export async function createTerminalSession(connection: Connection): Promise<string> {
  return connectWithKnownHostsPrompt(connection);
}

type HostKeyPromptType = 'unknown' | 'mismatch' | 'unavailable';

interface HostKeyPromptPayload {
  host: string;
  port: number;
  fingerprint: string;
  key_type: string;
  key_base64: string;
  reason?: string;
}

function parseHostKeyPrompt(error: unknown): { type: HostKeyPromptType; payload: HostKeyPromptPayload } | null {
  const str = String(error);
  const markers: Array<[string, HostKeyPromptType]> = [
    ['HOST_KEY_UNKNOWN|', 'unknown'],
    ['HOST_KEY_MISMATCH|', 'mismatch'],
    ['HOST_KEY_UNAVAILABLE|', 'unavailable']
  ];

  for (const [marker, type] of markers) {
    const idx = str.lastIndexOf(marker);
    if (idx === -1) continue;
    const jsonPart = str.slice(idx + marker.length).trim();
    try {
      const payload = JSON.parse(jsonPart) as HostKeyPromptPayload;
      if (!payload || typeof payload.host !== 'string') return null;
      return { type, payload };
    } catch {
      return null;
    }
  }
  return null;
}

async function connectWithKnownHostsPrompt(connection: any): Promise<string> {
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      const result = await invoke('connect', { config: connection });
      return result as string;
    } catch (error) {
      const parsed = parseHostKeyPrompt(error);
      if (!parsed) throw error;

      const { type, payload } = parsed;
      const title =
        type === 'unknown'
          ? '首次连接确认'
          : type === 'mismatch'
            ? '主机密钥已变更'
            : '无法校验主机密钥';

      const messageLines = [
        `${title}: ${payload.host}:${payload.port}`,
        `Key Type: ${payload.key_type}`,
        `Fingerprint: ${payload.fingerprint}`,
        payload.reason ? `Reason: ${payload.reason}` : null,
        '',
        type === 'unknown'
          ? '是否信任该主机并保存到 known_hosts？'
          : type === 'mismatch'
            ? '这可能是中间人攻击或服务器重装导致。仍要信任并替换 known_hosts 记录吗？'
            : 'known_hosts 文件不可用。仍要信任并保存到 known_hosts 吗？'
      ].filter(Boolean);

      const confirmed = window.confirm(messageLines.join('\n'));
      if (!confirmed) {
        throw error;
      }

      await invoke('known_hosts_save_host_key', {
        host: payload.host,
        port: payload.port,
        keyType: payload.key_type,
        keyBase64: payload.key_base64,
        replace: type === 'mismatch'
      });
    }
  }

  const result = await invoke('connect', { config: connection });
  return result as string;
}

/**
 * xterm 6.0: Optimized terminal initialization with better error handling
 * - Improved addon loading with graceful fallbacks
 * - Enhanced layout detection and fitting
 * - Better resource management
 */
export async function initTerminal(container: HTMLElement, sessionId: string, connection: Connection): Promise<ActiveTerminal | null> {
  try {
    // Clear container
    container.innerHTML = '';

    // Get current settings
    const appSettings = get(settings);

    // xterm 6.0: Create terminal with optimized options
    const term = new Terminal({
      cursorBlink: appSettings.terminal.cursorBlink,
      cursorStyle: appSettings.terminal.cursorStyle,
      cursorWidth: 1,
      fontSize: appSettings.terminal.fontSize,
      fontFamily: appSettings.terminal.fontFamily,
      theme: getXtermTheme(appSettings),
      scrollback: appSettings.terminal.scrollback,
      allowProposedApi: true,
      allowTransparency: true, // Enable transparency for background images
      convertEol: true, // Enable EOL conversion to fix line endings
      // xterm 6.0: Performance optimizations
      altClickMovesCursor: true, // Better UX
      scrollSensitivity: 1, // Smooth scrolling
      fastScrollSensitivity: 5, // Fast scroll speed
      rightClickSelectsWord: true, // Better UX
      macOptionIsMeta: false, // Standard behavior
    });

    // xterm 6.0: Load addons in optimal order
    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);

    // xterm 6.0: WebLinks addon with enhanced security
    term.loadAddon(
      new WebLinksAddon((event, uri) => {
        event.preventDefault();
        // Validate URI before opening
        if (uri && (uri.startsWith('http://') || uri.startsWith('https://'))) {
          window.open(uri, '_blank', 'noopener,noreferrer');
        }
      })
    );

    // 立即注册到池中，确保在组件挂载前可用
    const terminalInstance = TerminalInstance.fromInitialized(sessionId, term, fitAddon, searchAddon);
    terminalPool.registerInstance(terminalInstance);
    log.info('TermInit', 'Terminal instance registered to pool', { sessionId });

    // Open terminal first
    term.open(container);
    log.info('TermInit', 'Terminal opened in container', {
      sessionId,
      containerSize: {
        width: container.clientWidth,
        height: container.clientHeight,
      },
      terminalSize: { cols: term.cols, rows: term.rows },
    });

    // xterm 6.0: Improved WebGL addon loading with better fallback
    let webglAddon: WebglAddon | null = null;
    try {
      webglAddon = new WebglAddon();
      term.loadAddon(webglAddon);
      log.info('TermInit', 'WebGL renderer loaded successfully');
      webglAddon.onContextLoss(() => {
        log.warn('TermInit', 'WebGL context lost, falling back to canvas');
        try {
          webglAddon?.dispose();
        } catch {
          webglAddon = null;
        }
      });
    } catch (e) {
      log.warn('TermInit', 'WebGL addon unavailable, using canvas fallback', e);
    }

    // xterm 6.0: Enhanced layout detection with exponential backoff
    const waitForLayout = () =>
      new Promise<void>((resolve) => {
        if (typeof requestAnimationFrame === 'function') {
          requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
          return;
        }
        const timeout = (typeof window !== 'undefined' ? window.setTimeout : setTimeout) as typeof setTimeout;
        timeout(() => resolve(), 0);
      });

    let didFit = false;
    for (let i = 0; i < 10; i++) {
      await waitForLayout();
      const rect = container.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        fitAddon.fit();
        didFit = true;
        if (i > 0) {
          log.info('TermInit', `Layout stabilized after ${i + 1} attempts`);
        }
        break;
      }
    }

    // Fallback: Force fit even if layout not ready
    if (!didFit) {
      const rect = container.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        fitAddon.fit();
        log.warn('TermInit', 'Forcing fit on unready layout');
      }
    }

    // Handle user input
    const inputDisposable = term.onData((data) => {
      handleTerminalInput(sessionId, data, connection);
    });
    inputListeners.set(sessionId, inputDisposable);

    // Listen for terminal output from backend
    await setupTerminalListeners(sessionId, term);

    // Request terminal session from backend
    const result = await invoke('start_terminal', {
      sessionId,
      width: term.cols,
      height: term.rows,
    });

    if (!result) {
      log.error('TermInit', 'Failed to start terminal session', null);
      void term.write('\r\n\x1b[31mFailed to start terminal session\x1b[0m\r\n');
      errorMessage.set('启动终端会话失败');
      setTimeout(() => errorMessage.set(null), 5000);
      return null;
    }

    markTerminalStarted(sessionId);
    log.info('TermInit', 'Terminal session started successfully', {
      sessionId,
      cols: term.cols,
      rows: term.rows,
    });
    void sendTerminalResize(sessionId, term.cols, term.rows);
    term.focus();

    // Dynamic scrollbar color matching - wait for rendering to complete
    log.info('TermInit', 'About to call applyScrollbarColor', {
      sessionId,
      containerExists: !!container,
      appSettingsTerminalTheme: appSettings.appearance?.terminalTheme,
    });
    applyScrollbarColor(appSettings);

    // Update the store with the initialized terminal instance
    activeTerminals.update(items => items.map(t => {
      if (t.sessionId === sessionId) {
        return { ...t, terminal: term, fitAddon, searchAddon };
      }
      return t;
    }));

    log.info('TermInit', 'Terminal initialized successfully', {
      sessionId,
      cols: term.cols,
      rows: term.rows,
      renderer: webglAddon ? 'webgl' : 'canvas',
    });

    return {
      sessionId,
      connection,
      terminal: term,
      fitAddon,
      searchAddon,
    };
  } catch (error) {
    handleTerminalError(error, 'initTerminal', '初始化终端失败');
    return null;
  }
}

export async function initDetachedTerminal(container: HTMLElement, sessionId: string, connection: Connection): Promise<ActiveTerminal | null> {
  try {
    container.innerHTML = '';

    const appSettings = get(settings);

    const term = new Terminal({
      cursorBlink: appSettings.terminal.cursorBlink,
      cursorStyle: appSettings.terminal.cursorStyle,
      cursorWidth: 1,
      fontSize: appSettings.terminal.fontSize,
      fontFamily: appSettings.terminal.fontFamily,
      theme: getXtermTheme(appSettings),
      scrollback: appSettings.terminal.scrollback,
      allowProposedApi: true,
      allowTransparency: true, // Enable transparency for background images
      convertEol: true,
      altClickMovesCursor: true,
      scrollSensitivity: 1,
      fastScrollSensitivity: 5,
      rightClickSelectsWord: true,
      macOptionIsMeta: false,
    });

    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);

    term.loadAddon(
      new WebLinksAddon((event, uri) => {
        event.preventDefault();
        if (uri && (uri.startsWith('http://') || uri.startsWith('https://'))) {
          window.open(uri, '_blank', 'noopener,noreferrer');
        }
      })
    );

    // 立即注册到池中，确保在组件挂载前可用
    const terminalInstance = TerminalInstance.fromInitialized(sessionId, term, fitAddon, searchAddon);
    terminalPool.registerInstance(terminalInstance);
    log.info('TermInit', 'Detached terminal instance registered to pool', { sessionId });

    term.open(container);

    let webglAddon: WebglAddon | null = null;
    try {
      webglAddon = new WebglAddon();
      term.loadAddon(webglAddon);
      webglAddon.onContextLoss(() => {
        try {
          webglAddon?.dispose();
        } catch {
          webglAddon = null;
        }
      });
    } catch {
      webglAddon = null;
    }

    const waitForLayout = () =>
      new Promise<void>((resolve) => {
        if (typeof requestAnimationFrame === 'function') {
          requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
          return;
        }
        const timeout = (typeof window !== 'undefined' ? window.setTimeout : setTimeout) as typeof setTimeout;
        timeout(() => resolve(), 0);
      });

    for (let i = 0; i < 10; i++) {
      await waitForLayout();
      const rect = container.getBoundingClientRect();
      if (rect.width > 0 && rect.height > 0) {
        fitAddon.fit();
        break;
      }
    }

    const inputDisposable = term.onData((data) => {
      handleTerminalInput(sessionId, data, connection);
    });
    inputListeners.set(sessionId, inputDisposable);

    await setupTerminalListeners(sessionId, term);

    const result = await invoke('start_terminal', {
      sessionId,
      width: term.cols,
      height: term.rows,
    });

    if (!result) {
      void term.write('\r\n\x1b[31mFailed to start terminal session\x1b[0m\r\n');
      errorMessage.set('启动终端会话失败');
      setTimeout(() => errorMessage.set(null), 5000);
      return null;
    }

    markTerminalStarted(sessionId);
    void sendTerminalResize(sessionId, term.cols, term.rows);

    // Apply scrollbar color matching for detached terminal
    log.info('TermInit', 'Detached terminal: About to call applyScrollbarColor', {
      sessionId,
      containerExists: !!container,
      appSettingsTerminalTheme: appSettings.appearance?.terminalTheme,
    });
    applyScrollbarColor(appSettings);

    log.info('TermInit', 'Detached terminal initialized successfully', {
      sessionId,
      cols: term.cols,
      rows: term.rows,
      renderer: webglAddon ? 'webgl' : 'canvas',
    });

    return {
      sessionId,
      connection,
      terminal: term,
      fitAddon,
      searchAddon,
    };
  } catch (error) {
    handleTerminalError(error, 'initDetachedTerminal', '初始化终端失败');
    return null;
  }
}

export async function sendTerminalData(sessionId: string, data: string) {
  try {
    await invoke('send_terminal_data', { sessionId, data });
  } catch (error) {
    log.error('TermInput', 'Failed to send terminal data', error, { sessionId });
  }
}

/**
 * xterm 6.0: Get or create input send state with tracking
 */
function getInputSendState(sessionId: string): InputSendState {
  const existing = inputSendStates.get(sessionId);
  if (existing) return existing;
  const created: InputSendState = {
    buffer: '',
    timer: null,
    sending: false,
    lastFlushTime: 0,
    pendingChunks: 0,
  };
  inputSendStates.set(sessionId, created);
  return created;
}

/**
 * xterm 6.0: Optimized input flushing with better batching
 */
async function flushTerminalInput(sessionId: string, state: InputSendState) {
  if (state.sending) return;
  if (!state.buffer) return;

  state.sending = true;
  const now = nowMs();

  try {
    // xterm 6.0: Adaptive payload size based on recent performance
    const maxPayload = state.pendingChunks > 5 ? 4096 : 2048;
    const payload = state.buffer.slice(0, maxPayload);
    state.buffer = state.buffer.slice(payload.length);

    state.pendingChunks++;
    await invoke('send_terminal_data', { sessionId, data: payload });
    state.lastFlushTime = nowMs() - now;

    // xterm 6.0: Reduce pending chunk counter on successful send
    if (state.pendingChunks > 0) {
      state.pendingChunks--;
    }
  } catch (error) {
    log.error('TermInput', 'Failed to send terminal data', error, {
      sessionId,
      bufferLength: state.buffer.length,
      pendingChunks: state.pendingChunks,
    });
  } finally {
    state.sending = false;
    if (state.buffer.length > 0 && state.timer === null) {
      state.timer = scheduleNext(() => {
        state.timer = null;
        void flushTerminalInput(sessionId, state);
      });
    }
  }
}

/**
 * xterm 6.0: Enhanced input buffering with adaptive thresholds
 */
function sendTerminalDataBuffered(sessionId: string, data: string, immediate: boolean) {
  const state = getInputSendState(sessionId);
  state.buffer += data;

  // xterm 6.0: Dynamic threshold based on recent activity
  const threshold = state.pendingChunks > 10 ? 2048 : 1024;
  if (state.buffer.length >= threshold) {
    if (!immediate) {
      log.info('TermInput', 'Force immediate flush', {
        bufferLength: state.buffer.length,
        threshold,
        pendingChunks: state.pendingChunks,
      });
    }
    immediate = true;
  }

  if (state.timer !== null) {
    if (!immediate) return;
    cancelScheduled(state.timer);
    state.timer = null;
  }

  if (immediate) {
    log.info('TermInput', 'Immediate flush', {
      dataLength: data.length,
      bufferLength: state.buffer.length,
    });
    void flushTerminalInput(sessionId, state);
    return;
  }

  // xterm 6.0: Use minimal delay with RAF for smooth timing
  state.timer = scheduleNext(() => {
    state.timer = null;
    void flushTerminalInput(sessionId, state);
  });
}

function handleTerminalInputSingle(sessionId: string, data: string) {
  const hasControl =
    data.includes('\r') ||
    data.includes('\n') ||
    data.includes('\x03') ||
    data.includes('\x1b');

  const shouldImmediate = hasControl;
  
  // Debug log for control character detection
  if (shouldImmediate) {
     log.info('TermInput', 'Control char detected', { data: JSON.stringify(data) });
  }

  void sendTerminalDataBuffered(sessionId, data, shouldImmediate);
}

export function handleTerminalInput(sessionId: string, data: string, connection: Connection) {
  const enabled = get(broadcastInputEnabled);
  if (!enabled) {
    handleTerminalInputSingle(sessionId, data);
    return;
  }

  const selected = get(broadcastSessionIds);
  const baseTargets = selected.length > 0 ? selected : [sessionId];
  // Expand targets to include all child sessions from split panes
  const expandedTargets = new Set<string>();
  const sessionMap = get(terminalSessionMap);
  
  for (const targetId of baseTargets) {
    expandedTargets.add(targetId);
    // If this target is a root session, include all its children
    const children = sessionMap.get(targetId);
    if (children) {
      children.forEach(childId => expandedTargets.add(childId));
    }
  }
  
  const targets = Array.from(expandedTargets);
  if (!targets.includes(sessionId)) {
      targets.push(sessionId);
  }

  const terminals = get(activeTerminals);
  const connectionBySessionId = new Map(terminals.map(t => [t.sessionId, t.connection] as const));

  for (const targetSessionId of targets) {
    const targetConnection = connectionBySessionId.get(targetSessionId) ?? connection;
    if (!targetConnection) continue;
    handleTerminalInputSingle(targetSessionId, data);
  }
}

export async function sendTerminalResize(sessionId: string, width: number, height: number) {
  try {
    await invoke('resize_terminal', { sessionId, width, height });
    startedTerminalSessions.add(sessionId);
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    if (msg.includes('Session not found')) {
      log.warn('TermResize', 'Session not found', error, { sessionId });
      return;
    }
    log.error('TermResize', 'Failed to resize terminal', error, { sessionId, width, height });
  }
}

export async function monitorSessionStatus(sessionId: string) {
  try {
    // Listen for session status changes from backend
    const statusUnlisten = await listen(`session-status-${sessionId}`, (event: any) => {
      if (event.payload && event.payload.status) {
        log.info('SessionStatus', 'Status changed', {
          sessionId,
          status: event.payload.status,
        });

        // Handle different status changes
        switch (event.payload.status) {
          case 'disconnected':
            errorMessage.set(`会话已断开: ${sessionId}`);
            setTimeout(() => errorMessage.set(null), 5000);
            // Optionally attempt to reconnect
            break;
          case 'error':
            if (event.payload.error) {
              errorMessage.set(`会话错误: ${event.payload.error}`);
              setTimeout(() => errorMessage.set(null), 5000);
            }
            break;
          case 'connected':
            successMessage.set('会话已连接');
            setTimeout(() => successMessage.set(null), 3000);
            break;
        }
      }
    });

    sessionStatusListeners.set(sessionId, statusUnlisten);
  } catch (error) {
    log.error('SessionStatus', 'Failed to monitor session status', error, { sessionId });
  }
}

/**
 * xterm 6.0: Enhanced terminal cleanup with better resource management
 */

export async function closeSplitSession(sessionId: string) {
  try {
    await closeDetachedTerminal(sessionId);
    await invoke('disconnect', { sessionId });
    
    // Clean up listeners
    const inputListener = inputListeners.get(sessionId);
    if (inputListener) {
      inputListener.dispose();
      inputListeners.delete(sessionId);
    }
    
    const unlisten = outputListeners.get(sessionId);
    if (unlisten) {
      unlisten();
      outputListeners.delete(sessionId);
    }
    
    log.info('TermCleanup', 'Split session closed', { sessionId });
  } catch (e) {
    log.error('TermCleanup', 'Failed to close split session', e);
  }
}

export async function closeTerminal(sessionId: string) {
  const terminals = get(activeTerminals);
  const index = terminals.findIndex(t => t.sessionId === sessionId);

  if (index !== -1) {
    markTerminalStopped(sessionId);
    const terminal = terminals[index];

    // xterm 6.0: Graceful terminal disposal with cleanup tracking
    try {
      if (terminal.terminal) {
        // Dispose all addons first
        const disposables: Array<() => void | Promise<void>> = [];
        if ((terminal.terminal as any).dispose) {
          disposables.push(() => terminal.terminal.dispose());
        }

        // Execute disposals in order
        for (const dispose of disposables) {
          try {
            await dispose();
      } catch (e) {
        log.warn('TermCleanup', 'Disposal error', e);
      }
        }
      }
    } catch (e) {
      log.warn('TermCleanup', 'Failed to dispose terminal instance', e);
    }

    // xterm 6.0: Clean up output state
    const outputState = outputWriteStates.get(sessionId);
    if (outputState) {
      outputState.disposed = true;
      if (outputState.scheduled !== null) {
        cancelScheduled(outputState.scheduled);
      }
      outputWriteStates.delete(sessionId);
    }

    // xterm 6.0: Clean up input state
    const inputState = inputSendStates.get(sessionId);
    if (inputState) {
      if (inputState.timer !== null) {
        cancelScheduled(inputState.timer);
      }
      inputSendStates.delete(sessionId);
    }

    // Remove from store
    activeTerminals.update(items => items.filter(t => t.sessionId !== sessionId));
    broadcastSessionIds.update(ids => ids.filter(id => id !== sessionId));
    
    // Remove from session map
    terminalSessionMap.update(map => {
      map.delete(sessionId);
      return map;
    });

    // Adjust selected index
    const currentIndex = get(selectedTerminalIndex);
    if (currentIndex >= terminals.length - 1) {
      selectedTerminalIndex.set(Math.max(0, terminals.length - 2));
    }

    // Clean up listeners
    const unlisten = outputListeners.get(sessionId);
    if (unlisten) {
      unlisten();
      outputListeners.delete(sessionId);
    }

    const inputListener = inputListeners.get(sessionId);
    if (inputListener) {
      inputListener.dispose();
      inputListeners.delete(sessionId);
    }

    const statusUnlisten = sessionStatusListeners.get(sessionId);
    if (statusUnlisten) {
      statusUnlisten();
      sessionStatusListeners.delete(sessionId);
    }

    // Notify backend to close terminal
    try {
      await invoke('close_terminal', { sessionId });
      log.info('TermCleanup', 'Terminal closed', { sessionId });
    } catch (error) {
      log.error('TermCleanup', 'Failed to close terminal', error, { sessionId });
    }
  }
}

export async function closeDetachedTerminal(sessionId: string) {
  markTerminalStopped(sessionId);

  const outputState = outputWriteStates.get(sessionId);
  if (outputState) {
    outputState.disposed = true;
    if (outputState.scheduled !== null) {
      cancelScheduled(outputState.scheduled);
    }
    outputWriteStates.delete(sessionId);
  }

  const inputState = inputSendStates.get(sessionId);
  if (inputState) {
    if (inputState.timer !== null) {
      cancelScheduled(inputState.timer);
    }
    inputSendStates.delete(sessionId);
  }

  const unlisten = outputListeners.get(sessionId);
  if (unlisten) {
    unlisten();
    outputListeners.delete(sessionId);
  }

  const inputListener = inputListeners.get(sessionId);
  if (inputListener) {
    inputListener.dispose();
    inputListeners.delete(sessionId);
  }

  const statusUnlisten = sessionStatusListeners.get(sessionId);
  if (statusUnlisten) {
    statusUnlisten();
    sessionStatusListeners.delete(sessionId);
  }

  try {
    await invoke('close_terminal', { sessionId });
  } catch (error) {
    log.warn('TermCleanup', 'Failed to close detached terminal', error, { sessionId });
  }
}

export async function disconnectTerminal(sessionId: string) {
  const terminals = get(activeTerminals);
  const terminal = terminals.find(t => t.sessionId === sessionId);
  const name = terminal?.connection?.name ?? sessionId;
  const hadTerminal = Boolean(terminal);

  clearReconnectTimer(sessionId);
  clearReconnectKeyListener(sessionId);
  reconnectAttempts.delete(sessionId);

  await closeTerminal(sessionId);

  if (!hadTerminal) {
    markTerminalStopped(sessionId);
    try {
      await invoke('close_terminal', { sessionId });
    } catch (error) {
      log.warn('Disconnect', 'Failed to close_terminal during disconnect', error, { sessionId });
    }
  }

  try {
    await invoke('disconnect', { sessionId });
    successMessage.set(`已断开连接: ${name}`);
    setTimeout(() => successMessage.set(null), 3000);
  } catch (error) {
    errorMessage.set(`断开连接失败: ${name}`);
    setTimeout(() => errorMessage.set(null), 5000);
    log.warn('Disconnect', 'Failed to disconnect', error, { sessionId, name });
  }
}

export async function closeAllTerminals() {
  const terminals = get(activeTerminals);
  const sessionIds = terminals.map(t => t.sessionId);
  for (const sessionId of sessionIds) {
    await closeBackendTerminalsBestEffort(sessionId);
  }
}

async function closeBackendTerminalsBestEffort(sessionId: string) {
  for (let i = 0; i < 10; i++) {
    try {
      await invoke('close_terminal', { sessionId });
    } catch (error) {
      log.warn('TermCleanup', 'Failed to close terminal in best-effort mode', error, { sessionId, attempt: i });
      return;
    }
  }
}

type BackendSessionInfo = {
  id: string;
  connection_id: string;
  status: string;
  terminal_id?: string | null;
};

export async function restoreActiveSessions() {
  const current = get(activeTerminals);
  if (current.length > 0) return;

  let sessions: BackendSessionInfo[] = [];
  try {
    sessions = (await invoke('get_all_sessions')) as BackendSessionInfo[];
  } catch (e) {
    log.warn('SessionRestore', 'Failed to get_all_sessions', e);
    return;
  }

  const connectionList = get(connections);
  const connectionById = new Map(connectionList.map(c => [c.id, c] as const));

  const connectedSessions = sessions.filter(s => {
    const status = String((s as any)?.status ?? '');
    return status.toLowerCase() === 'connected';
  });

  const uiState = getStoredTerminalUiState();
  const orderIndex = new Map(uiState.order.map((id, i) => [id, i] as const));

  const entries = connectedSessions
    .map(s => {
      const sessionId = String((s as any)?.id ?? '');
      const connectionId = String((s as any)?.connection_id ?? '');
      if (!sessionId || !connectionId) return null;
      const connection = connectionById.get(connectionId);
      if (!connection) return null;
      return {
        sessionId,
        connection,
        terminal: null as any,
        fitAddon: null as any,
        searchAddon: null as any,
      } satisfies ActiveTerminal;
    })
    .filter(Boolean) as ActiveTerminal[];

  entries.sort((a, b) => {
    const ai = orderIndex.get(a.sessionId);
    const bi = orderIndex.get(b.sessionId);
    if (ai === undefined && bi === undefined) return a.connection.name.localeCompare(b.connection.name, 'zh-Hans-CN');
    if (ai === undefined) return 1;
    if (bi === undefined) return -1;
    return ai - bi;
  });

  if (entries.length === 0) return;

  for (const entry of entries) {
    await closeBackendTerminalsBestEffort(entry.sessionId);
  }

  activeTerminals.set(entries);

  const selectedSessionId = uiState.selectedSessionId;
  if (selectedSessionId) {
    const idx = entries.findIndex(e => e.sessionId === selectedSessionId);
    selectedTerminalIndex.set(idx >= 0 ? idx : 0);
  } else {
    selectedTerminalIndex.set(0);
  }
}

export async function setupTerminalListeners(sessionId: string, term: Terminal) {
    log.info('TermOutput', 'Setting up terminal listeners', { sessionId });

    const outputState: OutputWriteState = {
      chunks: [],
      chunkIndex: 0,
      scheduled: null,
      writing: false,
      disposed: false,
      chunkBudget: 256 * 1024,
      lastWriteTime: 0,
      consecutiveSlowWrites: 0,
    };
    outputWriteStates.set(sessionId, outputState);

    const TARGET_WRITE_MS = 12;
    const MIN_CHUNK_SIZE = 1024; // xterm 6.0: Optimize minimum chunk size for better batching

    // Immediately write a test message to verify terminal is working
    void term.write('\r\n\x1b[33mTerminal initialized...\x1b[0m\r\n');
    log.info('TermOutput', 'Test message written to terminal', { sessionId });

    /**
     * xterm 6.0: Optimized flush using Promise-based write API
     * - Improved batching with adaptive chunk sizing
     * - Better memory management with aggressive pruning
     * - Enhanced performance monitoring and adaptive budgeting
     */
    async function flushOutput() {
      if (outputState.disposed) return;
      outputState.scheduled = null;
      if (outputState.writing) return;
      if (outputState.chunkIndex >= outputState.chunks.length) return;

      // Debug log on first few flushes
      if (IS_DEV && outputState.chunkIndex === 0) {
        log.info('TermOutput', 'Starting flush', {
          sessionId,
          totalChunks: outputState.chunks.length,
          chunkBudget: outputState.chunkBudget,
        });
      }

      const CHUNK_LIMIT = outputState.chunkBudget;
      let count = 0;
      const parts: string[] = [];

      // xterm 6.0: Improved chunking logic with minimum size threshold
      while (outputState.chunkIndex < outputState.chunks.length) {
        const nextChunk = outputState.chunks[outputState.chunkIndex];
        const wouldExceed = count + nextChunk.length > CHUNK_LIMIT && count > 0;

        // Don't break if we haven't reached minimum chunk size yet
        if (wouldExceed && count >= MIN_CHUNK_SIZE) break;

        parts.push(nextChunk);
        count += nextChunk.length;
        outputState.chunkIndex += 1;

        if (count >= CHUNK_LIMIT) break;
      }

      // xterm 6.0: Aggressive memory pruning to prevent bloat
      if (outputState.chunks.length > 4096 || outputState.chunkIndex > Math.floor(outputState.chunks.length * 0.7)) {
        outputState.chunks.splice(0, outputState.chunkIndex);
        outputState.chunkIndex = 0;
      }

      const payload = parts.join('').split('\u0000').join('');
      if (payload.length === 0) return;

      // Debug log before writing
      if (IS_DEV && outputState.chunkIndex <= 2) {
        log.info('TermOutput', 'Writing to terminal', {
          sessionId,
          payloadLength: payload.length,
          payloadPreview: payload.substring(0, 50),
        });
      }

      outputState.writing = true;
      const writeStart = nowMs();
      try {
        // xterm 6.0: Promise-based write API provides better error handling
        await term.write(payload);
        const writeDuration = nowMs() - writeStart;
        outputState.lastWriteTime = writeDuration;

        // xterm 6.0: Adaptive budgeting with improved responsiveness
        if (writeDuration > TARGET_WRITE_MS) {
          outputState.consecutiveSlowWrites++;
          const reductionFactor = 0.5 + (0.2 / Math.max(1, outputState.consecutiveSlowWrites));
          outputState.chunkBudget = Math.max(32 * 1024, Math.floor(outputState.chunkBudget * reductionFactor));

          log.perf('TermOutput', 'write', writeDuration, {
            sessionId,
            payloadLength: payload.length,
            pendingChunks: outputState.chunks.length - outputState.chunkIndex,
            budget: outputState.chunkBudget,
            consecutiveSlowWrites: outputState.consecutiveSlowWrites,
          });
        } else if (writeDuration < TARGET_WRITE_MS / 2) {
          // Reset consecutive counter on good performance
          if (outputState.consecutiveSlowWrites > 0) {
            outputState.consecutiveSlowWrites = Math.max(0, outputState.consecutiveSlowWrites - 1);
          }
          outputState.chunkBudget = Math.min(2 * 1024 * 1024, Math.floor(outputState.chunkBudget * 1.1));
        }

        outputState.writing = false;
        if (outputState.chunkIndex < outputState.chunks.length && !outputState.disposed) {
          scheduleFlush();
        }
      } catch (error) {
        // xterm 6.0: Better error handling with context
        log.error('TermOutput', `write failed for session ${sessionId}`, error, {
          payloadLength: payload.length,
          pendingChunks: outputState.chunks.length - outputState.chunkIndex,
          budget: outputState.chunkBudget,
        });
        outputState.writing = false;
        // xterm 6.0: Add a small delay before retrying to prevent error storms
        const retryDelay = Math.min(100, outputState.consecutiveSlowWrites * 10);
        outputState.scheduled = { id: setTimeout(() => scheduleFlush(), retryDelay) as unknown as number, kind: 'timeout' };
      }
    }

    const scheduleFlush = () => {
      if (outputState.disposed) return;
      if (outputState.scheduled !== null) return;
      outputState.scheduled = scheduleNext(flushOutput);
    };

    // Output listener
    const outputUnlisten = await listen(`terminal-output-${sessionId}`, (event: any) => {
      // Debug log to verify events are received
      if (IS_DEV) {
        log.info('TermOutput', 'Received output event', {
          sessionId,
          hasPayload: !!event.payload,
          hasData: !!(event.payload && event.payload.data),
          dataLength: event.payload?.data?.length || 0,
        });
      }


      if (event.payload && event.payload.data) {
        const data = String(event.payload.data);

        outputState.chunks.push(data);
        if (IS_DEV && outputState.chunks.length <= 5) {
          log.info('TermOutput', 'Chunk added', {
            sessionId,
            chunkIndex: outputState.chunks.length - 1,
            isWriting: outputState.writing,
          });
        }
        if (!outputState.writing) scheduleFlush();
      }
    });

    // Error listener
    const errorUnlisten = await listen(`terminal-error-${sessionId}`, (event: any) => {
      if (event.payload && event.payload.error) {
        log.error('TermError', 'Terminal error reported from backend', event.payload.error, { sessionId });
        void term.write(`\r\n\x1b[31mError: ${event.payload.error}\x1b[0m\r\n`);
        errorMessage.set(`终端错误: ${event.payload.error}`);
        setTimeout(() => errorMessage.set(null), 5000);
      }
    });

    // Session Closed listener
    const closedUnlisten = await listen(`session-closed-${sessionId}`, (event: any) => {
        const reason = event.payload?.reason || 'unknown';
        log.info('SessionClosed', 'Session closed', { sessionId, reason });
        markTerminalStopped(sessionId);

        // Only show message if not manually closed
        if (reason !== 'user_closed') {
             void term.write(`\r\n\x1b[33mSession closed (Reason: ${reason})\x1b[0m\r\n`);
        }

        if (reason === 'connection_lost' || reason === 'server_closed' || reason === 'keepalive_failed') {
            const appSettings = get(settings);
            if (appSettings.connection?.autoReconnect) {
                scheduleAutoReconnect(sessionId, term, false);
            } else {
                void term.write('\r\n\x1b[36mPress R to reconnect...\x1b[0m\r\n');

                const disposable = term.onData(async (data) => {
                    if (data === 'r' || data === 'R') {
                        disposable.dispose();
                        await reconnectTerminal(sessionId);
                    }
                });
            }
        }
    });

    outputListeners.set(sessionId, () => {
        outputState.disposed = true;
        if (outputState.scheduled !== null) {
          cancelScheduled(outputState.scheduled);
        }
        outputWriteStates.delete(sessionId);

        outputUnlisten();
        errorUnlisten();
        closedUnlisten();
    });
}

function clearReconnectTimer(sessionId: string) {
  const timer = reconnectTimers.get(sessionId);
  if (timer !== undefined) {
    clearTimeout(timer);
    reconnectTimers.delete(sessionId);
  }
}

function clearReconnectKeyListener(sessionId: string) {
  const listener = reconnectKeyListeners.get(sessionId);
  if (listener) {
    listener.dispose();
    reconnectKeyListeners.delete(sessionId);
  }
}

function scheduleAutoReconnect(sessionId: string, term: Terminal, immediate: boolean) {
  clearReconnectTimer(sessionId);
  clearReconnectKeyListener(sessionId);

  const attempts = reconnectAttempts.get(sessionId) ?? 0;
  if (attempts >= MAX_AUTO_RECONNECT_RETRIES) {
    void term.write(`\r\n\x1b[31mAuto reconnect stopped after ${MAX_AUTO_RECONNECT_RETRIES} attempts.\x1b[0m\r\n`);
    errorMessage.set('自动重连已停止：超过最大重试次数');
    setTimeout(() => errorMessage.set(null), 5000);
    return;
  }

  const delay = immediate
    ? 0
    : Math.min(MAX_AUTO_RECONNECT_DELAY_MS, Math.floor(BASE_AUTO_RECONNECT_DELAY_MS * Math.pow(2, attempts)));
  const seconds = Math.max(0, Math.ceil(delay / 1000));
  void term.write(`\r\n\x1b[33mAuto reconnect attempt ${attempts + 1}/${MAX_AUTO_RECONNECT_RETRIES} in ${seconds}s... (Press R to immediate)\x1b[0m\r\n`);

  const disposable = term.onData((data) => {
    if (data === 'r' || data === 'R') {
      disposable.dispose();
      reconnectKeyListeners.delete(sessionId);
      scheduleAutoReconnect(sessionId, term, true);
    }
  });
  reconnectKeyListeners.set(sessionId, disposable);

  const timer = window.setTimeout(async () => {
    reconnectTimers.delete(sessionId);
    clearReconnectKeyListener(sessionId);

    reconnectAttempts.set(sessionId, attempts + 1);
    try {
      const ok = await reconnectTerminal(sessionId);
      if (ok) {
        reconnectAttempts.delete(sessionId);
        return;
      }
    } catch (e) {
      log.warn('Reconnect', 'Auto reconnect attempt failed', e, {
        sessionId,
        attempt: reconnectAttempts.get(sessionId) || 0,
      });
    }

    const appSettings = get(settings);
    if (appSettings.connection?.autoReconnect) {
      scheduleAutoReconnect(sessionId, term, false);
    }
  }, delay);

  reconnectTimers.set(sessionId, timer);
}

export async function reconnectTerminal(oldSessionId: string) {
  const terminals = get(activeTerminals);
  const index = terminals.findIndex(t => t.sessionId === oldSessionId);
  if (index === -1) return false;

  const terminalEntry = terminals[index];
  const term = terminalEntry.terminal;
  
  // Clean up old listeners
  const unlisten = outputListeners.get(oldSessionId);
  if (unlisten) {
      unlisten();
      outputListeners.delete(oldSessionId);
  }

  const oldInputListener = inputListeners.get(oldSessionId);
  if (oldInputListener) {
      oldInputListener.dispose();
      inputListeners.delete(oldSessionId);
  }

  void term.write('\r\n\x1b[33mReconnecting...\x1b[0m\r\n');

  try {
      // Connect
      const newSessionId = await invoke('connect', { config: terminalEntry.connection }) as string;
      
      // Start terminal
      const result = await invoke('start_terminal', {
          sessionId: newSessionId,
          width: term.cols,
          height: term.rows,
      });

      if (!result) throw new Error("Failed to start terminal");

      markTerminalStopped(oldSessionId);
      markTerminalStarted(newSessionId);

      // Update store
      activeTerminals.update(items => {
          const newItems = [...items];
          newItems[index] = { ...terminalEntry, sessionId: newSessionId };
          return newItems;
      });
      broadcastSessionIds.update(ids => ids.map(id => (id === oldSessionId ? newSessionId : id)));
      reconnectAttempts.delete(oldSessionId);
      clearReconnectTimer(oldSessionId);
      clearReconnectKeyListener(oldSessionId);
      
      // Setup new listeners
      await setupTerminalListeners(newSessionId, term);

      // Setup new input listener
      const newInputListener = term.onData((data) => {
        handleTerminalInput(newSessionId, data, terminalEntry.connection);
      });
      inputListeners.set(newSessionId, newInputListener);
      
      term.write('\r\n\x1b[32mReconnected!\x1b[0m\r\n');
      successMessage.set(`已重连: ${terminalEntry.connection.name}`);
      setTimeout(() => successMessage.set(null), 3000);
      term.focus();
      
      // Trigger resize
      await sendTerminalResize(newSessionId, term.cols, term.rows);

      return true;
  } catch (e) {
      void term.write(`\r\n\x1b[31mReconnection failed: ${e}\x1b[0m\r\n`);
      errorMessage.set(`重连失败: ${terminalEntry.connection.name}`);
      setTimeout(() => errorMessage.set(null), 5000);
      return false;
  }
}
