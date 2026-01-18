<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { activeTerminals, selectedTerminalIndex, broadcastInputEnabled, broadcastSessionIds } from '../lib/store';
  import { closeTerminal } from '../lib/terminalService';
  import TerminalView from './TerminalView.svelte';
  import XIcon from './icons/XIcon.svelte';
  import TerminalIcon from './icons/TerminalIcon.svelte';
  import { formatSpeed } from '../lib/transferQueueService';

  $: selectedBroadcastSet = new Set($broadcastSessionIds);
  let currentTime = '';
  let netSpeedBps = 0;
  let cpuUsage = 0;
  let memPercent = 0;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let clockTimer: ReturnType<typeof setInterval> | null = null;
  const lastNetSampleBySession = new Map<string, { rx: number; tx: number; time: number }>();

  function updateClock() {
    currentTime = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
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
    const terminal = $activeTerminals[$selectedTerminalIndex];
    if (!terminal) {
      netSpeedBps = 0;
      cpuUsage = 0;
      memPercent = 0;
      return;
    }

    const sessionId = terminal.sessionId;
    try {
      const [netOut, cpuOut, memOut] = await Promise.all([
        invoke('exec_command', { sessionId, command: 'cat /proc/net/dev' }),
        invoke('exec_command', { sessionId, command: 'top -bn1 | grep "Cpu(s)"' }),
        invoke('exec_command', { sessionId, command: 'free -m' }),
      ]);

      const now = performance.now();
      const { rx, tx } = parseNetBytes(netOut as string);
      const last = lastNetSampleBySession.get(sessionId);
      if (last) {
        const dt = (now - last.time) / 1000;
        if (dt > 0) {
          const delta = Math.max(0, (rx - last.rx) + (tx - last.tx));
          netSpeedBps = delta / dt;
        }
      }
      lastNetSampleBySession.set(sessionId, { rx, tx, time: now });

      parseCpuUsage(cpuOut as string);
      parseMemPercent(memOut as string);
    } catch {
      netSpeedBps = 0;
      cpuUsage = 0;
      memPercent = 0;
      lastNetSampleBySession.delete(sessionId);
    }
  }

  onMount(() => {
    updateClock();
    clockTimer = setInterval(updateClock, 1000);
    pollSelectedSession();
    pollTimer = setInterval(pollSelectedSession, 2000);
  });

  onDestroy(() => {
    if (clockTimer) clearInterval(clockTimer);
    if (pollTimer) clearInterval(pollTimer);
  });

  function handleTabClick(index: number, event: MouseEvent) {
    const terminal = $activeTerminals[index];
    if (!terminal) return;

    if ($broadcastInputEnabled && (event.ctrlKey || event.metaKey)) {
      const sessionId = terminal.sessionId;
      if (selectedBroadcastSet.has(sessionId)) {
        broadcastSessionIds.update(ids => ids.filter(id => id !== sessionId));
      } else {
        broadcastSessionIds.update(ids => [...ids, sessionId]);
      }
      return;
    }

    $selectedTerminalIndex = index;
  }

  function handleClose(sessionId: string, event: MouseEvent) {
    event.stopPropagation();
    closeTerminal(sessionId);
  }

  function toggleBroadcast() {
    broadcastInputEnabled.update(v => {
      const next = !v;
      if (!next) {
        broadcastSessionIds.set([]);
        return next;
      }

      const current = $activeTerminals[$selectedTerminalIndex];
      if (current) {
        broadcastSessionIds.set([current.sessionId]);
      }
      return next;
    });
  }
</script>

<div class="flex flex-col h-full w-full bg-white dark:bg-slate-950">
  <!-- Tabs Bar -->
  {#if $activeTerminals.length > 0}
    <div class="flex bg-slate-100 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 overflow-x-auto no-scrollbar">
      <div class="flex items-center px-2 gap-2 border-r border-slate-200 dark:border-slate-800">
        <button
          class="px-2 py-1 text-xs rounded-md transition-colors border
          {$broadcastInputEnabled
            ? 'bg-blue-600 border-blue-600 text-white'
            : 'bg-white/60 dark:bg-slate-950/60 border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800'}"
          on:click={toggleBroadcast}
          type="button"
          title="广播输入：Ctrl/⌘ 点击 Tab 选择多个会话"
        >
          广播{#if $broadcastInputEnabled}（{Math.max($broadcastSessionIds.length, 1)}）{/if}
        </button>
      </div>
      {#each $activeTerminals as terminal, index}
        <button
          class="group flex items-center gap-2 px-4 py-2.5 min-w-[160px] max-w-[240px] text-sm border-r border-slate-200 dark:border-slate-800 transition-colors relative
          {index === $selectedTerminalIndex 
            ? 'bg-white dark:bg-slate-950 text-blue-600 dark:text-blue-400 font-medium' 
            : 'bg-slate-100 dark:bg-slate-900 text-slate-500 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-800 hover:text-slate-700 dark:hover:text-slate-200'}"
          on:click={(e) => handleTabClick(index, e)}
        >
          {#if index === $selectedTerminalIndex}
            <div class="absolute top-0 left-0 right-0 h-0.5 bg-blue-500"></div>
          {/if}

          {#if $broadcastInputEnabled && selectedBroadcastSet.has(terminal.sessionId)}
            <div class="absolute inset-y-1 left-1 w-1 rounded bg-blue-500"></div>
          {/if}
          
          <TerminalIcon class="w-4 h-4 opacity-70" />
          <span class="truncate flex-1 text-left">{terminal.connection.name}</span>
          
          <span
            role="button"
            tabindex="0"
            class="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-400 hover:text-red-500 dark:hover:text-red-400 transition-all"
            on:click={(e) => handleClose(terminal.sessionId, e)}
            on:keydown={(e) => e.key === 'Enter' && handleClose(terminal.sessionId, e as any)}
          >
            <XIcon class="w-3.5 h-3.5" />
          </span>
        </button>
      {/each}
    </div>
  {/if}

  <!-- Terminal Content Area -->
  <div class="flex-1 relative overflow-hidden">
    {#if $activeTerminals.length === 0}
      <div class="absolute inset-0 flex flex-col items-center justify-center text-slate-400 dark:text-slate-500 bg-slate-50/50 dark:bg-slate-950/50">
        <div class="w-16 h-16 mb-4 rounded-2xl bg-white dark:bg-slate-900 flex items-center justify-center border border-slate-200 dark:border-slate-800 shadow-sm">
          <TerminalIcon class="w-8 h-8 opacity-50" />
        </div>
        <p class="text-lg font-medium text-slate-600 dark:text-slate-400">无活动会话</p>
        <p class="text-sm mt-2 opacity-60">请从左侧列表选择连接以开始</p>
      </div>
    {:else}
      {#each $activeTerminals as terminal, index (terminal.sessionId)}
        <TerminalView 
          terminalData={terminal} 
          isVisible={index === $selectedTerminalIndex} 
        />
      {/each}
    {/if}
  </div>

  <!-- Status Bar -->
  <div class="flex items-center justify-between px-4 py-1.5 bg-slate-800/80 dark:bg-slate-900/90 border-t border-slate-700/50 dark:border-slate-800/50 text-xs text-slate-300 dark:text-slate-400">
    <div class="flex items-center gap-4">
      <div class="flex items-center gap-1">
        <svg class="w-3.5 h-3.5 text-blue-400" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
          <path fill-rule="evenodd" d="M12.293 5.293a1 1 0 011.414 0l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414-1.414L14.586 11H3a1 1 0 110-2h11.586l-2.293-2.293a1 1 0 010-1.414z" clip-rule="evenodd"></path>
        </svg>
        <span>实时流量:</span>
        <span class="font-mono font-medium text-green-300">{formatSpeed(netSpeedBps)}</span>
        <span class="ml-2 text-slate-500">CPU</span>
        <span class="font-mono font-medium text-slate-200">{cpuUsage}%</span>
        <span class="ml-2 text-slate-500">MEM</span>
        <span class="font-mono font-medium text-slate-200">{memPercent}%</span>
      </div>
      <div class="text-slate-500">|</div>
      <div class="text-slate-500">
        活动传输: <span class="text-slate-300">{$activeTerminals.length}</span>
      </div>
    </div>
    <div class="text-slate-500 text-xs">
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
