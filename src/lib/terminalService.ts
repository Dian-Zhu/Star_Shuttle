import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Terminal } from 'xterm';
import { FitAddon } from 'xterm-addon-fit';
import { get } from 'svelte/store';
import { activeTerminals, selectedTerminalIndex, type Connection, type ActiveTerminal, errorMessage, successMessage, settings } from './store';
import 'xterm/css/xterm.css';

// Output listeners storage
const outputListeners = new Map<string, () => void>();

// Session status monitoring
const sessionStatusListeners = new Map<string, () => void>();

export async function connectAndOpen(connection: Connection) {
  try {
    errorMessage.set(null);
    console.log('Connecting to:', connection.name);

    // Call backend connect command
    const result = await invoke('connect', { config: connection });
    const sessionId = result as string;
    
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
        fitAddon: null as any
      }
    ]);
    
    // Select the new terminal
    selectedTerminalIndex.set(terminals.length);

    successMessage.set(`连接成功: ${connection.name}`);
    setTimeout(() => successMessage.set(null), 3000);
    
  } catch (error) {
    console.error('Error connecting to:', connection.name, error);
    errorMessage.set(`连接失败：${error}`);
    setTimeout(() => errorMessage.set(null), 5000);
  }
}

export async function createTerminalSession(connection: Connection): Promise<string> {
  console.log('Connecting to:', connection.name);
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
      scrollback: 5000,
      allowProposedApi: true,
      convertEol: true, // Enable EOL conversion to fix line endings
    });

    // Create fit addon and load
    const fitAddon = new FitAddon();
    term.loadAddon(fitAddon);
    term.open(container);

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
      sendTerminalData(sessionId, data);
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
        console.log('Session status changed:', sessionId, event.payload.status);
        
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
    
    // Remove from store
    activeTerminals.update(items => items.filter(t => t.sessionId !== sessionId));
    
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
    // Output listener
    const outputUnlisten = await listen(`terminal-output-${sessionId}`, (event: any) => {
      if (event.payload && event.payload.data) {
        term.write(event.payload.data);
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
        console.log('Session closed:', sessionId, reason);
        
        // Only show message if not manually closed
        if (reason !== 'user_closed') {
             term.write(`\r\n\x1b[33mSession closed (Reason: ${reason})\x1b[0m\r\n`);
        }
        
        if (reason === 'connection_lost' || reason === 'server_closed' || reason === 'keepalive_failed') {
            const appSettings = get(settings);
            if (appSettings.connection?.autoReconnect) {
                let countdown = 3;
                term.write(`\r\n\x1b[33mAuto reconnecting in ${countdown}s... (Press R to immediate)\x1b[0m\r\n`);
                
                let cancelled = false;
                const disposable = term.onData(async (data) => {
                     if (data === 'r' || data === 'R') {
                        cancelled = true;
                        disposable.dispose();
                        await reconnectTerminal(sessionId);
                     }
                });

                const doReconnect = async () => {
                    for (let i = 0; i < 3; i++) {
                         await new Promise(r => setTimeout(r, 1000));
                         if (cancelled) return;
                         countdown--;
                         if (countdown > 0) {
                            term.write(`\r\n\x1b[33mAuto reconnecting in ${countdown}s... (Press R to immediate)\x1b[0m\r\n`);
                         }
                    }
                    if (!cancelled) {
                        disposable.dispose();
                        await reconnectTerminal(sessionId);
                    }
                };
                doReconnect();

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
        outputUnlisten();
        errorUnlisten();
        closedUnlisten();
    });
}

export async function reconnectTerminal(oldSessionId: string) {
  const terminals = get(activeTerminals);
  const index = terminals.findIndex(t => t.sessionId === oldSessionId);
  if (index === -1) return;

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
      
      // Setup new listeners
      await setupTerminalListeners(newSessionId, term);
      
      term.write('\r\n\x1b[32mReconnected!\x1b[0m\r\n');
      term.focus();
      
      // Trigger resize
      await sendTerminalResize(newSessionId, term.cols, term.rows);

  } catch (e) {
      term.write(`\r\n\x1b[31mReconnection failed: ${e}\x1b[0m\r\n`);
      
      const appSettings = get(settings);
      if (appSettings.connection?.autoReconnect) {
          let countdown = 5; // Longer delay for retry on failure
          term.write(`\r\n\x1b[33mRetrying in ${countdown}s... (Press R to immediate)\x1b[0m\r\n`);
          
          let cancelled = false;
          const disposable = term.onData(async (data) => {
                if (data === 'r' || data === 'R') {
                  cancelled = true;
                  disposable.dispose();
                  await reconnectTerminal(oldSessionId);
                }
          });

          const doRetry = async () => {
              for (let i = 0; i < 5; i++) {
                    await new Promise(r => setTimeout(r, 1000));
                    if (cancelled) return;
                    countdown--;
                    if (countdown > 0) {
                      term.write(`\r\n\x1b[33mRetrying in ${countdown}s... (Press R to immediate)\x1b[0m\r\n`);
                    }
              }
              if (!cancelled) {
                  disposable.dispose();
                  await reconnectTerminal(oldSessionId);
              }
          };
          doRetry();

      } else {
          term.write('\r\n\x1b[36mPress R to retry...\x1b[0m\r\n');
          
          const disposable = term.onData(async (data) => {
              if (data === 'r' || data === 'R') {
                  disposable.dispose();
                  await reconnectTerminal(oldSessionId);
              }
          });
      }
  }
}
