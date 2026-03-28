import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Terminal } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import { SearchAddon } from 'xterm-addon-search';
import { WebglAddon } from 'xterm-addon-webgl';
import { WebLinksAddon } from 'xterm-addon-web-links';
import { get } from 'svelte/store';
import {
  activeTerminals,
  connections,
  selectedTerminalIndex,
  type Connection,
  type ActiveTerminal,
  type AppSettings,
  settings,
  connectionHistory,
  broadcastInputEnabled,
  broadcastSessionIds,
  getStoredTerminalUiState,
  getXtermTheme,
  getBaseXtermTheme,
  terminalSessionMap,
  connectingConnections,
  clearErrorMessage,
  showErrorMessage,
  showSuccessMessage,
} from './store';
import { auditService } from './auditService';
import { terminalPool } from './terminalPool';
import { TerminalInstance } from './terminalInstance';
import {
  buildHostKeyConfirmMessage,
  parseHostKeyPrompt,
  saveHostKeyPrompt,
} from './hostKeyPrompt';
import 'xterm/css/xterm.css';

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
    showErrorMessage(userMessage, 5000);
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
const reconnectInFlight = new Map<string, Promise<boolean>>();
const reconnectSuppressed = new Set<string>();
const reconnectConnectConfigs = new Map<string, any>();
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
 * Helper to convert hex to rgba
 */
function hexToRgba(hex: string, alpha: number): string {
  const cleanHex = hex.replace('#', '');
  const r = parseInt(cleanHex.substring(0, 2), 16);
  const g = parseInt(cleanHex.substring(2, 4), 16);
  const b = parseInt(cleanHex.substring(4, 6), 16);
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * Apply scrollbar color to match terminal theme background
 */
export function applyScrollbarColor(appSettings: AppSettings): void {
  log.info('Scrollbar', 'Updating scrollbar colors for terminal');
  const theme = getBaseXtermTheme(appSettings);
  let terminalBg = theme.background || '#0f172a';
  
  // If background image is present, force terminal background to be transparent
  // This ensures the background image behind the terminal is visible
  if (appSettings.appearance?.backgroundImage) {
    terminalBg = 'rgba(0,0,0,0)';
  } else {
    // Apply opacity if no background image
    const opacity = appSettings.appearance.backgroundOpacity ?? 1;
    if (opacity < 1 && terminalBg.startsWith('#')) {
       terminalBg = hexToRgba(terminalBg, opacity);
    }
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
export function calculateBrightness(color: string): number {
  const hex = color.replace('#', '');
  // Handle short hex codes (e.g. #fff)
  if (hex.length === 3) {
    const r = parseInt(hex[0] + hex[0], 16);
    const g = parseInt(hex[1] + hex[1], 16);
    const b = parseInt(hex[2] + hex[2], 16);
    return (r * 299 + g * 587 + b * 114) / 1000;
  }
  
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
  disposed: boolean;
  lastFlushTime: number;
  pendingChunks: number;
};

const outputWriteStates = new Map<string, OutputWriteState>();
const inputSendStates = new Map<string, InputSendState>();
const commandAuditBuffers = new Map<string, string>();
// Populated by restoreActiveSessions() so initDetachedTerminal can re-attach to an
// already-started backend terminal without calling start_terminal again.
const restoredSessionsWithBackendTerminal = new Set<string>();

function cleanupBufferedTerminalState(sessionId: string) {
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
    inputState.disposed = true;
    inputState.buffer = '';
    if (inputState.timer !== null) {
      cancelScheduled(inputState.timer);
    }
    inputSendStates.delete(sessionId);
  }

  commandAuditBuffers.delete(sessionId);
  reconnectConnectConfigs.delete(sessionId);
}

function cleanupTerminalListeners(sessionId: string) {
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
}

function nowMs(): number {
  if (typeof performance !== 'undefined' && typeof performance.now === 'function') return performance.now();
  return Date.now();
}

function cloneConnectConfig(config: any): any {
  if (typeof structuredClone === 'function') {
    try {
      return structuredClone(config);
    } catch {
      // fall through to JSON clone
    }
  }
  try {
    return JSON.parse(JSON.stringify(config));
  } catch {
    return config;
  }
}

function rememberReconnectConfig(sessionId: string, connectConfig: any) {
  reconnectConnectConfigs.set(sessionId, cloneConnectConfig(connectConfig));
}

function writeTerminalAsync(term: Terminal, data: string): Promise<void> {
  return new Promise((resolve, reject) => {
    try {
      term.write(data, () => resolve());
    } catch (error) {
      reject(error);
    }
  });
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


async function invokeWithTimeout<T>(cmd: string, args: any, timeoutMs: number = 30000): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`连接超时 (${timeoutMs / 1000}秒)`));
    }, timeoutMs);

    invoke(cmd, args)
      .then((res) => {
        clearTimeout(timer);
        resolve(res as T);
      })
      .catch((err) => {
        clearTimeout(timer);
        reject(err);
      });
  });
}

type ConnectWithPromptsResult = {
  sessionId: string;
  connectConfig: any;
};

async function connectWithInteractivePrompts(config: any, connectionName: string): Promise<ConnectWithPromptsResult> {
  let connectConfig = await ensureConnectConfig(config, connectionName);
  try {
    const sessionId = await connectWithKnownHostsPrompt(connectConfig);
    return { sessionId, connectConfig };
  } catch (error) {
    if (!shouldPromptForPasswordOnConnectError(error, connectConfig)) {
      throw error;
    }
    const prompted = await promptPassword(connectionName);
    connectConfig = applyPromptedPassword(connectConfig, prompted);
    const sessionId = await connectWithKnownHostsPrompt(connectConfig);
    return { sessionId, connectConfig };
  }
}

export async function connectAndOpen(connection: Connection, connectConfig?: any) {
  try {
    clearErrorMessage();
    connectingConnections.update(s => {
      s.add(connection.id);
      return s;
    });

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

      showSuccessMessage(`已启动 RDP: ${connection.name}`, 3000);
      connectingConnections.update(s => {
        s.delete(connection.id);
        return s;
      });
      return;
    }

    if (protocol === 'Telnet') {
      const sessionId = await invokeWithTimeout<string>('connect', { config: baseConfig });
      rememberReconnectConfig(sessionId, baseConfig);

      let nextSelectedIndex = -1;
      let alreadyExists = false;
      activeTerminals.update(items => {
        const existingIndex = items.findIndex(t => t.sessionId === sessionId);
        if (existingIndex !== -1) {
          nextSelectedIndex = existingIndex;
          alreadyExists = true;
          return items;
        }

        nextSelectedIndex = items.length;
        return [
          ...items,
          {
            sessionId,
            connection,
            terminal: null as any,
            fitAddon: null as any,
            searchAddon: null as any
          }
        ];
      });
      if (nextSelectedIndex >= 0) {
        selectedTerminalIndex.set(nextSelectedIndex);
      }
      if (alreadyExists) {
        connectingConnections.update(s => {
          s.delete(connection.id);
          return s;
        });
        return;
      }

      connectionHistory.update(history => {
        const newHistory = history.filter(h => h.connection.id !== connection.id);
        newHistory.unshift({
          connection,
          lastConnected: Date.now()
        });
        return newHistory.slice(0, 50);
      });

      showSuccessMessage(`连接成功: ${connection.name}`, 3000);
      connectingConnections.update(s => {
        s.delete(connection.id);
        return s;
      });
      return;
    }

    const { sessionId, connectConfig: effectiveConnectConfig } = await connectWithInteractivePrompts(baseConfig, connection.name);
    rememberReconnectConfig(sessionId, effectiveConnectConfig);
    
    let nextSelectedIndex = -1;
    let alreadyExists = false;
    activeTerminals.update(items => {
      const existingIndex = items.findIndex(t => t.sessionId === sessionId);
      if (existingIndex !== -1) {
        nextSelectedIndex = existingIndex;
        alreadyExists = true;
        return items;
      }

      nextSelectedIndex = items.length;
      return [
        ...items,
        {
          sessionId,
          connection,
          terminal: null as any, // Will be initialized by view
          fitAddon: null as any,
          searchAddon: null as any
        }
      ];
    });
    if (nextSelectedIndex >= 0) {
      selectedTerminalIndex.set(nextSelectedIndex);
    }

    if (alreadyExists) {
      connectingConnections.update(s => {
        s.delete(connection.id);
        return s;
      });
      return;
    }

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

    showSuccessMessage(`连接成功: ${connection.name}`, 3000);
    connectingConnections.update(s => {
      s.delete(connection.id);
      return s;
    });
    
  } catch (error) {
    connectingConnections.update(s => {
      s.delete(connection.id);
      return s;
    });
    log.error('Connection', `Failed to connect to ${connection.name}`, error);
    const msg = normalizeErrorMessage(error);
    showErrorMessage(`连接失败：${msg}`, 5000);
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
  const { sessionId, connectConfig } = await connectWithInteractivePrompts(connection, connection.name);
  rememberReconnectConfig(sessionId, connectConfig);
  return sessionId;
}

async function connectWithKnownHostsPrompt(connection: Connection | Record<string, unknown>): Promise<string> {
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      const result = await invokeWithTimeout<string>('connect', { config: connection });
      return result as string;
    } catch (error) {
      const parsed = parseHostKeyPrompt(error);
      if (!parsed) throw error;

      const confirmed = window.confirm(buildHostKeyConfirmMessage(parsed));
      if (!confirmed) {
        throw error;
      }

      await saveHostKeyPrompt(parsed);
    }
  }

  const result = await invokeWithTimeout<string>('connect', { config: connection });
  return result as string;
}

type TerminalInitMode = 'attached' | 'detached';

type TerminalInitCommon = {
  appSettings: AppSettings;
  term: Terminal;
  fitAddon: FitAddon;
  searchAddon: SearchAddon;
  webglAddon: WebglAddon | null;
};

function resetTerminalInitState(sessionId: string, term?: Terminal | null) {
  cleanupTerminalListeners(sessionId);
  cleanupBufferedTerminalState(sessionId);
  terminalPool.destroyInstance(sessionId);
  if (term) {
    try {
      term.dispose();
    } catch {
      // Ignore disposal failures while resetting a terminal instance.
    }
  }
}

function createTerminalForMode(appSettings: AppSettings, mode: TerminalInitMode): Terminal {
  const options: ConstructorParameters<typeof Terminal>[0] = {
    cursorBlink: appSettings.terminal.cursorBlink,
    cursorStyle: appSettings.terminal.cursorStyle,
    cursorWidth: 1,
    fontSize: appSettings.terminal.fontSize,
    fontFamily: appSettings.terminal.fontFamily,
    theme: getXtermTheme(appSettings),
    scrollback: appSettings.terminal.scrollback,
    allowProposedApi: true,
    allowTransparency: true,
    convertEol: true,
    altClickMovesCursor: true,
    scrollSensitivity: 1,
    fastScrollSensitivity: 5,
    rightClickSelectsWord: true,
    macOptionIsMeta: false,
  };

  if (mode === 'attached') {
    const baseTheme = getBaseXtermTheme(appSettings);
    const bgBrightness = calculateBrightness(baseTheme.background || '#000000');
    const isLightTheme = bgBrightness > 128;
    options.fontWeight = isLightTheme ? '600' : 'normal';
    options.fontWeightBold = isLightTheme ? 'bold' : 'bold';
  }

  return new Terminal(options);
}

function attachTerminalAddons(term: Terminal): { fitAddon: FitAddon; searchAddon: SearchAddon } {
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
  return { fitAddon, searchAddon };
}

function attachWebglAddon(term: Terminal, mode: TerminalInitMode): WebglAddon | null {
  let webglAddon: WebglAddon | null = null;
  try {
    webglAddon = new WebglAddon();
    term.loadAddon(webglAddon);
    if (mode === 'attached') {
      log.info('TermInit', 'WebGL renderer loaded successfully');
    }
    webglAddon.onContextLoss(() => {
      if (mode === 'attached') {
        log.warn('TermInit', 'WebGL context lost, falling back to canvas');
      }
      try {
        webglAddon?.dispose();
      } catch {
        webglAddon = null;
      }
    });
  } catch (error) {
    if (mode === 'attached') {
      log.warn('TermInit', 'WebGL addon unavailable, using canvas fallback', error);
    }
  }
  return webglAddon;
}

function waitForLayoutTick(): Promise<void> {
  return new Promise<void>((resolve) => {
    if (typeof requestAnimationFrame === 'function') {
      requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
      return;
    }
    const timeout = (typeof window !== 'undefined' ? window.setTimeout : setTimeout) as typeof setTimeout;
    timeout(() => resolve(), 0);
  });
}

async function fitTerminalLayout(
  container: HTMLElement,
  fitAddon: FitAddon,
  mode: TerminalInitMode
): Promise<void> {
  let didFit = false;
  for (let i = 0; i < 10; i++) {
    await waitForLayoutTick();
    const rect = container.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      fitAddon.fit();
      didFit = true;
      if (mode === 'attached' && i > 0) {
        log.info('TermInit', `Layout stabilized after ${i + 1} attempts`);
      }
      break;
    }
  }

  if (!didFit && mode === 'attached') {
    const rect = container.getBoundingClientRect();
    if (rect.width > 0 && rect.height > 0) {
      fitAddon.fit();
      log.warn('TermInit', 'Forcing fit on unready layout');
    }
  }
}

async function initializeTerminalCore(
  container: HTMLElement,
  sessionId: string,
  connection: Connection,
  mode: TerminalInitMode
): Promise<TerminalInitCommon> {
  resetTerminalInitState(sessionId);
  container.innerHTML = '';

  const appSettings = get(settings);
  const term = createTerminalForMode(appSettings, mode);
  const { fitAddon, searchAddon } = attachTerminalAddons(term);
  const terminalInstance = TerminalInstance.fromInitialized(sessionId, term, fitAddon, searchAddon);
  terminalPool.registerInstance(terminalInstance);
  log.info('TermInit', mode === 'attached' ? 'Terminal instance registered to pool' : 'Detached terminal instance registered to pool', { sessionId });

  term.open(container);
  if (mode === 'attached') {
    log.info('TermInit', 'Terminal opened in container', {
      sessionId,
      containerSize: {
        width: container.clientWidth,
        height: container.clientHeight,
      },
      terminalSize: { cols: term.cols, rows: term.rows },
    });
  }

  const webglAddon = attachWebglAddon(term, mode);
  await fitTerminalLayout(container, fitAddon, mode);

  const inputDisposable = term.onData((data: string) => {
    handleTerminalInput(sessionId, data, connection);
  });
  inputListeners.set(sessionId, inputDisposable);
  await setupTerminalListeners(sessionId, term);

  return { appSettings, term, fitAddon, searchAddon, webglAddon };
}

async function startBackendTerminalSession(
  sessionId: string,
  term: Terminal,
  logFailure: boolean
): Promise<boolean> {
  if (restoredSessionsWithBackendTerminal.has(sessionId)) {
    // Renderer was restarted while backend terminal still exists; we can re-attach
    // by setting up listeners only. Do NOT call start_terminal again.
    markTerminalStarted(sessionId);
    return true;
  }

  let result: unknown;
  try {
    result = await invoke('start_terminal', {
      sessionId,
      width: term.cols,
      height: term.rows,
    });
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    // Be tolerant to backend already having a terminal attached (e.g. restore path).
    if (msg.toLowerCase().includes('already started')) {
      markTerminalStarted(sessionId);
      return true;
    }

    if (logFailure) {
      log.error('TermInit', 'Failed to start terminal session', error);
    }
    void term.write('\r\n\x1b[31mFailed to start terminal session\x1b[0m\r\n');
    showErrorMessage('启动终端会话失败', 5000);
    return false;
  }

  if (result) return true;
  if (logFailure) {
    log.error('TermInit', 'Failed to start terminal session', null);
  }
  void term.write('\r\n\x1b[31mFailed to start terminal session\x1b[0m\r\n');
  showErrorMessage('启动终端会话失败', 5000);
  return false;
}

export async function initTerminal(container: HTMLElement, sessionId: string, connection: Connection): Promise<ActiveTerminal | null> {
  let initialized: TerminalInitCommon | null = null;
  try {
    initialized = await initializeTerminalCore(container, sessionId, connection, 'attached');
    const { appSettings, term, fitAddon, searchAddon, webglAddon } = initialized;
    const started = await startBackendTerminalSession(sessionId, term, true);
    if (!started) {
      resetTerminalInitState(sessionId, term);
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

    log.info('TermInit', 'About to call applyScrollbarColor', {
      sessionId,
      containerExists: !!container,
      appSettingsTerminalTheme: appSettings.appearance?.terminalTheme,
    });
    applyScrollbarColor(appSettings);

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
    resetTerminalInitState(sessionId, initialized?.term);
    handleTerminalError(error, 'initTerminal', '初始化终端失败');
    return null;
  }
}

export async function initDetachedTerminal(container: HTMLElement, sessionId: string, connection: Connection): Promise<ActiveTerminal | null> {
  let initialized: TerminalInitCommon | null = null;
  try {
    initialized = await initializeTerminalCore(container, sessionId, connection, 'detached');
    const { appSettings, term, fitAddon, searchAddon, webglAddon } = initialized;
    const started = await startBackendTerminalSession(sessionId, term, false);
    if (!started) {
      resetTerminalInitState(sessionId, term);
      return null;
    }

    markTerminalStarted(sessionId);
    void sendTerminalResize(sessionId, term.cols, term.rows);

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
    resetTerminalInitState(sessionId, initialized?.term);
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
  if (existing && !existing.disposed) return existing;
  const created: InputSendState = {
    buffer: '',
    timer: null,
    sending: false,
    disposed: false,
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
  if (state.disposed) return;
  if (inputSendStates.get(sessionId) !== state) return;
  if (state.sending) return;
  if (!state.buffer) return;

  state.sending = true;
  const now = nowMs();

  try {
    if (state.disposed || inputSendStates.get(sessionId) !== state) return;
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
  }

  if (state.disposed || inputSendStates.get(sessionId) !== state) return;
  if (state.buffer.length > 0 && state.timer === null) {
    state.timer = scheduleNext(() => {
      // This state may have been cleaned up while the callback was pending.
      if (inputSendStates.get(sessionId) !== state || state.disposed) return;
      state.timer = null;
      void flushTerminalInput(sessionId, state);
    });
  }
}

/**
 * xterm 6.0: Enhanced input buffering with adaptive thresholds
 */
function sendTerminalDataBuffered(sessionId: string, data: string, immediate: boolean) {
  const state = getInputSendState(sessionId);
  if (state.disposed || inputSendStates.get(sessionId) !== state) return;
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
    if (inputSendStates.get(sessionId) !== state || state.disposed) return;
    state.timer = null;
    void flushTerminalInput(sessionId, state);
  });
}

function handleTerminalInputSingle(sessionId: string, data: string) {
  void handleTerminalInputSingleAsync(sessionId, data);
}

function extractCommandsForAudit(sessionId: string, data: string): string[] {
  let buffer = commandAuditBuffers.get(sessionId) ?? '';
  const commands: string[] = [];

  for (const ch of data) {
    if (ch === '\r' || ch === '\n') {
      const command = buffer.trim();
      if (command) {
        commands.push(command);
      }
      buffer = '';
      continue;
    }

    if (ch === '\u007f' || ch === '\b') {
      buffer = buffer.slice(0, -1);
      continue;
    }

    if (ch === '\u0015') {
      buffer = '';
      continue;
    }

    if (ch === '\u0003' || ch === '\u001b') {
      buffer = '';
      continue;
    }

    if (ch >= ' ' || ch === '\t') {
      buffer += ch;
    }
  }

  commandAuditBuffers.set(sessionId, buffer);
  return commands;
}

function summarizeCommandForAudit(command: string, riskLevel: string): string {
  let summary = command.replace(/\s+/g, ' ').trim();
  if (!summary) return '<empty>';

  summary = summary.replace(
    /\b([A-Za-z_][A-Za-z0-9_]*(?:pass(word)?|token|secret|api[_-]?key|access[_-]?key|private[_-]?key|pwd))\s*=\s*(?:'[^']*'|"[^"]*"|[^\s;]+)/gi,
    '$1=<redacted>',
  );
  summary = summary.replace(
    /(--?(?:password|passphrase|passwd|token|secret|api[_-]?key|access[_-]?key|private[_-]?key)\b(?:=|\s+))(?:'[^']*'|"[^"]*"|[^\s;]+)/gi,
    '$1<redacted>',
  );
  summary = summary.replace(
    /\b(-p)\s+(?:'[^']*'|"[^"]*"|[^\s;]+)/g,
    '$1 <redacted>',
  );
  summary = summary.replace(
    /(https?:\/\/[^/\s:@]+:)[^@\s]+@/gi,
    '$1<redacted>@',
  );
  summary = summary.replace(
    /\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/g,
    '<redacted-jwt>',
  );

  if (riskLevel === 'HIGH' || riskLevel === 'CRITICAL') {
    const head = summary.split(/\s+/).slice(0, 4).join(' ');
    summary = `[${riskLevel}] ${head || '<command>'} [summary]`;
  }

  const maxLen = 200;
  if (summary.length > maxLen) {
    summary = `${summary.slice(0, maxLen)}...`;
  }

  return summary;
}

async function auditTerminalCommands(sessionId: string, data: string): Promise<boolean> {
  const commands = extractCommandsForAudit(sessionId, data);
  for (const command of commands) {
    const analysis = auditService.analyzeCommand(command);
    const auditCommand = summarizeCommandForAudit(command, analysis.riskLevel);
    if (auditService.requiresConfirmation(analysis.riskLevel)) {
      const confirmed = window.confirm(
        [
          `检测到高风险命令: ${auditCommand}`,
          `风险等级: ${analysis.riskLevel}`,
          `原因: ${analysis.description}`,
          '',
          '提示: 上方命令已脱敏显示',
          '确认继续执行吗？'
        ].join('\n')
      );

      await auditService.recordEvent(
        auditService.createEvent({
          command: auditCommand,
          sessionId,
          action: confirmed ? 'WARNED' : 'BLOCKED',
          details: {
            source: 'terminal-input',
            riskLevel: analysis.riskLevel,
            detectedPatterns: analysis.detectedPatterns,
            commandWasSanitized: true,
          },
        })
      );

      if (!confirmed) {
        showErrorMessage(`已阻止高风险命令: ${auditCommand}`, 5000);
        return false;
      }
      continue;
    }

    await auditService.recordEvent(
      auditService.createEvent({
        command: auditCommand,
        sessionId,
        action: 'ALLOWED',
        details: {
          source: 'terminal-input',
          riskLevel: analysis.riskLevel,
          detectedPatterns: analysis.detectedPatterns,
          commandWasSanitized: true,
        },
      })
    );
  }

  return true;
}

async function handleTerminalInputSingleAsync(sessionId: string, data: string) {
  if (!(await auditTerminalCommands(sessionId, data))) {
    return;
  }

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
            showErrorMessage(`会话已断开: ${sessionId}`, 5000);
            // Optionally attempt to reconnect
            break;
          case 'error':
            if (event.payload.error) {
              showErrorMessage(`会话错误: ${event.payload.error}`, 5000);
            }
            break;
          case 'connected':
            showSuccessMessage('会话已连接', 3000);
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
    suppressReconnect(sessionId);
    terminalPool.destroyInstance(sessionId);
    await closeDetachedTerminal(sessionId);
    await invoke('disconnect', { sessionId });
    cleanupTerminalListeners(sessionId);
    
    log.info('TermCleanup', 'Split session closed', { sessionId });
  } catch (e) {
    log.error('TermCleanup', 'Failed to close split session', e);
  }
}

export async function closeTerminal(sessionId: string) {
  suppressReconnect(sessionId);
  const terminals = get(activeTerminals);
  const index = terminals.findIndex(t => t.sessionId === sessionId);

  if (index !== -1) {
    cleanupBufferedTerminalState(sessionId);
    terminalPool.destroyInstance(sessionId);

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

    cleanupTerminalListeners(sessionId);

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
  cleanupBufferedTerminalState(sessionId);
  cleanupTerminalListeners(sessionId);

  try {
    await invoke('close_terminal', { sessionId });
  } catch (error) {
    log.warn('TermCleanup', 'Failed to close detached terminal', error, { sessionId });
  }
}

export async function disconnectTerminal(sessionId: string) {
  // If this is a root session with split children, close them first to avoid orphan sessions.
  const splitSessionIds = new Set<string>();
  splitSessionIds.add(sessionId);
  {
    const sessionMap = get(terminalSessionMap);
    const children = sessionMap.get(sessionId);
    if (children) {
      children.forEach(id => splitSessionIds.add(id));
    }
    for (const t of get(activeTerminals)) {
      if ((t as any)?.parentId === sessionId) {
        splitSessionIds.add(t.sessionId);
      }
    }
  }

  if (splitSessionIds.size > 1) {
    for (const childId of splitSessionIds) {
      if (childId === sessionId) continue;
      suppressReconnect(childId);
      try {
        await closeSplitSession(childId);
      } catch (error) {
        log.warn('Disconnect', 'Failed to close split child session', error, { sessionId, childId });
      }
    }

    // Ensure UI does not keep stale child sessions/broadcast targets.
    activeTerminals.update(items => items.filter(t => !splitSessionIds.has(t.sessionId) || t.sessionId === sessionId));
    broadcastSessionIds.update(ids => ids.filter(id => !splitSessionIds.has(id) || id === sessionId));
  }

  const terminals = get(activeTerminals);
  const terminal = terminals.find(t => t.sessionId === sessionId);
  const name = terminal?.connection?.name ?? sessionId;
  const hadTerminal = Boolean(terminal);

  suppressReconnect(sessionId);

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
    showSuccessMessage(`已断开连接: ${name}`, 3000);
  } catch (error) {
    showErrorMessage(`断开连接失败: ${name}`, 5000);
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
      return;
    } catch (error) {
      log.warn('TermCleanup', 'Failed to close terminal in best-effort mode', error, { sessionId, attempt: i });
      await new Promise(resolve => setTimeout(resolve, 50));
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

  const connectedById = new Map<string, BackendSessionInfo>();
  for (const s of connectedSessions) {
    const sid = String((s as any)?.id ?? '');
    if (sid) connectedById.set(sid, s);
  }

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

  restoredSessionsWithBackendTerminal.clear();
  for (const entry of entries) {
    const backend = connectedById.get(entry.sessionId);
    const terminalId = (backend as any)?.terminal_id;
    if (terminalId) {
      restoredSessionsWithBackendTerminal.add(entry.sessionId);
    }
  }

  entries.sort((a, b) => {
    const ai = orderIndex.get(a.sessionId);
    const bi = orderIndex.get(b.sessionId);
    if (ai === undefined && bi === undefined) return a.connection.name.localeCompare(b.connection.name, 'zh-Hans-CN');
    if (ai === undefined) return 1;
    if (bi === undefined) return -1;
    return ai - bi;
  });

  if (entries.length === 0) return;

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
        // xterm 5.x write is callback-based; use explicit async wrapper.
        await writeTerminalAsync(term, payload);
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
        showErrorMessage(`终端错误: ${event.payload.error}`, 5000);
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

                clearReconnectKeyListener(sessionId);
                const disposable = term.onData(async (data: string) => {
                    if (data === 'r' || data === 'R') {
                        disposable.dispose();
                        reconnectKeyListeners.delete(sessionId);
                        await reconnectTerminal(sessionId);
                    }
                });
                reconnectKeyListeners.set(sessionId, disposable);
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

function clearReconnectState(sessionId: string) {
  clearReconnectTimer(sessionId);
  clearReconnectKeyListener(sessionId);
  reconnectAttempts.delete(sessionId);
}

function suppressReconnect(sessionId: string) {
  reconnectSuppressed.add(sessionId);
  clearReconnectState(sessionId);
}

function scheduleAutoReconnect(sessionId: string, term: Terminal, immediate: boolean) {
  if (reconnectSuppressed.has(sessionId)) {
    return;
  }

  clearReconnectTimer(sessionId);
  clearReconnectKeyListener(sessionId);

  const attempts = reconnectAttempts.get(sessionId) ?? 0;
  if (attempts >= MAX_AUTO_RECONNECT_RETRIES) {
    void term.write(`\r\n\x1b[31mAuto reconnect stopped after ${MAX_AUTO_RECONNECT_RETRIES} attempts.\x1b[0m\r\n`);
    showErrorMessage('自动重连已停止：超过最大重试次数', 5000);
    return;
  }

  const delay = immediate
    ? 0
    : Math.min(MAX_AUTO_RECONNECT_DELAY_MS, Math.floor(BASE_AUTO_RECONNECT_DELAY_MS * Math.pow(2, attempts)));
  const seconds = Math.max(0, Math.ceil(delay / 1000));
  void term.write(`\r\n\x1b[33mAuto reconnect attempt ${attempts + 1}/${MAX_AUTO_RECONNECT_RETRIES} in ${seconds}s... (Press R to immediate)\x1b[0m\r\n`);

  const disposable = term.onData((data: string) => {
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
    if (reconnectSuppressed.has(sessionId)) {
      return;
    }

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
    if (appSettings.connection?.autoReconnect && !reconnectSuppressed.has(sessionId)) {
      scheduleAutoReconnect(sessionId, term, false);
    }
  }, delay);

  reconnectTimers.set(sessionId, timer);
}

async function reconnectTerminalInternal(oldSessionId: string): Promise<boolean> {
  if (reconnectSuppressed.has(oldSessionId)) {
    return false;
  }

  const terminals = get(activeTerminals);
  const terminalEntry = terminals.find(t => t.sessionId === oldSessionId);
  if (!terminalEntry) return false;
  const term = terminalEntry.terminal;
  if (!term || typeof (term as Partial<Terminal>).write !== 'function') {
    log.warn('Reconnect', 'Reconnect skipped: terminal instance is unavailable', { oldSessionId });
    return false;
  }
  let newSessionId: string | null = null;
  let hadOldInputListener = false;
  let oldInputDetached = false;

  void term.write('\r\n\x1b[33mReconnecting...\x1b[0m\r\n');

  try {
      const reconnectConfig = reconnectConnectConfigs.get(oldSessionId) ?? terminalEntry.connection;
      const { sessionId: createdSessionId, connectConfig: effectiveReconnectConfig } =
        await connectWithInteractivePrompts(reconnectConfig, terminalEntry.connection.name);
      newSessionId = createdSessionId;
      const nextSessionId = createdSessionId;

      hadOldInputListener = inputListeners.has(oldSessionId);
      const oldInputListener = inputListeners.get(oldSessionId);
      if (oldInputListener) {
        oldInputListener.dispose();
        inputListeners.delete(oldSessionId);
        oldInputDetached = true;
      }

      cleanupTerminalListeners(nextSessionId);
      cleanupBufferedTerminalState(nextSessionId);
      await setupTerminalListeners(nextSessionId, term);
      const newInputListener = term.onData((data: string) => {
        handleTerminalInput(nextSessionId, data, terminalEntry.connection);
      });
      inputListeners.set(nextSessionId, newInputListener);
      
      // Start terminal
      const result = await invoke('start_terminal', {
          sessionId: nextSessionId,
          width: term.cols,
          height: term.rows,
      });

      if (!result) throw new Error('Failed to start terminal');

      rememberReconnectConfig(nextSessionId, effectiveReconnectConfig);

      if (reconnectSuppressed.has(oldSessionId)) {
        cleanupTerminalListeners(nextSessionId);
        cleanupBufferedTerminalState(nextSessionId);
        if (hadOldInputListener) {
          const restored = term.onData((data: string) => {
            handleTerminalInput(oldSessionId, data, terminalEntry.connection);
          });
          inputListeners.set(oldSessionId, restored);
          oldInputDetached = false;
        }
        markTerminalStopped(nextSessionId);
        try {
          await invoke('close_terminal', { sessionId: nextSessionId });
        } catch (error) {
          log.warn('Reconnect', 'Failed to close new terminal for suppressed reconnect', error, { oldSessionId, newSessionId: nextSessionId });
        }
        try {
          await invoke('disconnect', { sessionId: nextSessionId });
        } catch (error) {
          log.warn('Reconnect', 'Failed to disconnect new session for suppressed reconnect', error, { oldSessionId, newSessionId: nextSessionId });
        }
        return false;
      }

      markTerminalStopped(oldSessionId);
      markTerminalStarted(nextSessionId);

      if (!terminalPool.migrateSession(oldSessionId, nextSessionId)) {
        log.warn('Reconnect', 'Terminal pool migration skipped', { oldSessionId, newSessionId: nextSessionId });
      }

      let replaced = false;
      activeTerminals.update(items => {
        let foundRoot = false;
        const next = items.map(item => {
          if (item.sessionId === oldSessionId) {
            foundRoot = true;
            return { ...item, sessionId: nextSessionId, terminal: term };
          }
          if (item.parentId === oldSessionId) {
            return { ...item, parentId: nextSessionId };
          }
          return item;
        });
        replaced = foundRoot;
        return next;
      });

      if (!replaced) {
        cleanupTerminalListeners(nextSessionId);
        cleanupBufferedTerminalState(nextSessionId);
        if (hadOldInputListener) {
          const restored = term.onData((data: string) => {
            handleTerminalInput(oldSessionId, data, terminalEntry.connection);
          });
          inputListeners.set(oldSessionId, restored);
          oldInputDetached = false;
        }
        markTerminalStopped(nextSessionId);
        try {
          await invoke('close_terminal', { sessionId: nextSessionId });
        } catch (error) {
          log.warn('Reconnect', 'Failed to close new terminal after stale reconnect', error, { oldSessionId, newSessionId: nextSessionId });
        }
        try {
          await invoke('disconnect', { sessionId: nextSessionId });
        } catch (error) {
          log.warn('Reconnect', 'Failed to disconnect new session after stale reconnect', error, { oldSessionId, newSessionId: nextSessionId });
        }
        return false;
      }

      broadcastSessionIds.update(ids => ids.map(id => (id === oldSessionId ? nextSessionId : id)));
      terminalSessionMap.update(map => {
        if (map.has(oldSessionId)) {
          const children = map.get(oldSessionId);
          map.delete(oldSessionId);
          if (children) {
            children.delete(oldSessionId);
            children.add(nextSessionId);
            map.set(nextSessionId, children);
          } else {
            map.set(nextSessionId, new Set([nextSessionId]));
          }
          return map;
        }
        for (const children of map.values()) {
          if (children.has(oldSessionId)) {
            children.delete(oldSessionId);
            children.add(nextSessionId);
          }
        }
        return map;
      });
      reconnectAttempts.delete(oldSessionId);
      clearReconnectTimer(oldSessionId);
      clearReconnectKeyListener(oldSessionId);
      cleanupBufferedTerminalState(oldSessionId);
      cleanupTerminalListeners(oldSessionId);

      try {
        await invoke('close_terminal', { sessionId: oldSessionId });
      } catch (error) {
        log.warn('Reconnect', 'Failed to close old terminal during reconnect', error, { oldSessionId });
      }

      try {
        await invoke('disconnect', { sessionId: oldSessionId });
      } catch (error) {
        log.warn('Reconnect', 'Failed to disconnect old session during reconnect', error, { oldSessionId });
      }

      term.write('\r\n\x1b[32mReconnected!\x1b[0m\r\n');
      showSuccessMessage(`已重连: ${terminalEntry.connection.name}`, 3000);
      term.focus();
      
      // Trigger resize
      await sendTerminalResize(nextSessionId, term.cols, term.rows);

      return true;
  } catch (e) {
      if (reconnectSuppressed.has(oldSessionId)) {
        return false;
      }
      if (newSessionId) {
        cleanupTerminalListeners(newSessionId);
        cleanupBufferedTerminalState(newSessionId);
        markTerminalStopped(newSessionId);
        try {
          await invoke('close_terminal', { sessionId: newSessionId });
        } catch (error) {
          log.warn('Reconnect', 'Failed to close new terminal after reconnect failure', error, { oldSessionId, newSessionId });
        }
        try {
          await invoke('disconnect', { sessionId: newSessionId });
        } catch (error) {
          log.warn('Reconnect', 'Failed to disconnect new session after reconnect failure', error, { oldSessionId, newSessionId });
        }
      }
      if (oldInputDetached && hadOldInputListener && !inputListeners.has(oldSessionId)) {
        const restored = term.onData((data: string) => {
          handleTerminalInput(oldSessionId, data, terminalEntry.connection);
        });
        inputListeners.set(oldSessionId, restored);
      }
      void term.write(`\r\n\x1b[31mReconnection failed: ${e}\x1b[0m\r\n`);
      showErrorMessage(`重连失败: ${terminalEntry.connection.name}`, 5000);
      return false;
  }
}

export async function reconnectTerminal(oldSessionId: string) {
  const existing = reconnectInFlight.get(oldSessionId);
  if (existing) {
    return existing;
  }

  const reconnectPromise = reconnectTerminalInternal(oldSessionId).finally(() => {
    if (reconnectInFlight.get(oldSessionId) === reconnectPromise) {
      reconnectInFlight.delete(oldSessionId);
    }
  });

  reconnectInFlight.set(oldSessionId, reconnectPromise);
  return reconnectPromise;
}
