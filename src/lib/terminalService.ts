import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Terminal } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import { SearchAddon } from 'xterm-addon-search';
import { WebglAddon } from 'xterm-addon-webgl';
import { get } from 'svelte/store';
import { activeTerminals, connections, selectedTerminalIndex, type Connection, type ActiveTerminal, errorMessage, successMessage, settings, connectionHistory, broadcastInputEnabled, broadcastSessionIds, getStoredTerminalUiState, getXtermTheme } from './store';
import 'xterm/css/xterm.css';

const IS_DEV = import.meta.env.DEV;

// Output listeners storage
const outputListeners = new Map<string, () => void>();
const inputListeners = new Map<string, { dispose: () => void }>();

// Session status monitoring
const sessionStatusListeners = new Map<string, () => void>();
const reconnectAttempts = new Map<string, number>();
const reconnectTimers = new Map<string, number>();
const reconnectKeyListeners = new Map<string, { dispose: () => void }>();
const MAX_AUTO_RECONNECT_RETRIES = 5;
const BASE_AUTO_RECONNECT_DELAY_MS = 1500;
const MAX_AUTO_RECONNECT_DELAY_MS = 30000;

type OutputWriteState = {
  chunks: string[];
  chunkIndex: number;
  scheduled: { id: number; kind: 'raf' | 'timeout' } | null;
  writing: boolean;
  disposed: boolean;
  paused: boolean;
  pausedBuffer: string;
  chunkBudget: number;
};

type InputSendState = {
  buffer: string;
  timer: { id: number; kind: 'raf' | 'timeout' } | null;
  sending: boolean;
};

const outputWriteStates = new Map<string, OutputWriteState>();
const inputSendStates = new Map<string, InputSendState>();

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
      selectedTerminalIndex.set(terminals.length);

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
    
    // Select the new terminal
    selectedTerminalIndex.set(terminals.length);

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
    console.error('Error connecting to:', connection.name, error);
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

export async function initTerminal(container: HTMLElement, sessionId: string, connection: Connection): Promise<ActiveTerminal | null> {
  try {
    // Clear container
    container.innerHTML = '';

    // Get current settings
    const appSettings = get(settings);

    // Create new Terminal instance
    const term = new Terminal({
      cursorBlink: appSettings.terminal.cursorBlink,
      cursorStyle: appSettings.terminal.cursorStyle,
      cursorWidth: 1,
      fontSize: appSettings.terminal.fontSize,
      fontFamily: appSettings.terminal.fontFamily,
      theme: getXtermTheme(appSettings),
      scrollback: appSettings.terminal.scrollback,
      allowProposedApi: true,
      convertEol: true, // Enable EOL conversion to fix line endings
    });

    // Create fit addon and load
    const fitAddon = new FitAddon();
    const searchAddon = new SearchAddon();
    term.loadAddon(fitAddon);
    term.loadAddon(searchAddon);
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
    } catch (e) {
      if (IS_DEV) console.warn('WebGL addon unavailable:', e);
    }

    // Fit terminal to container
    // We need to wait a bit for the container to have dimensions
    setTimeout(() => {
        fitAddon.fit();
    }, 100);

    // Handle user input
    const inputDisposable = term.onData((data) => {
      handleTerminalInput(sessionId, data, connection);
    });
    inputListeners.set(sessionId, inputDisposable);

    // Listen for terminal output from backend
    await setupTerminalListeners(sessionId, term);

    // Start monitoring session status
    // await monitorSessionStatus(sessionId); // Deprecated in favor of session-closed event

    // Request terminal session from backend
    const result = await invoke('start_terminal', {
      sessionId,
      width: term.cols,
      height: term.rows,
    });

    if (!result) {
      console.error('Failed to start terminal session');
      term.write('\r\n\x1b[31mFailed to start terminal session\x1b[0m\r\n');
      errorMessage.set('启动终端会话失败');
      setTimeout(() => errorMessage.set(null), 5000);
      return null;
    }

    term.focus();

    // Return the terminal object
    // Note: We don't store the resizeObserver here as it's attached to the DOM element
    // We might need to handle cleanup separately if the component is destroyed but terminal stays
    
    // Update the store with the initialized terminal instance
    // This is crucial for features like broadcast input to work correctly
    activeTerminals.update(items => items.map(t => {
      if (t.sessionId === sessionId) {
        return { ...t, terminal: term, fitAddon, searchAddon };
      }
      return t;
    }));

    return {
      sessionId,
      connection,
      terminal: term,
      fitAddon,
      searchAddon,
    };
  } catch (error) {
    console.error('Failed to initialize terminal:', error);
    errorMessage.set(`初始化终端失败: ${error}`);
    setTimeout(() => errorMessage.set(null), 5000);
    return null;
  }
}

export async function sendTerminalData(sessionId: string, data: string) {
  try {
    await invoke('send_terminal_data', { sessionId, data });
  } catch (error) {
    console.error('Failed to send terminal data:', error);
  }
}

function getInputSendState(sessionId: string): InputSendState {
  const existing = inputSendStates.get(sessionId);
  if (existing) return existing;
  const created: InputSendState = {
    buffer: '',
    timer: null,
    sending: false,
  };
  inputSendStates.set(sessionId, created);
  return created;
}

async function flushTerminalInput(sessionId: string, state: InputSendState) {
  if (state.sending) return;
  if (!state.buffer) return;
  state.sending = true;
  try {
    const payload = state.buffer.slice(0, 2048);
    state.buffer = state.buffer.slice(payload.length);
    await invoke('send_terminal_data', { sessionId, data: payload });
  } catch (error) {
    console.error('Failed to send terminal data:', error);
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

function sendTerminalDataBuffered(sessionId: string, data: string, immediate: boolean) {
  const state = getInputSendState(sessionId);
  state.buffer += data;

  // 设定一个固定的较大阈值，防止缓冲区过大
  // 1024 字符已经相当多了，足以覆盖大多数快速输入场景
  if (state.buffer.length >= 1024) {
    if (!immediate && IS_DEV) {
      console.log(`[TermInput] Force immediate due to buffer size: ${state.buffer.length}`);
    }
    immediate = true;
  }

  if (state.timer !== null) {
    if (!immediate) return;
    cancelScheduled(state.timer);
    state.timer = null;
  }

  if (immediate) {
    if (IS_DEV) {
       console.log(`[TermInput] Immediate flush: len=${data.length}, buf=${state.buffer.length}, data=${JSON.stringify(data)}`);
    }
    void flushTerminalInput(sessionId, state);
    return;
  }

  // 使用 requestAnimationFrame 替代 setTimeout，延迟从10ms降至3ms
  // 这能提供更平滑的时序，避免setTimeout的精度问题
  state.timer = scheduleNext(() => {
    state.timer = null;
    void flushTerminalInput(sessionId, state);
  });

  return;
}

function handleTerminalInputSingle(sessionId: string, data: string) {
  const hasControl =
    data.includes('\r') ||
    data.includes('\n') ||
    data.includes('\x03') ||
    data.includes('\x1b');

  const shouldImmediate = hasControl;
  
  // Debug log for control character detection
  if (shouldImmediate && IS_DEV) {
     console.log(`[TermInput] Control char detected: ${JSON.stringify(data)}`);
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
  const targets = baseTargets.includes(sessionId) ? baseTargets : [sessionId, ...baseTargets];

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
  } catch (error) {
    console.error('Failed to resize terminal:', error);
  }
}

export async function monitorSessionStatus(sessionId: string) {
  try {
    // Listen for session status changes from backend
    const statusUnlisten = await listen(`session-status-${sessionId}`, (event: any) => {
      if (event.payload && event.payload.status) {
        if (IS_DEV) console.log('Session status changed:', sessionId, event.payload.status);
        
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
    console.error('Failed to monitor session status:', error);
  }
}

export async function closeTerminal(sessionId: string) {
  const terminals = get(activeTerminals);
  const index = terminals.findIndex(t => t.sessionId === sessionId);
  
  if (index !== -1) {
    const terminal = terminals[index];
    
    // Clean up xterm instance
    try {
      (terminal as any).terminal?.dispose?.();
    } catch (e) {
      if (IS_DEV) console.warn('Failed to dispose terminal instance:', e);
    }

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
    
    // Remove from store
    activeTerminals.update(items => items.filter(t => t.sessionId !== sessionId));
    broadcastSessionIds.update(ids => ids.filter(id => id !== sessionId));
    
    // Adjust selected index
    const currentIndex = get(selectedTerminalIndex);
    if (currentIndex >= terminals.length - 1) {
      selectedTerminalIndex.set(Math.max(0, terminals.length - 2));
    }

    // Clean up output listener
    const unlisten = outputListeners.get(sessionId);
    if (unlisten) {
      unlisten();
      outputListeners.delete(sessionId);
    }

    // Clean up input listener
    const inputListener = inputListeners.get(sessionId);
    if (inputListener) {
      inputListener.dispose();
      inputListeners.delete(sessionId);
    }

    // Clean up status listener
    const statusUnlisten = sessionStatusListeners.get(sessionId);
    if (statusUnlisten) {
      statusUnlisten();
      sessionStatusListeners.delete(sessionId);
    }

    // Notify backend to close terminal
    try {
      await invoke('close_terminal', { sessionId });
    } catch (error) {
      console.error('Failed to close terminal:', error);
    }
  }
}

export async function disconnectTerminal(sessionId: string) {
  const terminals = get(activeTerminals);
  const terminal = terminals.find(t => t.sessionId === sessionId);
  const name = terminal?.connection?.name ?? sessionId;

  clearReconnectTimer(sessionId);
  clearReconnectKeyListener(sessionId);
  reconnectAttempts.delete(sessionId);

  await closeTerminal(sessionId);

  try {
    await invoke('close_terminal', { sessionId });
  } catch (error) {
    if (IS_DEV) console.warn('Failed to close_terminal during disconnect:', error);
  }

  try {
    await invoke('disconnect', { sessionId });
    successMessage.set(`已断开连接: ${name}`);
    setTimeout(() => successMessage.set(null), 3000);
  } catch (error) {
    errorMessage.set(`断开连接失败: ${name}`);
    setTimeout(() => errorMessage.set(null), 5000);
    if (IS_DEV) console.warn('Failed to disconnect:', error);
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
    } catch {
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
    if (IS_DEV) console.warn('Failed to get_all_sessions:', e);
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
    const outputState: OutputWriteState = {
      chunks: [],
      chunkIndex: 0,
      scheduled: null,
      writing: false,
      disposed: false,
      paused: false,
      pausedBuffer: '',
      chunkBudget: 256 * 1024,
    };
    outputWriteStates.set(sessionId, outputState);

    const MAX_PAUSED_BUFFER = 2 * 1024 * 1024;
    const TARGET_WRITE_MS = 12;

    function flushOutput() {
      if (outputState.disposed) return;
      outputState.scheduled = null;
      if (outputState.paused) return;
      if (outputState.writing) return;
      if (outputState.chunkIndex >= outputState.chunks.length) return;

      const CHUNK_LIMIT = outputState.chunkBudget;
      let count = 0;
      const parts: string[] = [];

      while (outputState.chunkIndex < outputState.chunks.length) {
        const nextChunk = outputState.chunks[outputState.chunkIndex];
        if (count + nextChunk.length > CHUNK_LIMIT && count > 0) break;

        parts.push(nextChunk);
        count += nextChunk.length;
        outputState.chunkIndex += 1;

        if (count >= CHUNK_LIMIT) break;
      }

      if (outputState.chunkIndex > 2048 || outputState.chunkIndex > Math.floor(outputState.chunks.length / 2)) {
        outputState.chunks.splice(0, outputState.chunkIndex);
        outputState.chunkIndex = 0;
      }

      const payload = parts.join('');
      if (payload.length === 0) return;

      outputState.writing = true;
      const writeStart = performance.now();
      term.write(payload, () => {
        const writeDuration = performance.now() - writeStart;
        if (writeDuration > TARGET_WRITE_MS) {
          outputState.chunkBudget = Math.max(64 * 1024, Math.floor(outputState.chunkBudget * 0.7));
        } else if (writeDuration < TARGET_WRITE_MS / 2) {
          outputState.chunkBudget = Math.min(1024 * 1024, Math.floor(outputState.chunkBudget * 1.15));
        }

        if (IS_DEV && writeDuration > 32) {
          console.warn(
            `[TermOutput] slow write ${sessionId}: ${writeDuration.toFixed(1)}ms, payload=${payload.length}, pending=${outputState.chunks.length}, budget=${outputState.chunkBudget}`,
          );
        }
        outputState.writing = false;
        if (!outputState.paused && outputState.chunkIndex < outputState.chunks.length && !outputState.disposed) {
          scheduleFlush();
        }
      });
    }

    const scheduleFlush = () => {
      if (outputState.disposed) return;
      if (outputState.paused) return;
      if (outputState.scheduled !== null) return;
      outputState.scheduled = scheduleNext(flushOutput);
    };

    const computePaused = () => {
      const hidden = typeof document !== 'undefined' && document.hidden === true;
      const terminals = get(activeTerminals);
      const index = get(selectedTerminalIndex);
      const selectedSessionId = terminals[index]?.sessionId;
      return hidden || selectedSessionId !== sessionId;
    };

    const applyPausedState = () => {
      const nextPaused = computePaused();
      if (nextPaused === outputState.paused) return;
      outputState.paused = nextPaused;

      if (outputState.paused) {
        if (outputState.scheduled !== null) {
          cancelScheduled(outputState.scheduled);
          outputState.scheduled = null;
        }
        return;
      }

      if (outputState.pausedBuffer) {
        outputState.chunks.push(outputState.pausedBuffer);
        outputState.pausedBuffer = '';
      }
      if (!outputState.writing) scheduleFlush();
    };

    applyPausedState();

    const onVisibilityChange = () => {
      if (outputState.disposed) return;
      applyPausedState();
    };

    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', onVisibilityChange);
    }

    const unsubscribeSelected = selectedTerminalIndex.subscribe(() => {
      if (outputState.disposed) return;
      applyPausedState();
    });

    const unsubscribeTerminals = activeTerminals.subscribe(() => {
      if (outputState.disposed) return;
      applyPausedState();
    });

    // Output listener
    const outputUnlisten = await listen(`terminal-output-${sessionId}`, (event: any) => {
      if (event.payload && event.payload.data) {
        const data = String(event.payload.data);

        if (outputState.paused) {
          outputState.pausedBuffer += data;
          if (outputState.pausedBuffer.length > MAX_PAUSED_BUFFER) {
            outputState.pausedBuffer = outputState.pausedBuffer.slice(-MAX_PAUSED_BUFFER);
          }
          return;
        }

        outputState.chunks.push(data);
        if (!outputState.writing) scheduleFlush();
      }
    });

    // Error listener
    const errorUnlisten = await listen(`terminal-error-${sessionId}`, (event: any) => {
      if (event.payload && event.payload.error) {
        console.error('Terminal error:', event.payload.error);
        term.write(`\r\n\x1b[31mError: ${event.payload.error}\x1b[0m\r\n`);
        errorMessage.set(`终端错误: ${event.payload.error}`);
        setTimeout(() => errorMessage.set(null), 5000);
      }
    });

    // Session Closed listener
    const closedUnlisten = await listen(`session-closed-${sessionId}`, (event: any) => {
        const reason = event.payload?.reason || 'unknown';
        if (IS_DEV) console.log('Session closed:', sessionId, reason);
        
        // Only show message if not manually closed
        if (reason !== 'user_closed') {
             term.write(`\r\n\x1b[33mSession closed (Reason: ${reason})\x1b[0m\r\n`);
        }
        
        if (reason === 'connection_lost' || reason === 'server_closed' || reason === 'keepalive_failed') {
            const appSettings = get(settings);
            if (appSettings.connection?.autoReconnect) {
                scheduleAutoReconnect(sessionId, term, false);
            } else {
                term.write('\r\n\x1b[36mPress R to reconnect...\x1b[0m\r\n');
                
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

        if (typeof document !== 'undefined') {
          document.removeEventListener('visibilitychange', onVisibilityChange);
        }
        unsubscribeSelected();
        unsubscribeTerminals();

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
    term.write(`\r\n\x1b[31mAuto reconnect stopped after ${MAX_AUTO_RECONNECT_RETRIES} attempts.\x1b[0m\r\n`);
    errorMessage.set('自动重连已停止：超过最大重试次数');
    setTimeout(() => errorMessage.set(null), 5000);
    return;
  }

  const delay = immediate
    ? 0
    : Math.min(MAX_AUTO_RECONNECT_DELAY_MS, Math.floor(BASE_AUTO_RECONNECT_DELAY_MS * Math.pow(2, attempts)));
  const seconds = Math.max(0, Math.ceil(delay / 1000));
  term.write(`\r\n\x1b[33mAuto reconnect attempt ${attempts + 1}/${MAX_AUTO_RECONNECT_RETRIES} in ${seconds}s... (Press R to immediate)\x1b[0m\r\n`);

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
      if (IS_DEV) console.warn('Auto reconnect attempt failed', e);
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

  term.write('\r\n\x1b[33mReconnecting...\x1b[0m\r\n');

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
      term.write(`\r\n\x1b[31mReconnection failed: ${e}\x1b[0m\r\n`);
      errorMessage.set(`重连失败: ${terminalEntry.connection.name}`);
      setTimeout(() => errorMessage.set(null), 5000);
      return false;
  }
}
