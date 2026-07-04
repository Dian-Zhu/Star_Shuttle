<script lang="ts">
  import { onMount } from 'svelte';
  import { activeTerminals, selectedTerminalIndex, showCommandPalette, showConnectionForm, editingConnection } from '../lib/store';
  import { execRemoteCommand } from '../lib/connectionService';
  import TerminalView from './TerminalView.svelte';
  import TerminalIcon from './icons/TerminalIcon.svelte';
  import PlusIcon from './icons/PlusIcon.svelte';
  import { formatTransferRate } from '../lib/transferRateFormatter';

  let currentTime = '';
  let netSpeedBps = 0;
  let cpuUsage = 0;
  let memPercent = 0;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let clockTimer: ReturnType<typeof setInterval> | null = null;
  let pollInFlight = false;
  let pollPending = false;
  let pollingEnabled = false;
  let pollVersion = 0;
  let lastSelectedSessionId: string | null = null;
  let lastClockMinute = '';
  const lastNetSampleBySession = new Map<string, { rx: number; tx: number; time: number }>();
  const lastCpuSampleBySession = new Map<string, { idle: number; total: number }>();
  const POLL_INTERVAL_MS = 15000;

  function openNewConnection() {
    editingConnection.set(null);
    showConnectionForm.set(true);
  }

  function openCommandPalette() {
    showCommandPalette.set(true);
  }

  function updateClock() {
    const next = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    if (next === lastClockMinute) return;
    lastClockMinute = next;
    currentTime = next;
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
      pollTimer = setInterval(() => void pollSelectedSession(false), POLL_INTERVAL_MS);
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

  function parseCpuUsage(output: string): number | null {
    const source = output.trim();
    if (!source) return null;

    const idleMatch = source.match(/(\d+(?:\.\d+)?)\s*id\b/i) ?? source.match(/(\d+(?:\.\d+)?)%\s*idle\b/i);
    if (!idleMatch) return null;
    const idle = parseFloat(idleMatch[1]);
    if (Number.isNaN(idle)) return null;
    return Math.max(0, Math.round((100 - idle) * 10) / 10);
  }

  function parseVmStatMemPercent(output: string): number | null {
    const pageSizeMatch = output.match(/page size of\s+(\d+)\s+bytes/i);
    const pageSize = pageSizeMatch ? parseInt(pageSizeMatch[1], 10) : 4096;
    if (!Number.isFinite(pageSize) || pageSize <= 0) return null;

    const readPages = (label: RegExp): number => {
      const match = output.match(label);
      if (!match) return 0;
      const value = parseInt(match[1].replace(/\./g, ''), 10);
      return Number.isFinite(value) ? value : 0;
    };

    const freePages = readPages(/Pages free:\s+([\d.]+)/i) + readPages(/Pages speculative:\s+([\d.]+)/i);
    const activePages = readPages(/Pages active:\s+([\d.]+)/i);
    const inactivePages = readPages(/Pages inactive:\s+([\d.]+)/i);
    const wiredPages = readPages(/Pages wired down:\s+([\d.]+)/i);
    const compressedPages = readPages(/Pages occupied by compressor:\s+([\d.]+)/i);
    const totalPages = freePages + activePages + inactivePages + wiredPages + compressedPages;
    if (totalPages <= 0) return null;

    const usedPages = Math.max(0, totalPages - freePages);
    return Math.max(0, Math.min(100, Math.round((usedPages / totalPages) * 100)));
  }

  function parseProcMeminfoPercent(output: string): number | null {
    const totalMatch = output.match(/^MemTotal:\s+(\d+)/m);
    const availableMatch = output.match(/^MemAvailable:\s+(\d+)/m);
    const freeMatch = output.match(/^MemFree:\s+(\d+)/m);
    const totalKb = totalMatch ? parseInt(totalMatch[1], 10) : NaN;
    if (!Number.isFinite(totalKb) || totalKb <= 0) return null;

    const availableKb = availableMatch
      ? parseInt(availableMatch[1], 10)
      : (freeMatch ? parseInt(freeMatch[1], 10) : NaN);
    if (!Number.isFinite(availableKb)) return null;

    const usedKb = Math.max(0, totalKb - availableKb);
    return Math.max(0, Math.min(100, Math.round((usedKb / totalKb) * 100)));
  }

  function parseMemPercent(output: string): number | null {
    const procMeminfoPercent = parseProcMeminfoPercent(output);
    if (procMeminfoPercent !== null) {
      return procMeminfoPercent;
    }

    const lines = output.split('\n');
    const memLine = lines.find(l => l.startsWith('Mem:'));
    if (memLine) {
      const parts = memLine.split(/\s+/);
      if (parts.length >= 4) {
        const total = parseInt(parts[1], 10);
        const used = parseInt(parts[2], 10);
        if (!Number.isNaN(total) && total > 0) {
          const value = Number.isNaN(used) ? 0 : used;
          let percent = Math.max(0, Math.min(100, Math.round((value / total) * 100)));
          if (parts.length >= 7) {
            const avail = parseInt(parts[6], 10);
            if (!Number.isNaN(avail)) {
              const adjustedUsed = total - avail;
              percent = Math.max(0, Math.min(100, Math.round((adjustedUsed / total) * 100)));
            }
          }
          return percent;
        }
      }
    }

    return parseVmStatMemPercent(output);
  }

  function parseProcStatCpuUsage(
    sessionId: string,
    output: string
  ): number | null {
    const line = output.trim();
    if (!line.startsWith('cpu ')) return null;

    const parts = line.split(/\s+/).slice(1).map(part => Number(part));
    if (parts.length < 4 || parts.some(part => !Number.isFinite(part))) {
      return null;
    }

    const idle = (parts[3] ?? 0) + (parts[4] ?? 0);
    const total = parts.reduce((sum, value) => sum + value, 0);
    const previous = lastCpuSampleBySession.get(sessionId);
    lastCpuSampleBySession.set(sessionId, { idle, total });

    if (!previous) {
      return null;
    }

    const deltaTotal = total - previous.total;
    const deltaIdle = idle - previous.idle;
    if (deltaTotal <= 0) {
      return null;
    }

    return Math.max(0, Math.min(100, Math.round((1 - deltaIdle / deltaTotal) * 1000) / 10));
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

  function getSelectedSessionId(): string | null {
    const terminal = $activeTerminals[$selectedTerminalIndex];
    return terminal ? terminal.sessionId : null;
  }

  function isSessionStillSelected(sessionId: string): boolean {
    return getSelectedSessionId() === sessionId;
  }

  async function pollSelectedSession(force = false) {
    if (!force && !pollingEnabled) return;
    if (pollInFlight) {
      pollPending = true;
      return;
    }
    const terminal = $activeTerminals[$selectedTerminalIndex];
    if (!terminal) {
      netSpeedBps = 0;
      cpuUsage = 0;
      memPercent = 0;
      lastNetSampleBySession.delete(lastSelectedSessionId ?? '');
      lastCpuSampleBySession.delete(lastSelectedSessionId ?? '');
      return;
    }

    const sessionId = terminal.sessionId;
    const version = ++pollVersion;
    pollInFlight = true;
    try {
      const combinedCommand = [
        '(cat /proc/net/dev 2>/dev/null || true)',
        'echo "---STAR_SHUTTLE_SPLIT---"',
        '(cat /proc/meminfo 2>/dev/null || free -m 2>/dev/null || vm_stat 2>/dev/null || true)',
        'echo "---STAR_SHUTTLE_SPLIT---"',
        '(head -n 1 /proc/stat 2>/dev/null || (top -bn1 | grep "Cpu(s)") 2>/dev/null || (top -l 1 | grep "CPU usage") 2>/dev/null || true)'
      ].join('; ');
      const output = await execRemoteCommand(sessionId, combinedCommand);
      if (version !== pollVersion || !isSessionStillSelected(sessionId)) return;

      const parts = output.split('---STAR_SHUTTLE_SPLIT---');
      if (parts.length >= 3) {
        const [netOut, memOut, cpuOut] = parts;

        const now = performance.now();
        const { rx, tx } = parseNetBytes(netOut);
        const last = lastNetSampleBySession.get(sessionId);
        if (last) {
          const dt = (now - last.time) / 1000;
          if (dt > 0) {
            const delta = Math.max(0, (rx - last.rx) + (tx - last.tx));
            netSpeedBps = delta / dt;
          }
        } else {
          netSpeedBps = 0;
        }
        lastNetSampleBySession.set(sessionId, { rx, tx, time: now });

        const parsedCpu = parseProcStatCpuUsage(sessionId, cpuOut) ?? parseCpuUsage(cpuOut);
        const parsedMem = parseMemPercent(memOut);
        cpuUsage = parsedCpu ?? 0;
        memPercent = parsedMem ?? 0;
      } else {
        netSpeedBps = 0;
        cpuUsage = 0;
        memPercent = 0;
        lastNetSampleBySession.delete(sessionId);
        lastCpuSampleBySession.delete(sessionId);
      }
    } catch {
      if (version !== pollVersion || !isSessionStillSelected(sessionId)) return;
      netSpeedBps = 0;
      cpuUsage = 0;
      memPercent = 0;
      lastNetSampleBySession.delete(sessionId);
      lastCpuSampleBySession.delete(sessionId);
    } finally {
      pollInFlight = false;
      if (pollPending) {
        pollPending = false;
        if (pollingEnabled) void pollSelectedSession();
      }
    }
  }

  $: {
    $activeTerminals;
    $selectedTerminalIndex;
    const selectedNow = getSelectedSessionId();
    if (selectedNow !== lastSelectedSessionId) {
      const previousSelectedSessionId = lastSelectedSessionId;
      lastSelectedSessionId = selectedNow;
      pollVersion += 1;
      netSpeedBps = 0;
      cpuUsage = 0;
      memPercent = 0;
      if (previousSelectedSessionId) {
        lastNetSampleBySession.delete(previousSelectedSessionId);
        lastCpuSampleBySession.delete(previousSelectedSessionId);
      }
      updatePollingState();
      if (pollingEnabled && selectedNow) {
        void pollSelectedSession(true);
      }
    } else {
      updatePollingState();
    }
  }

  onMount(() => {
    updateClock();
    clockTimer = setInterval(updateClock, 1000);
    updatePollingState();

    const onVisibilityChange = () => {
      if (updatePollingState()) void pollSelectedSession(true);
    };
    const onFocus = () => {
      if (updatePollingState()) void pollSelectedSession(true);
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
        <p class="text-sm mt-2 opacity-60">新建一个连接，或通过命令面板快速打开已保存的主机</p>
        <div class="mt-6 flex items-center gap-3">
          <button
            class="inline-flex items-center gap-2 rounded-lg bg-primary-600 px-4 py-2 text-sm font-medium text-white shadow-md transition-all hover:bg-primary-500"
            on:click={openNewConnection}
          >
            <PlusIcon class="w-4 h-4" />
            <span>新建连接</span>
          </button>
          <button
            class="inline-flex items-center gap-2 rounded-lg border border-app-border bg-app-bg px-4 py-2 text-sm font-medium text-app-text transition-colors hover:bg-app-bg-hover"
            on:click={openCommandPalette}
          >
            <svg class="w-4 h-4 text-app-text-secondary" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-4.35-4.35M10.5 18a7.5 7.5 0 100-15 7.5 7.5 0 000 15z" />
            </svg>
            <span>命令面板</span>
          </button>
        </div>
      </div>
    {:else}
      {#each rootTerminals as terminal (terminal.sessionId)}
        <TerminalView 
          terminalData={terminal} 
          isVisible={terminal.sessionId === $activeTerminals[$selectedTerminalIndex]?.sessionId || terminal.sessionId === $activeTerminals[$selectedTerminalIndex]?.parentId} 
        />
      {/each}
    {/if}

    <!-- 悬浮状态窗（模糊透明玻璃拟态） -->
    <div class="pointer-events-none absolute bottom-3 right-3 z-20 flex flex-col gap-1.5 rounded-xl border border-white/10 bg-app-surface/50 px-3.5 py-2.5 text-xs text-app-text-secondary shadow-lg backdrop-blur-md">
      <div class="flex items-center justify-between gap-3">
        <span class="flex items-center gap-1">
          <svg class="w-3.5 h-3.5 text-primary-500 dark:text-primary-400" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
            <path fill-rule="evenodd" d="M12.293 5.293a1 1 0 011.414 0l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414-1.414L14.586 11H3a1 1 0 110-2h11.586l-2.293-2.293a1 1 0 010-1.414z" clip-rule="evenodd"></path>
          </svg>
          实时流量
        </span>
        <span class="font-mono font-medium text-green-600 dark:text-green-300">{formatTransferRate(netSpeedBps)}</span>
      </div>
      <div class="flex items-center justify-between gap-3">
        <span>CPU</span>
        <span class="font-mono font-medium text-app-text">{cpuUsage}%</span>
      </div>
      <div class="flex items-center justify-between gap-3">
        <span>MEM</span>
        <span class="font-mono font-medium text-app-text">{memPercent}%</span>
      </div>
      <div class="flex items-center justify-between gap-3">
        <span>活动传输</span>
        <span class="text-app-text">{$activeTerminals.length}</span>
      </div>
      <div class="flex items-center justify-between gap-3">
        <span>时间</span>
        <span class="text-app-text">{currentTime}</span>
      </div>
    </div>
  </div>
</div>

<style>
  /* Custom scrollbar for terminal list */
</style>
