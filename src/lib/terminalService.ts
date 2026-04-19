import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';
import { SearchAddon } from '@xterm/addon-search';
import { WebglAddon } from '@xterm/addon-webgl';
import { WebLinksAddon } from '@xterm/addon-web-links';
import { get } from 'svelte/store';
import {
  activeTerminals,
  activeTerminalSessionIds,
  connections,
  selectedTerminalIndex,
  type Connection,
  type ActiveTerminal,
  type AppSettings,
  type HistoryConnectionSnapshot,
  settings,
  connectionHistory,
  broadcastInputEnabled,
  broadcastSessionIds,
  getStoredTerminalUiState,
  getXtermTheme,
  getBaseXtermTheme,
  terminalSessionMap,
  terminalSessionStates,
  setTerminalSessionState,
  removeTerminalSessionState,
  connectingConnections,
  clearErrorMessage,
  showErrorMessage,
  showSuccessMessage,
  requestPasswordPrompt,
} from './store';
import { terminalPool } from './terminalPool';
import { TerminalInstance } from './terminalInstance';
import { computeNextSelectedIndexAfterRemoval } from './terminalStateUtils';
import {
  attachTerminalInputListener as attachTerminalInputListenerRegistry,
  armTerminalKeyListener,
  clearOutputListenerCleanup,
  clearReconnectKeyListener,
  deleteOutputListenerCleanup,
  detachTerminalInputListener,
  getOutputListenerCleanup,
  setOutputListenerCleanup,
} from './terminalListenerRegistry';
import {
  BASE_AUTO_RECONNECT_DELAY_MS,
  clearReconnectAttempts,
  clearReconnectConfig,
  clearReconnectInFlight,
  clearReconnectTimer as clearReconnectTimerState,
  getReconnectAttempts,
  getReconnectConfig,
  getReconnectInFlight,
  isReconnectSuppressed,
  MAX_AUTO_RECONNECT_RETRIES,
  MAX_AUTO_RECONNECT_DELAY_MS,
  releaseReconnectSuppressionState,
  rememberReconnectConfig as rememberReconnectConfigState,
  setReconnectAttempts,
  setReconnectInFlight,
  setReconnectTimer,
  suppressReconnectState,
} from './terminalReconnectState';
import {
  cleanupBufferedIoState,
  disposeOutputBuffer,
  enqueueTerminalOutput,
  initializeOutputBuffer,
  sendTerminalDataBuffered as queueTerminalDataBuffered,
  type IoLogger,
} from './terminalIoBuffer';
import {
  canAttemptSessionReconnect,
  canContinueSessionReconnectFlow,
  interpretBackendSession,
  type BackendSessionInfo,
  type TerminalSessionReconnectContext,
} from './terminalSessionModel';
import {
  buildHostKeyConfirmMessage,
  parseHostKeyPrompt,
  saveHostKeyPrompt,
} from './hostKeyPrompt';
import { sanitizeTerminalDisplayText } from './terminalDisplaySanitizer';
import { extractTerminalWorkingDirectory } from './terminalCwd';

const IS_DEV = import.meta.env.DEV;

// Dev-only logger for terminal orchestration.
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

const ioLogger: IoLogger = log;

function reportTerminalError(error: unknown, context: string, userMessage?: string) {
  const errorMsg = normalizeErrorMessage(error);

  if (IS_DEV) {
    log.error('TermError', `${context}: ${errorMsg}`, error);
  }

  if (userMessage) {
    showErrorMessage(sanitizeTerminalDisplayText(userMessage), 5000);
  }
}

// Sessions that currently have a live backend terminal.
const startedTerminalSessions = new Set<string>();
const lastTerminalSizes = new Map<string, { width: number; height: number }>();
const terminalCwdRemainders = new Map<string, string>();

export function markTerminalStarted(sessionId: string) {
  startedTerminalSessions.add(sessionId);
}

export function markTerminalStopped(sessionId: string) {
  startedTerminalSessions.delete(sessionId);
}

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

// Sessions discovered during restore that already have a backend terminal.
const restoredSessionsWithBackendTerminal = new Set<string>();

function cleanupBufferedTerminalState(sessionId: string) {
  markTerminalStopped(sessionId);
  cleanupBufferedIoState(sessionId);
  clearReconnectConfig(sessionId);
  lastTerminalSizes.delete(sessionId);
}

function updateSessionLifecycleState(
  sessionId: string,
  connectionPhase: 'connected' | 'reconnecting' | 'closing' | 'closed',
  terminalPhase: 'detached' | 'starting' | 'attached' | 'closed',
  reason: string | null = null,
  restored = false
) {
  setTerminalSessionState(sessionId, {
    connectionPhase,
    terminalPhase,
    reason,
    restored,
  });
}

function getSessionReconnectContext(sessionId: string): TerminalSessionReconnectContext {
  return {
    inUi: get(activeTerminalSessionIds).has(sessionId),
    reconnectSuppressed: isReconnectSuppressed(sessionId),
    reconnectInFlight: Boolean(getReconnectInFlight(sessionId)),
  };
}

function canAttemptReconnectForSession(sessionId: string): boolean {
  const context = getSessionReconnectContext(sessionId);
  const state = get(terminalSessionStates).get(sessionId);
  return canAttemptSessionReconnect(state, context);
}

function canContinueReconnectForSession(sessionId: string): boolean {
  const context = getSessionReconnectContext(sessionId);
  const state = get(terminalSessionStates).get(sessionId);
  return canContinueSessionReconnectFlow(state, {
    inUi: context.inUi,
    reconnectSuppressed: context.reconnectSuppressed,
  });
}

function attachTerminalInputListener(sessionId: string, term: Terminal) {
  attachTerminalInputListenerRegistry(sessionId, term, (data: string) => {
    handleTerminalInput(sessionId, data);
  });
}

function cleanupTerminalListeners(sessionId: string) {
  clearOutputListenerCleanup(sessionId);
  detachTerminalInputListener(sessionId);
}

function removeSessionReferences(sessionId: string) {
  terminalCwdRemainders.delete(sessionId);
  broadcastSessionIds.update(ids => ids.filter(id => id !== sessionId));
  terminalSessionMap.update(map => {
    map.delete(sessionId);
    for (const children of map.values()) {
      children.delete(sessionId);
    }
    return map;
  });
}

function removeTerminalFromUi(sessionId: string): boolean {
  const terminals = get(activeTerminals);
  const removedIndex = terminals.findIndex(t => t.sessionId === sessionId);
  if (removedIndex < 0) {
    removeSessionReferences(sessionId);
    return false;
  }

  const currentIndex = get(selectedTerminalIndex);
  activeTerminals.update(items => items.filter(t => t.sessionId !== sessionId));
  removeSessionReferences(sessionId);
  selectedTerminalIndex.set(
    computeNextSelectedIndexAfterRemoval(currentIndex, removedIndex, terminals.length)
  );
  return true;
}

function shouldTreatMissingSessionAsClosed(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error ?? '');
  return message.includes('Session not found');
}

function finalizeTerminalClosure(sessionId: string) {
  cleanupBufferedTerminalState(sessionId);
  terminalPool.destroyInstance(sessionId);
  cleanupTerminalListeners(sessionId);
  removeTerminalFromUi(sessionId);
  releaseReconnectSuppression(sessionId);
  removeTerminalSessionState(sessionId);
}

function cacheReconnectConfig(sessionId: string, connectConfig: any) {
  rememberReconnectConfigState(sessionId, connectConfig);
}

export async function invokeWithTimeout<T>(cmd: string, args: any, timeoutMs: number = 30000): Promise<T> {
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

function toHistoryConnectionSnapshot(connection: Connection): HistoryConnectionSnapshot {
  return {
    id: connection.id,
    name: connection.name,
    protocol: connection.protocol,
    host: connection.host,
    port: connection.port,
    username: connection.username,
    description: connection.description,
    tags: Array.isArray(connection.tags) ? [...connection.tags] : [],
    group_id: connection.group_id,
  };
}

function addConnectionHistoryEntry(connection: Connection): void {
  const snapshot = toHistoryConnectionSnapshot(connection);
  connectionHistory.update(history => {
    const newHistory = history.filter(h => h.connection.id !== snapshot.id);
    newHistory.unshift({
      connection: snapshot,
      lastConnected: Date.now()
    });
    return newHistory.slice(0, 50);
  });
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

      addConnectionHistoryEntry(connection);

      showSuccessMessage(`已启动 RDP: ${connection.name}`, 3000);
      connectingConnections.update(s => {
        s.delete(connection.id);
        return s;
      });
      return;
    }

    if (protocol === 'Telnet') {
      const sessionId = await invokeWithTimeout<string>('connect', { config: baseConfig });
      cacheReconnectConfig(sessionId, baseConfig);
      updateSessionLifecycleState(sessionId, 'connected', 'detached');

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

      addConnectionHistoryEntry(connection);

      showSuccessMessage(`连接成功: ${connection.name}`, 3000);
      connectingConnections.update(s => {
        s.delete(connection.id);
        return s;
      });
      return;
    }

    const { sessionId, connectConfig: effectiveConnectConfig } = await connectWithInteractivePrompts(baseConfig, connection.name);
    cacheReconnectConfig(sessionId, effectiveConnectConfig);
    updateSessionLifecycleState(sessionId, 'connected', 'detached');
    
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

    addConnectionHistoryEntry(connection);

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
    const msg = toSafeUserMessage(error);
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

function toSafeUserMessage(error: unknown): string {
  return sanitizeTerminalDisplayText(normalizeErrorMessage(error));
}

function writeTerminalStatus(term: Terminal, colorCode: '31' | '32' | '33' | '36', label: string, value?: unknown) {
  const suffix = value == null ? '' : `: ${sanitizeTerminalDisplayText(value)}`;
  void term.write(`\r\n\x1b[${colorCode}m${label}${suffix}\x1b[0m\r\n`);
}

function showSafeErrorMessage(prefix: string, value: unknown, timeoutMs = 5000) {
  showErrorMessage(`${prefix}: ${sanitizeTerminalDisplayText(value)}`, timeoutMs);
}

type ConfirmDialogDetail = {
  title: string;
  message: string;
  confirmText?: string;
  cancelText?: string;
  kind?: 'info' | 'warning' | 'danger';
  resolve: (confirmed: boolean) => void;
  reject: (error: Error) => void;
};

function dispatchConfirmDialogRequest(detail: Omit<ConfirmDialogDetail, 'resolve' | 'reject'>): Promise<boolean> {
  return new Promise((resolve, reject) => {
    const event = new CustomEvent<ConfirmDialogDetail>('starshuttle:confirm-dialog-request', {
      cancelable: true,
      detail: {
        ...detail,
        resolve,
        reject,
      },
    });
    window.dispatchEvent(event);
    if (!event.defaultPrevented) {
      reject(new Error('确认对话框不可用'));
    }
  });
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
  const entered = await requestPasswordPrompt(`请输入连接「${connectionName}」的密码`);
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
  cacheReconnectConfig(sessionId, connectConfig);
  updateSessionLifecycleState(sessionId, 'connected', 'detached');
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

      const confirmed = await dispatchConfirmDialogRequest({
        title: '主机密钥确认',
        message: buildHostKeyConfirmMessage(parsed),
        confirmText: '信任并继续',
        cancelText: '取消',
        kind: parsed.type === 'mismatch' ? 'danger' : 'warning',
      });
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
      // Best-effort cleanup only.
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
      if (!uri) return;
      try {
        const parsed = new URL(uri);
        if (parsed.protocol === 'http:' || parsed.protocol === 'https:') {
          window.open(uri, '_blank', 'noopener,noreferrer');
        }
      } catch {
        // Invalid URL — ignore
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

  attachTerminalInputListener(sessionId, term);
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
    updateSessionLifecycleState(sessionId, 'connected', 'attached', null, true);
    return true;
  }

  updateSessionLifecycleState(sessionId, 'connected', 'starting');

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
      updateSessionLifecycleState(sessionId, 'connected', 'attached');
      return true;
    }

    if (logFailure) {
      log.error('TermInit', 'Failed to start terminal session', error);
    }
    updateSessionLifecycleState(sessionId, 'connected', 'closed', 'start_failed');
    void term.write('\r\n\x1b[31mFailed to start terminal session\x1b[0m\r\n');
    showErrorMessage('启动终端会话失败', 5000);
    return false;
  }

  if (result) {
    updateSessionLifecycleState(sessionId, 'connected', 'attached');
    return true;
  }
  if (logFailure) {
    log.error('TermInit', 'Failed to start terminal session', null);
  }
  updateSessionLifecycleState(sessionId, 'connected', 'closed', 'start_failed');
  void term.write('\r\n\x1b[31mFailed to start terminal session\x1b[0m\r\n');
  showErrorMessage('启动终端会话失败', 5000);
  return false;
}

export async function initTerminal(container: HTMLElement, sessionId: string, connection: Connection): Promise<ActiveTerminal | null> {
  let initialized: TerminalInitCommon | null = null;
  try {
    initialized = await initializeTerminalCore(container, sessionId, 'attached');
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
    reportTerminalError(error, 'initTerminal', '初始化终端失败');
    return null;
  }
}

export async function initDetachedTerminal(container: HTMLElement, sessionId: string, connection: Connection): Promise<ActiveTerminal | null> {
  const recoverFromInitFailure = async (term?: Terminal) => {
    resetTerminalInitState(sessionId, term);
    suppressReconnect(sessionId);
    cleanupBufferedTerminalState(sessionId);
    cleanupTerminalListeners(sessionId);
    terminalPool.destroyInstance(sessionId);
    removeTerminalFromUi(sessionId);
    try {
      await invoke('disconnect', { sessionId });
    } catch (disconnectError) {
      log.warn('TermInit', 'Best-effort disconnect failed after detached init failure', disconnectError, {
        sessionId,
      });
    }
    removeTerminalSessionState(sessionId);
  };

  let initialized: TerminalInitCommon | null = null;
  try {
    initialized = await initializeTerminalCore(container, sessionId, 'detached');
    const { appSettings, term, fitAddon, searchAddon, webglAddon } = initialized;
    const started = await startBackendTerminalSession(sessionId, term, false);
    if (!started) {
      await recoverFromInitFailure(term);
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
    await recoverFromInitFailure(initialized?.term);
    reportTerminalError(error, 'initDetachedTerminal', '初始化终端失败');
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

function sendTerminalDataBuffered(sessionId: string, data: string, immediate: boolean) {
  queueTerminalDataBuffered(sessionId, data, immediate, {
    invokeSend: async (targetSessionId: string, payload: string) => {
      await invoke('send_terminal_data', { sessionId: targetSessionId, data: payload });
    },
    logger: ioLogger,
  });
}

export function handleTerminalInputSingle(sessionId: string, data: string) {
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

export function handleTerminalInput(sessionId: string, data: string, connection?: Connection) {
  void connection;
  const enabled = get(broadcastInputEnabled);
  if (!enabled) {
    handleTerminalInputSingle(sessionId, data);
    return;
  }

  const terminals = get(activeTerminals);
  const activeSessionIds = new Set(terminals.map(t => t.sessionId));
  const selected = get(broadcastSessionIds);
  const liveSelected = selected.filter(id => activeSessionIds.has(id));
  if (liveSelected.length !== selected.length) {
    broadcastSessionIds.set(liveSelected);
  }

  const baseTargets = liveSelected.length > 0 ? liveSelected : [sessionId];
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
  
  const targets = Array.from(expandedTargets).filter(id => activeSessionIds.has(id));
  if (activeSessionIds.has(sessionId) && !targets.includes(sessionId)) {
    targets.push(sessionId);
  }

  const connectionBySessionId = new Map(terminals.map(t => [t.sessionId, t.connection] as const));

  for (const targetSessionId of targets) {
    const targetConnection = connectionBySessionId.get(targetSessionId);
    if (!targetConnection) continue;
    handleTerminalInputSingle(targetSessionId, data);
  }
}

export async function sendTerminalResize(sessionId: string, width: number, height: number) {
  if (width <= 0 || height <= 0) return;

  const lastSize = lastTerminalSizes.get(sessionId);
  if (lastSize && lastSize.width === width && lastSize.height === height) {
    return;
  }

  try {
    await invoke('resize_terminal', { sessionId, width, height });
    startedTerminalSessions.add(sessionId);
    lastTerminalSizes.set(sessionId, { width, height });
  } catch (error) {
    const msg = error instanceof Error ? error.message : String(error);
    if (msg.includes('Session not found')) {
      log.warn('TermResize', 'Session not found', error, { sessionId });
      return;
    }
    log.error('TermResize', 'Failed to resize terminal', error, { sessionId, width, height });
  }
}

export async function closeSplitSession(sessionId: string) {
  suppressReconnect(sessionId);

  try {
    await closeDetachedTerminal(sessionId);
    await invoke('disconnect', { sessionId });
  } catch (e) {
    if (!shouldTreatMissingSessionAsClosed(e)) {
      log.error('TermCleanup', 'Failed to close split session', e, { sessionId });
      throw e;
    }
    log.warn('TermCleanup', 'Split session already closed in backend; cleaning UI state', e, {
      sessionId,
    });
  }

  finalizeTerminalClosure(sessionId);
  log.info('TermCleanup', 'Split session closed', { sessionId });
}

export async function closeTerminal(sessionId: string) {
  suppressReconnect(sessionId);
  if (!get(activeTerminalSessionIds).has(sessionId)) return;

  try {
    await invoke('close_terminal', { sessionId });
  } catch (e) {
    if (!shouldTreatMissingSessionAsClosed(e)) {
      log.error('TermCleanup', 'Failed to close terminal', e, { sessionId });
      showErrorMessage('关闭终端失败', 5000);
      throw e;
    }
    log.warn('TermCleanup', 'Terminal already closed in backend; cleaning UI state', e, {
      sessionId,
    });
  }

  finalizeTerminalClosure(sessionId);
  log.info('TermCleanup', 'Terminal closed', { sessionId });
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

  const closedChildren = new Set<string>();
  if (splitSessionIds.size > 1) {
    for (const childId of splitSessionIds) {
      if (childId === sessionId) continue;
      suppressReconnect(childId);
      try {
        await closeSplitSession(childId);
        closedChildren.add(childId);
      } catch (error) {
        log.warn('Disconnect', 'Failed to close split child session', error, { sessionId, childId });
      }
    }

    // Ensure UI does not keep stale child sessions/broadcast targets.
    if (closedChildren.size > 0) {
      activeTerminals.update(items => items.filter(t => !closedChildren.has(t.sessionId)));
      for (const childId of closedChildren) {
        removeSessionReferences(childId);
      }
    }
  }

  const terminals = get(activeTerminals);
  const terminal = terminals.find(t => t.sessionId === sessionId);
  const name = terminal?.connection?.name ?? sessionId;

  suppressReconnect(sessionId);

  try {
    await invoke('disconnect', { sessionId });
  } catch (error) {
    if (!shouldTreatMissingSessionAsClosed(error)) {
      showErrorMessage(`断开连接失败: ${name}`, 5000);
      log.warn('Disconnect', 'Failed to disconnect', error, { sessionId, name });
      return;
    }
    log.warn('Disconnect', 'Session already missing in backend; cleaning UI state', error, {
      sessionId,
      name,
    });
  }

  finalizeTerminalClosure(sessionId);
  showSuccessMessage(`已断开连接: ${name}`, 3000);
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

  const interpretedSessions = sessions
    .map((session) => interpretBackendSession(session))
    .filter((session): session is NonNullable<typeof session> => session !== null);

  const uiState = getStoredTerminalUiState();
  const orderIndex = new Map(uiState.order.map((id, i) => [id, i] as const));

  restoredSessionsWithBackendTerminal.clear();
  for (const session of interpretedSessions) {
    updateSessionLifecycleState(
      session.sessionId,
      session.connectionPhase,
      session.terminalPhase,
      null,
      session.canRestoreTerminal
    );
    if (session.canRestoreTerminal) {
      restoredSessionsWithBackendTerminal.add(session.sessionId);
    }
  }

  const entries = interpretedSessions
    .filter((session) => session.canRestoreTerminal)
    .map((session) => {
      const connection = connectionById.get(session.connectionId);
      if (!connection) return null;
      return {
        sessionId: session.sessionId,
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

  const previousCleanup = getOutputListenerCleanup(sessionId);
  if (previousCleanup) {
    previousCleanup();
    deleteOutputListenerCleanup(sessionId);
  }

  initializeOutputBuffer(sessionId);

  const unlisteners: Array<() => void> = [];
  let disposed = false;
  const teardown = () => {
    if (disposed) return;
    disposed = true;
    disposeOutputBuffer(sessionId);
    while (unlisteners.length > 0) {
      const fn = unlisteners.pop();
      if (!fn) continue;
      try {
        fn();
      } catch {
        // Best-effort cleanup only.
      }
    }
  };

  setOutputListenerCleanup(sessionId, teardown);

  try {
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
        const cwdState = extractTerminalWorkingDirectory(
          data,
          terminalCwdRemainders.get(sessionId) ?? ''
        );
        terminalCwdRemainders.set(sessionId, cwdState.remainder);
        if (cwdState.cwd) {
          activeTerminals.update(items => {
            let changed = false;
            const next = items.map(item => {
              if (item.sessionId !== sessionId || item.currentDirectory === cwdState.cwd) {
                return item;
              }
              changed = true;
              return { ...item, currentDirectory: cwdState.cwd };
            });
            return changed ? next : items;
          });
        }
        enqueueTerminalOutput(sessionId, term, data, { logger: ioLogger, isDev: IS_DEV });
      }
    });
    if (disposed) {
      outputUnlisten();
      return;
    }
    unlisteners.push(outputUnlisten);

    // Error listener
    const errorUnlisten = await listen(`terminal-error-${sessionId}`, (event: any) => {
      if (event.payload && event.payload.error) {
        log.error('TermError', 'Terminal error reported from backend', event.payload.error, {
          sessionId,
        });
        writeTerminalStatus(term, '31', 'Error', event.payload.error);
        showSafeErrorMessage('终端错误', event.payload.error, 5000);
      }
    });
    if (disposed) {
      errorUnlisten();
      return;
    }
    unlisteners.push(errorUnlisten);

    // Session Closed listener
    const closedUnlisten = await listen(`session-closed-${sessionId}`, (event: any) => {
      const reason = event.payload?.reason || 'unknown';
      log.info('SessionClosed', 'Session closed', { sessionId, reason: sanitizeTerminalDisplayText(reason) });
      markTerminalStopped(sessionId);

      // Only show message if not manually closed
      if (reason !== 'user_closed') {
        writeTerminalStatus(term, '33', 'Session closed (Reason)', reason);
      }

      if (
        reason === 'connection_lost' ||
        reason === 'server_closed' ||
        reason === 'keepalive_failed'
      ) {
        updateSessionLifecycleState(sessionId, 'reconnecting', 'closed', reason);
        const appSettings = get(settings);
        if (appSettings.connection?.autoReconnect) {
          scheduleAutoReconnect(sessionId, term, false);
        } else {
          void term.write('\r\n\x1b[36mPress R to reconnect...\x1b[0m\r\n');

          detachTerminalInputListener(sessionId);
          armManualReconnectKey(sessionId, term);
        }
      } else if (reason === 'user_closed') {
        updateSessionLifecycleState(sessionId, 'closed', 'closed', reason);
      } else {
        updateSessionLifecycleState(sessionId, 'connected', 'closed', reason);
      }
    });
    if (disposed) {
      closedUnlisten();
      return;
    }
    unlisteners.push(closedUnlisten);
  } catch (error) {
    teardown();
    deleteOutputListenerCleanup(sessionId);
    throw error;
  }
}

function clearReconnectTimer(sessionId: string) {
  clearReconnectTimerState(sessionId);
}

function clearReconnectState(sessionId: string) {
  clearReconnectTimer(sessionId);
  clearReconnectKeyListener(sessionId);
  clearReconnectAttempts(sessionId);
}

function suppressReconnect(sessionId: string) {
  suppressReconnectState(sessionId);
  clearReconnectState(sessionId);
}

function releaseReconnectSuppression(sessionId: string) {
  if (getReconnectInFlight(sessionId)) {
    return;
  }
  releaseReconnectSuppressionState(sessionId);
}

function armReconnectKeyListener(
  sessionId: string,
  term: Terminal,
  onTrigger: () => void | Promise<void>
) {
  armTerminalKeyListener(sessionId, term, (data: string) => data === 'r' || data === 'R', onTrigger);
}

function armManualReconnectKey(sessionId: string, term: Terminal) {
  armReconnectKeyListener(sessionId, term, async () => {
    const ok = await reconnectTerminal(sessionId);
    if (ok || !canAttemptReconnectForSession(sessionId)) {
      return;
    }
    void term.write('\r\n\x1b[36mPress R to reconnect...\x1b[0m\r\n');
    armManualReconnectKey(sessionId, term);
  });
}

function scheduleAutoReconnect(sessionId: string, term: Terminal, immediate: boolean) {
  if (!canAttemptReconnectForSession(sessionId)) {
    return;
  }

  clearReconnectTimer(sessionId);
  clearReconnectKeyListener(sessionId);
  detachTerminalInputListener(sessionId);

  const attempts = getReconnectAttempts(sessionId);
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

  armReconnectKeyListener(sessionId, term, () => {
    scheduleAutoReconnect(sessionId, term, true);
  });

  const timer = window.setTimeout(async () => {
    clearReconnectTimerState(sessionId);
    clearReconnectKeyListener(sessionId);
    if (!canAttemptReconnectForSession(sessionId)) {
      return;
    }

    setReconnectAttempts(sessionId, attempts + 1);
    try {
      const ok = await reconnectTerminal(sessionId);
      if (ok) {
        clearReconnectAttempts(sessionId);
        return;
      }
    } catch (e) {
      log.warn('Reconnect', 'Auto reconnect attempt failed', e, {
        sessionId,
        attempt: getReconnectAttempts(sessionId),
      });
    }

    const appSettings = get(settings);
    if (appSettings.connection?.autoReconnect && canAttemptReconnectForSession(sessionId)) {
      scheduleAutoReconnect(sessionId, term, false);
    }
  }, delay);

  setReconnectTimer(sessionId, timer);
}

async function reconnectTerminalInternal(oldSessionId: string): Promise<boolean> {
  if (!canContinueReconnectForSession(oldSessionId)) {
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

  clearReconnectTimer(oldSessionId);
  clearReconnectKeyListener(oldSessionId);
  cleanupTerminalListeners(oldSessionId);
  updateSessionLifecycleState(oldSessionId, 'reconnecting', 'closed');

  void term.write('\r\n\x1b[33mReconnecting...\x1b[0m\r\n');

  try {
      const reconnectConfig = getReconnectConfig(oldSessionId) ?? terminalEntry.connection;
      const { sessionId: createdSessionId, connectConfig: effectiveReconnectConfig } =
        await connectWithInteractivePrompts(reconnectConfig, terminalEntry.connection.name);
      newSessionId = createdSessionId;
      const nextSessionId = createdSessionId;
      updateSessionLifecycleState(nextSessionId, 'connected', 'detached');

      cleanupTerminalListeners(nextSessionId);
      cleanupBufferedTerminalState(nextSessionId);
      await setupTerminalListeners(nextSessionId, term);
      
      // Start terminal
      const result = await invoke('start_terminal', {
          sessionId: nextSessionId,
          width: term.cols,
          height: term.rows,
      });

      if (!result) throw new Error('Failed to start terminal');

      cacheReconnectConfig(nextSessionId, effectiveReconnectConfig);

      if (!canContinueReconnectForSession(oldSessionId)) {
        cleanupTerminalListeners(nextSessionId);
        cleanupBufferedTerminalState(nextSessionId);
        markTerminalStopped(nextSessionId);
        removeTerminalSessionState(nextSessionId);
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

      const previousSize = lastTerminalSizes.get(oldSessionId);
      lastTerminalSizes.delete(oldSessionId);
      if (previousSize) {
        lastTerminalSizes.set(nextSessionId, previousSize);
      }

      let replaced = false;
      activeTerminals.update(items => {
        let foundRoot = false;
        const next = items.map(item => {
          if (item.sessionId === oldSessionId) {
            foundRoot = true;
            return { ...item, sessionId: nextSessionId, terminal: term, currentDirectory: undefined };
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
        markTerminalStopped(nextSessionId);
        removeTerminalSessionState(nextSessionId);
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
      clearReconnectAttempts(oldSessionId);
      clearReconnectTimer(oldSessionId);
      clearReconnectKeyListener(oldSessionId);
      clearReconnectInFlight(oldSessionId);
      cleanupBufferedTerminalState(oldSessionId);
      cleanupTerminalListeners(oldSessionId);
      clearReconnectAttempts(nextSessionId);
      clearReconnectTimer(nextSessionId);
      clearReconnectKeyListener(nextSessionId);
      clearReconnectInFlight(nextSessionId);
      releaseReconnectSuppressionState(oldSessionId);
      releaseReconnectSuppressionState(nextSessionId);
      attachTerminalInputListener(nextSessionId, term);
      removeTerminalSessionState(oldSessionId);
      updateSessionLifecycleState(nextSessionId, 'connected', 'attached');

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
      if (!canContinueReconnectForSession(oldSessionId)) {
        return false;
      }
      if (newSessionId) {
        cleanupTerminalListeners(newSessionId);
        cleanupBufferedTerminalState(newSessionId);
        markTerminalStopped(newSessionId);
        removeTerminalSessionState(newSessionId);
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
      const appSettings = get(settings);
      if (appSettings.connection?.autoReconnect) {
        updateSessionLifecycleState(oldSessionId, 'reconnecting', 'closed', 'reconnect_failed');
      } else {
        updateSessionLifecycleState(oldSessionId, 'connected', 'closed', 'reconnect_failed');
      }
      if (!appSettings.connection?.autoReconnect) {
        if (canAttemptReconnectForSession(oldSessionId)) {
          void term.write('\r\n\x1b[36mPress R to reconnect...\x1b[0m\r\n');
          armManualReconnectKey(oldSessionId, term);
        }
      }
      writeTerminalStatus(term, '31', 'Reconnection failed', e);
      showErrorMessage(`重连失败: ${terminalEntry.connection.name}`, 5000);
      return false;
  }
}

export async function reconnectTerminal(oldSessionId: string) {
  const existing = getReconnectInFlight(oldSessionId);
  if (existing) {
    return existing;
  }

  const reconnectPromise = reconnectTerminalInternal(oldSessionId).finally(() => {
    clearReconnectInFlight(oldSessionId, reconnectPromise);
    releaseReconnectSuppression(oldSessionId);
  });

  setReconnectInFlight(oldSessionId, reconnectPromise);
  return reconnectPromise;
}
