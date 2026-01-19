import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Terminal } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import { SearchAddon } from 'xterm-addon-search';
import { WebglAddon } from 'xterm-addon-webgl';
import { get } from 'svelte/store';
import { activeTerminals, selectedTerminalIndex, type Connection, type ActiveTerminal, errorMessage, successMessage, settings, connectionHistory, broadcastInputEnabled, broadcastSessionIds } from './store';
import { auditService } from './auditService';
import 'xterm/css/xterm.css';

const IS_DEV = import.meta.env.DEV;

// Output listeners storage
const outputListeners = new Map<string, () => void>();

// Session status monitoring
const sessionStatusListeners = new Map<string, () => void>();
const inputLineBuffers = new Map<string, string>();
const reconnectAttempts = new Map<string, number>();
const reconnectTimers = new Map<string, number>();
const reconnectKeyListeners = new Map<string, { dispose: () => void }>();
const MAX_AUTO_RECONNECT_RETRIES = 5;
const BASE_AUTO_RECONNECT_DELAY_MS = 1500;
const MAX_AUTO_RECONNECT_DELAY_MS = 30000;

type OutputWriteState = {
  chunks: string[];
  scheduled: number | null;
  writing: boolean;
  disposed: boolean;
};

type InputSendState = {
  buffer: string;
  timer: number | null;
  chain: Promise<void>;
};

const outputWriteStates = new Map<string, OutputWriteState>();
const inputSendStates = new Map<string, InputSendState>();

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
      fontSize: appSettings.terminal.fontSize,
      fontFamily: appSettings.terminal.fontFamily,
      theme: appSettings.theme === 'light' ? {
        background: '#ffffff', // white
        foreground: '#0f172a', // slate-950
        cursor: '#2563eb',     // blue-600
        selectionBackground: '#e2e8f0', // slate-200
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
      } : {
        background: '#0f172a', // slate-950
        foreground: '#e2e8f0', // slate-200
        cursor: '#3b82f6',     // blue-500
        selectionBackground: '#334155', // slate-700
      },
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

    // Handle resize
    const resizeObserver = new ResizeObserver(() => {
      // Only fit if the terminal is visible
      if (container.offsetParent !== null) {
          fitAddon.fit();
          sendTerminalResize(sessionId, term.cols, term.rows);
      }
    });
    resizeObserver.observe(container);

    // Handle user input
    term.onData((data) => {
      handleTerminalInput(sessionId, data, connection);
    });

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
    chain: Promise.resolve(),
  };
  inputSendStates.set(sessionId, created);
  return created;
}

function flushTerminalInput(sessionId: string, state: InputSendState) {
  const payload = state.buffer;
  if (!payload) return;
  state.buffer = '';
  state.chain = state.chain
    .then(() => invoke('send_terminal_data', { sessionId, data: payload }) as Promise<unknown>)
    .then(() => undefined)
    .catch((error) => {
      console.error('Failed to send terminal data:', error);
    });
}

function sendTerminalDataBuffered(sessionId: string, data: string, immediate: boolean) {
  const state = getInputSendState(sessionId);
  state.buffer += data;

  if (state.buffer.length >= 1024) {
    immediate = true;
  }

  if (state.timer !== null) {
    if (!immediate) return state.chain;
    clearTimeout(state.timer);
    state.timer = null;
  }

  if (immediate) {
    flushTerminalInput(sessionId, state);
    return state.chain;
  }

  state.timer = window.setTimeout(() => {
    state.timer = null;
    flushTerminalInput(sessionId, state);
  }, 10);

  return state.chain;
}

function handleTerminalInputSingle(sessionId: string, data: string, connection: Connection) {
  const currentBuffer = inputLineBuffers.get(sessionId) ?? '';
  let buffer = currentBuffer;

  const flushBuffer = (next: string) => {
    buffer = next;
    inputLineBuffers.set(sessionId, next);
  };

  const commitBuffer = async () => {
    const command = buffer.trim();
    flushBuffer('');
    if (!command) {
      await sendTerminalDataBuffered(sessionId, '\r', true);
      return;
    }

    const analysis = auditService.analyzeCommand(command);
    const details = { detectedPatterns: analysis.detectedPatterns };

    if (!auditService.shouldPrompt(analysis.riskLevel)) {
      await auditService.recordEvent(
        auditService.createEvent({
          command,
          sessionId,
          userId: connection.username,
          action: 'ALLOWED',
          details
        })
      );
      await sendTerminalDataBuffered(sessionId, '\r', true);
      return;
    }

    const confirmed = window.confirm(
      [
        `检测到高危命令（${analysis.riskLevel}）`,
        `描述：${analysis.description}`,
        '',
        command,
        '',
        '是否继续执行？'
      ].join('\n')
    );

    if (!confirmed) {
      await auditService.recordEvent(
        auditService.createEvent({
          command,
          sessionId,
          userId: connection.username,
          action: 'BLOCKED',
          details
        })
      );
      void sendTerminalDataBuffered(sessionId, '\u0003', true);
      return;
    }

    await auditService.recordEvent(
      auditService.createEvent({
        command,
        sessionId,
        userId: connection.username,
        action: 'ALLOWED',
        details
      })
    );
    await sendTerminalDataBuffered(sessionId, '\r', true);
  };

  for (const ch of data) {
    if (ch === '\r' || ch === '\n') {
      void commitBuffer();
      continue;
    }

    if (ch === '\u0003' || ch === '\u0015') {
      flushBuffer('');
      void sendTerminalDataBuffered(sessionId, ch, true);
      continue;
    }

    if (ch === '\u007f') {
      if (buffer.length > 0) {
        flushBuffer(buffer.slice(0, -1));
      }
      void sendTerminalDataBuffered(sessionId, ch, true);
      continue;
    }

    if (ch === '\u001b') {
      void sendTerminalDataBuffered(sessionId, data, true);
      return;
    }

    if (ch >= ' ' && ch <= '~') {
      flushBuffer(buffer + ch);
      void sendTerminalDataBuffered(sessionId, ch, false);
      continue;
    }

    void sendTerminalDataBuffered(sessionId, ch, true);
  }
}

function handleTerminalInput(sessionId: string, data: string, connection: Connection) {
  const enabled = get(broadcastInputEnabled);
  const selected = get(broadcastSessionIds);

  if (!enabled) {
    handleTerminalInputSingle(sessionId, data, connection);
    return;
  }

  const terminals = get(activeTerminals);
  const connectionBySessionId = new Map(terminals.map(t => [t.sessionId, t.connection] as const));

  const targets = (selected.length > 0 ? selected : [sessionId]).includes(sessionId)
    ? (selected.length > 0 ? selected : [sessionId])
    : [sessionId, ...(selected.length > 0 ? selected : [])];

  for (const targetSessionId of targets) {
    const targetConnection = connectionBySessionId.get(targetSessionId);
    if (!targetConnection) continue;
    handleTerminalInputSingle(targetSessionId, data, targetConnection);
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
    terminal.terminal.dispose();

    const outputState = outputWriteStates.get(sessionId);
    if (outputState) {
      outputState.disposed = true;
      if (outputState.scheduled !== null) {
        cancelAnimationFrame(outputState.scheduled);
      }
      outputWriteStates.delete(sessionId);
    }

    const inputState = inputSendStates.get(sessionId);
    if (inputState) {
      if (inputState.timer !== null) {
        clearTimeout(inputState.timer);
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

async function setupTerminalListeners(sessionId: string, term: Terminal) {
    const outputState: OutputWriteState = {
      chunks: [],
      scheduled: null,
      writing: false,
      disposed: false,
    };
    outputWriteStates.set(sessionId, outputState);

    const flushOutput = () => {
      if (outputState.disposed) return;
      outputState.scheduled = null;
      if (outputState.writing) return;
      if (outputState.chunks.length === 0) return;
      const data = outputState.chunks.join('');
      outputState.chunks = [];
      outputState.writing = true;
      term.write(data, () => {
        outputState.writing = false;
        if (outputState.chunks.length > 0 && !outputState.disposed && outputState.scheduled === null) {
          outputState.scheduled = requestAnimationFrame(flushOutput);
        }
      });
    };

    // Output listener
    const outputUnlisten = await listen(`terminal-output-${sessionId}`, (event: any) => {
      if (event.payload && event.payload.data) {
        outputState.chunks.push(event.payload.data);
        if (outputState.scheduled === null && !outputState.writing) {
          outputState.scheduled = requestAnimationFrame(flushOutput);
        }
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
          cancelAnimationFrame(outputState.scheduled);
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
