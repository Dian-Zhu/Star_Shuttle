<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { activeTerminals, selectedTerminalIndex, broadcastInputEnabled, broadcastSessionIds } from '../lib/store';
  import TerminalView from './TerminalView.svelte';
  import TerminalIcon from './icons/TerminalIcon.svelte';
  import { formatSpeed } from '../lib/transferQueueService';

  let currentTime = '';
  let netSpeedBps = 0;
  let cpuUsage = 0;
  let memPercent = 0;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let clockTimer: ReturnType<typeof setInterval> | null = null;
  let pollInFlight = false;
  let pollingEnabled = false;
  const lastNetSampleBySession = new Map<string, { rx: number; tx: number; time: number }>();
  const POLL_INTERVAL_MS = 5000;

  function updateClock() {
    currentTime = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  function stopPolling() {
    pollingEnabled = false;
    if (pollTimer) {
      clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  function startPolling(): boolean {
    if (pollingEnabled) return false;
    pollingEnabled = true;
    if (!pollTimer) {
      pollTimer = setInterval(() => void pollSelectedSession(), POLL_INTERVAL_MS);
    }
    return true;
  }

  $: rootTerminals = $activeTerminals.filter(t => !t.parentId);

  function updatePollingState(): boolean {
    const hasTerminal = rootTerminals.length > 0;
    const visible = typeof document === 'undefined' ? true : document.hidden !== true;
    const focused = typeof document === 'undefined' ? true : (document.hasFocus?.() ?? true);
    const shouldEnable = hasTerminal && visible && focused;

    if (!shouldEnable) {
      stopPolling();
      return false;
    }

    return startPolling();
  }

  function parseCpuUsage(output: string) {
    const match = output.match(/(\d+\.\d+)\s*id/);
    if (match) {
      const idle = parseFloat(match[1]);
      if (!Number.isNaN(idle)) cpuUsage = Math.max(0, Math.round((100 - idle) * 10) / 10);
    }
  }

  function parseMemPercent(output: string) {
    const lines = output.split('\n');
    const memLine = lines.find(l => l.startsWith('Mem:'));
    if (!memLine) return;
    const parts = memLine.split(/\s+/);
    if (parts.length < 4) return;
    const total = parseInt(parts[1]);
    const used = parseInt(parts[2]);
    if (Number.isNaN(total) || total <= 0) return;
    const value = Number.isNaN(used) ? 0 : used;
    memPercent = Math.max(0, Math.min(100, Math.round((value / total) * 100)));
    if (parts.length >= 7) {
      const avail = parseInt(parts[6]);
      if (!Number.isNaN(avail)) {
        const adjustedUsed = total - avail;
        memPercent = Math.max(0, Math.min(100, Math.round((adjustedUsed / total) * 100)));
      }
    }
  }

  function parseNetBytes(output: string): { rx: number; tx: number } {
    const lines = output.split('\n').slice(2);
    let rx = 0;
    let tx = 0;
    for (const raw of lines) {
      const line = raw.trim();
      if (!line) continue;
      const parts = line.split(/[:\s]+/).filter(Boolean);
      if (parts.length < 10) continue;
      const iface = parts[0];
      if (iface === 'lo') continue;
      const rxBytes = Number(parts[1]);
      const txBytes = Number(parts[9]);
      if (!Number.isFinite(rxBytes) || !Number.isFinite(txBytes)) continue;
      rx += rxBytes;
      tx += txBytes;
    }
    return { rx, tx };
  }

  async function pollSelectedSession() {
    if (!pollingEnabled) return;
    if (pollInFlight) return;
    const terminal = $activeTerminals[$selectedTerminalIndex];
    if (!terminal) {
      netSpeedBps = 0;
      cpuUsage = 0;
      memPercent = 0;
      return;
    }

    const sessionId = terminal.sessionId;
    pollInFlight = true;
    try {
      // Combine commands to reduce overhead
      // cat /proc/net/dev; echo "---"; top -bn1 | grep "Cpu(s)"; echo "---"; free -m
      const combinedCommand = 'cat /proc/net/dev; echo "---STAR_SHUTTLE_SPLIT---"; top -bn1 | grep "Cpu(s)"; echo "---STAR_SHUTTLE_SPLIT---"; free -m';
      const output = await invoke('exec_command', { sessionId, command: combinedCommand }) as string;
      
      const parts = output.split('---STAR_SHUTTLE_SPLIT---');
      if (parts.length >= 3) {
        const [netOut, cpuOut, memOut] = parts;
        
        const now = performance.now();
        const { rx, tx } = parseNetBytes(netOut);
        const last = lastNetSampleBySession.get(sessionId);
        if (last) {
          const dt = (now - last.time) / 1000;
          if (dt > 0) {
            const delta = Math.max(0, (rx - last.rx) + (tx - last.tx));
            netSpeedBps = delta / dt;
          }
        }
        lastNetSampleBySession.set(sessionId, { rx, tx, time: now });

        parseCpuUsage(cpuOut);
        parseMemPercent(memOut);
      }
    } catch {
      netSpeedBps = 0;
      cpuUsage = 0;
      memPercent = 0;
      lastNetSampleBySession.delete(sessionId);
    } finally {
      pollInFlight = false;
    }
  }

  $: {
    $activeTerminals;
    $selectedTerminalIndex;
    updatePollingState();
    if (pollingEnabled) {
      void pollSelectedSession();
    }
  }

  onMount(() => {
    updateClock();
    clockTimer = setInterval(updateClock, 1000);
    updatePollingState();

    const onVisibilityChange = () => {
      if (updatePollingState()) void pollSelectedSession();
    };
    const onFocus = () => {
      if (updatePollingState()) void pollSelectedSession();
    };
    const onBlur = () => {
      updatePollingState();
    };

    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', onVisibilityChange);
    }
    if (typeof window !== 'undefined') {
      window.addEventListener('focus', onFocus);
      window.addEventListener('blur', onBlur);
    }

    return () => {
      if (typeof document !== 'undefined') {
        document.removeEventListener('visibilitychange', onVisibilityChange);
      }
      if (typeof window !== 'undefined') {
        window.removeEventListener('focus', onFocus);
        window.removeEventListener('blur', onBlur);
      }
      if (clockTimer) clearInterval(clockTimer);
      stopPolling();
    };
  });
</script>

<div class="flex flex-col h-full w-full bg-app-bg">
  <!-- Terminal Content Area -->
  <div class="flex-1 relative overflow-hidden">
    {#if rootTerminals.length === 0}
      <div class="absolute inset-0 flex flex-col items-center justify-center text-app-text-secondary bg-app-surface">
        <div class="w-16 h-16 mb-4 rounded-2xl bg-app-bg flex items-center justify-center border border-app-border shadow-sm">
          <TerminalIcon class="w-8 h-8 opacity-50" />
        </div>
        <p class="text-lg font-medium text-app-text">无活动会话</p>
        <p class="text-sm mt-2 opacity-60">请从左侧列表选择连接以开始</p>
      </div>
    {:else}
      {#each rootTerminals as terminal (terminal.sessionId)}
        <TerminalView 
          terminalData={terminal} 
          isVisible={terminal.sessionId === $activeTerminals[$selectedTerminalIndex]?.sessionId || terminal.sessionId === $activeTerminals[$selectedTerminalIndex]?.parentId} 
        />
      {/each}
    {/if}
  </div>

  <!-- Status Bar -->
  <div class="flex items-center justify-between px-4 py-1.5 bg-app-status-bar border-t border-app-border text-xs text-app-text-secondary">
    <div class="flex items-center gap-4">
      <div class="flex items-center gap-1">
        <svg class="w-3.5 h-3.5 text-primary-500 dark:text-primary-400" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
          <path fill-rule="evenodd" d="M12.293 5.293a1 1 0 011.414 0l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414-1.414L14.586 11H3a1 1 0 110-2h11.586l-2.293-2.293a1 1 0 010-1.414z" clip-rule="evenodd"></path>
        </svg>
        <span>实时流量:</span>
        <span class="font-mono font-medium text-green-600 dark:text-green-300">{formatSpeed(netSpeedBps)}</span>
        <span class="ml-2 text-app-text-secondary">CPU</span>
        <span class="font-mono font-medium text-app-text">{cpuUsage}%</span>
        <span class="ml-2 text-app-text-secondary">MEM</span>
        <span class="font-mono font-medium text-app-text">{memPercent}%</span>
      </div>
      <div class="text-app-text-secondary">|</div>
      <div class="text-app-text-secondary">
        活动传输: <span class="text-app-text">{$activeTerminals.length}</span>
      </div>
    </div>
    <div class="text-app-text-secondary text-xs">
      {currentTime}
    </div>
  </div>
</div>

<style>
  .no-scrollbar::-webkit-scrollbar {
    display: none;
  }
  .no-scrollbar {
    -ms-overflow-style: none;
    scrollbar-width: none;
  }
</style>
