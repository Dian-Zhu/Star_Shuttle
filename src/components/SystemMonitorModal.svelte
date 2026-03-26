<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { fade } from 'svelte/transition';
  import XIcon from './icons/XIcon.svelte';
  import type { Connection } from '../lib/store';

  export let connection: Connection;
  export let sessionId: string;
  export let onClose: () => void;

  let loading = true;
  let error: string | null = null;
  let lastUpdated: Date | null = null;
  let intervalId: ReturnType<typeof setInterval> | null = null;
  let fetchInFlight = false;
  let fetchQueued = false;
  let fetchVersion = 0;
  let currentSessionId = '';
  let mounted = false;

  // Stats
  let stats = {
    uptime: '',
    loadAvg: [] as string[],
    cpuUsage: 0,
    memTotal: 0,
    memUsed: 0,
    memFree: 0,
    diskUsage: [] as { mount: string; size: string; used: string; avail: string; usePercent: string }[]
  };

  function resetStats() {
    stats = {
      uptime: '',
      loadAvg: [],
      cpuUsage: 0,
      memTotal: 0,
      memUsed: 0,
      memFree: 0,
      diskUsage: []
    };
  }

  async function execMonitorCommand(command: string): Promise<string> {
    try {
      const result = await invoke('exec_command', { sessionId, command });
      return typeof result === 'string' ? result : String(result ?? '');
    } catch {
      return '';
    }
  }

  // Helper to parse uptime output
  // " 14:32:01 up 10 days,  4:12,  1 user,  load average: 0.00, 0.01, 0.05"
  function parseUptime(output: string): boolean {
    stats.uptime = '';
    stats.loadAvg = [];
    const source = output.trim();
    if (!source) return false;
    try {
        const loadMatch = source.match(/load average[s]?:\s*([0-9.,\s]+)/i);
        if (loadMatch && loadMatch[1]) {
            const loads = loadMatch[1]
              .split(/[,\s]+/)
              .map(s => s.trim())
              .filter(Boolean)
              .slice(0, 3);
            if (loads.length > 0) {
              stats.loadAvg = loads;
            }
        }

        const upParts = source.split('up');
        if (upParts.length > 1) {
            const commaParts = upParts[1].split(',');
            if (commaParts.length > 0) {
                stats.uptime = commaParts[0].trim();
                if (commaParts.length > 1 && commaParts[1].includes(':')) {
                     stats.uptime += ', ' + commaParts[1].trim();
                }
            }
        }
    } catch (e) {
        console.error('Failed to parse uptime', e);
    }
    return Boolean(stats.uptime) || stats.loadAvg.length > 0;
  }

  // Helper to parse free -m
  //               total        used        free      shared  buff/cache   available
  // Mem:           7961        3672         256         236        4032        3788
  // Swap:          2047           0        2047
  function parseFree(output: string): boolean {
      stats.memTotal = 0;
      stats.memUsed = 0;
      stats.memFree = 0;
      if (!output.trim()) return false;
      try {
          const lines = output.split('\n');
          const memLine = lines.find(l => l.startsWith('Mem:'));
          if (memLine) {
              const parts = memLine.split(/\s+/);
              // parts[0] is "Mem:"
              if (parts.length >= 4) {
                  stats.memTotal = parseInt(parts[1]);
                  stats.memUsed = parseInt(parts[2]);
                  stats.memFree = parseInt(parts[3]);
                  
                  // Adjust for available if present (more accurate)
                  if (parts.length >= 7) {
                      const avail = parseInt(parts[6]);
                      stats.memUsed = stats.memTotal - avail;
                      stats.memFree = avail;
                  }
                  return stats.memTotal > 0;
              }
          }

          // macOS / BSD fallback: vm_stat output
          const pageSizeMatch = output.match(/page size of\s+(\d+)\s+bytes/i);
          const pageSize = pageSizeMatch ? parseInt(pageSizeMatch[1]) : 4096;
          const readPages = (label: RegExp): number => {
            const match = output.match(label);
            if (!match) return 0;
            return parseInt(match[1].replace(/\./g, ''), 10);
          };

          const freePages = readPages(/Pages free:\s+([\d.]+)/i) + readPages(/Pages speculative:\s+([\d.]+)/i);
          const activePages = readPages(/Pages active:\s+([\d.]+)/i);
          const inactivePages = readPages(/Pages inactive:\s+([\d.]+)/i);
          const wiredPages = readPages(/Pages wired down:\s+([\d.]+)/i);
          const compressedPages = readPages(/Pages occupied by compressor:\s+([\d.]+)/i);

          const totalPages = freePages + activePages + inactivePages + wiredPages + compressedPages;
          if (totalPages > 0 && Number.isFinite(pageSize)) {
            const totalBytes = totalPages * pageSize;
            const freeBytes = freePages * pageSize;
            const usedBytes = Math.max(0, totalBytes - freeBytes);
            stats.memTotal = Math.round(totalBytes / (1024 * 1024));
            stats.memUsed = Math.round(usedBytes / (1024 * 1024));
            stats.memFree = Math.round(freeBytes / (1024 * 1024));
            return stats.memTotal > 0;
          }
      } catch (e) {
          console.error('Failed to parse free', e);
      }
      return false;
  }

  // Helper to parse top/cpu
  // %Cpu(s):  0.3 us,  0.2 sy,  0.0 ni, 99.5 id,  0.0 wa,  0.0 hi,  0.0 si,  0.0 st
  function parseCpu(output: string): boolean {
      stats.cpuUsage = 0;
      if (!output.trim()) return false;
      try {
          // Try to match "id" (idle) percentage
          const match = output.match(/(\d+(?:\.\d+)?)\s*id\b/i) ?? output.match(/(\d+(?:\.\d+)?)%\s*idle\b/i);
          if (match) {
              const idle = parseFloat(match[1]);
              stats.cpuUsage = Math.max(0, Math.round((100 - idle) * 10) / 10);
              return true;
          } else {
              // Fallback if "id" not found, try to parse line
              const parts = output.split(',');
              for (const part of parts) {
                  if (part.includes('id')) {
                      const val = parseFloat(part.trim().split(' ')[0]);
                      if (!isNaN(val)) {
                          stats.cpuUsage = Math.max(0, Math.round((100 - val) * 10) / 10);
                          return true;
                      }
                  }
              }
          }
      } catch (e) {
          console.error('Failed to parse cpu', e);
      }
      return false;
  }

  // Helper to parse df -h
  // Filesystem      Size  Used Avail Use% Mounted on
  // /dev/sda1        20G   10G  9.0G  53% /
  function parseDf(output: string): boolean {
      stats.diskUsage = [];
      if (!output.trim()) return false;
      try {
          const lines = output.split('\n').slice(1); // Skip header
          stats.diskUsage = lines.filter(l => l.trim()).map(line => {
              const parts = line.split(/\s+/);
              if (parts.length >= 6) {
                  return {
                      mount: parts.slice(5).join(' '),
                      size: parts[1],
                      used: parts[2],
                      avail: parts[3],
                      usePercent: parts[4]
                  };
              }
              return null;
          }).filter(Boolean) as any;
      } catch (e) {
          console.error('Failed to parse df', e);
      }
      return stats.diskUsage.length > 0;
  }

  async function fetchData() {
      if (fetchInFlight) {
          fetchQueued = true;
          return;
      }
      fetchInFlight = true;
      const version = ++fetchVersion;
      const requestedSessionId = sessionId;
      try {
          // Run commands in parallel with per-command fallback to avoid whole-view failure.
          const [uptimeOut, freeOut, cpuOut, dfOut] = await Promise.all([
              execMonitorCommand('uptime 2>/dev/null || true'),
              execMonitorCommand('(free -m 2>/dev/null || vm_stat 2>/dev/null || true)'),
              execMonitorCommand('((top -bn1 | grep "Cpu(s)") 2>/dev/null || (top -l 1 | grep "CPU usage") 2>/dev/null || true)'),
              execMonitorCommand('(df -h 2>/dev/null | head -n 6 || true)')
          ]);

          if (version !== fetchVersion || requestedSessionId !== sessionId) {
            return;
          }

          const uptimeOk = parseUptime(uptimeOut);
          const freeOk = parseFree(freeOut);
          const cpuOk = parseCpu(cpuOut);
          const diskOk = parseDf(dfOut);
          const anyMetricAvailable = uptimeOk || freeOk || cpuOk || diskOk;

          if (!anyMetricAvailable) {
            error = '目标系统不支持当前监控命令，无法获取指标';
          } else if (!(uptimeOk && freeOk && cpuOk && diskOk)) {
            error = '部分监控命令在目标系统不可用，已降级展示可用指标';
          } else {
            error = null;
          }

          lastUpdated = new Date();
      } catch (e) {
          if (version === fetchVersion && requestedSessionId === sessionId) {
            console.error('Monitor fetch error:', e);
            resetStats();
            error = `获取数据失败: ${e}`;
          }
      } finally {
          if (version === fetchVersion && requestedSessionId === sessionId) {
            loading = false;
          }
          fetchInFlight = false;
          if (fetchQueued) {
            fetchQueued = false;
            void fetchData();
          }
      }
  }

  onMount(() => {
      mounted = true;
      currentSessionId = sessionId;
      fetchData();
      intervalId = setInterval(() => void fetchData(), 3000);
  });

  onDestroy(() => {
      mounted = false;
      if (intervalId) clearInterval(intervalId);
      fetchVersion += 1;
      fetchQueued = false;
      fetchInFlight = false;
  });

  $: if (mounted && sessionId !== currentSessionId) {
      currentSessionId = sessionId;
      fetchVersion += 1;
      fetchQueued = false;
      fetchInFlight = false;
      loading = true;
      error = null;
      lastUpdated = null;
      resetStats();
      if (intervalId) clearInterval(intervalId);
      intervalId = setInterval(() => void fetchData(), 3000);
      void fetchData();
  }

  // Calculate memory percentage
  $: memPercent = stats.memTotal > 0 ? Math.round((stats.memUsed / stats.memTotal) * 100) : 0;
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4" role="button" tabindex="0" on:click|self={onClose} on:keydown={(e) => e.key === 'Escape' && onClose()}>
  <div class="bg-app-surface border border-app-border rounded-xl shadow-2xl w-full max-w-4xl max-h-[90vh] flex flex-col overflow-hidden" transition:fade={{ duration: 200 }}>
    <!-- Header -->
    <div class="flex items-center justify-between px-6 py-4 border-b border-app-border bg-app-surface">
      <div class="flex items-center gap-3">
        <div class="w-2 h-2 rounded-full {error ? 'bg-red-500' : 'bg-green-500'} animate-pulse"></div>
        <h2 class="text-lg font-semibold text-app-text">系统监控 - {connection.name}</h2>
        {#if lastUpdated}
            <span class="text-xs text-app-text-secondary font-mono">更新于 {lastUpdated.toLocaleTimeString()}</span>
        {/if}
      </div>
      <button 
        class="text-app-text-secondary hover:text-app-text transition-colors p-1 rounded-md hover:bg-app-bg-hover"
        on:click={onClose}
      >
        <XIcon class="w-5 h-5" />
      </button>
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-y-auto p-6 custom-scrollbar bg-app-bg">
        {#if loading && !lastUpdated}
            <div class="flex flex-col items-center justify-center h-64 text-app-text-secondary">
                <div class="w-8 h-8 border-2 border-primary-500 border-t-transparent rounded-full animate-spin mb-2"></div>
                <p>正在连接并获取数据...</p>
            </div>
        {:else if error && !lastUpdated}
            <div class="flex flex-col items-center justify-center h-64 text-red-500 dark:text-red-400">
                <svg class="w-12 h-12 mb-2 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
                <p>{error}</p>
                <button class="mt-4 px-4 py-2 bg-app-surface rounded hover:bg-app-bg-hover text-app-text transition-colors" on:click={fetchData}>重试</button>
            </div>
        {:else}
            <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4 mb-6">
                <!-- CPU Card -->
                <div class="bg-app-surface border border-app-border rounded-xl p-4 flex flex-col relative overflow-hidden group hover:border-primary-500/50 transition-colors">
                    <h3 class="text-xs font-medium text-app-text-secondary uppercase tracking-wider mb-2">CPU 使用率</h3>
                    <div class="flex items-end gap-2 mt-auto z-10">
                        <span class="text-3xl font-bold text-app-text">{stats.cpuUsage}%</span>
                    </div>
                    <!-- Progress Bar -->
                    <div class="w-full bg-app-bg h-1.5 mt-3 rounded-full overflow-hidden">
                        <div class="h-full bg-primary-500 transition-all duration-500" style="width: {stats.cpuUsage}%"></div>
                    </div>
                </div>

                <!-- Memory Card -->
                <div class="bg-app-surface border border-app-border rounded-xl p-4 flex flex-col relative overflow-hidden group hover:border-purple-500/50 transition-colors">
                    <h3 class="text-xs font-medium text-app-text-secondary uppercase tracking-wider mb-2">内存使用</h3>
                    <div class="flex items-end gap-2 mt-auto z-10">
                        <span class="text-3xl font-bold text-app-text">{memPercent}%</span>
                        <span class="text-xs text-app-text-secondary mb-1">{stats.memUsed}M / {stats.memTotal}M</span>
                    </div>
                    <div class="w-full bg-app-bg h-1.5 mt-3 rounded-full overflow-hidden">
                        <div class="h-full bg-purple-500 transition-all duration-500" style="width: {memPercent}%"></div>
                    </div>
                </div>

                <!-- Load Average -->
                <div class="bg-app-surface border border-app-border rounded-xl p-4 flex flex-col hover:border-yellow-500/50 transition-colors">
                    <h3 class="text-xs font-medium text-app-text-secondary uppercase tracking-wider mb-2">平均负载</h3>
                    <div class="flex flex-col gap-1 mt-auto">
                        <div class="flex justify-between text-sm">
                            <span class="text-app-text-secondary">1 min</span>
                            <span class="text-app-text font-mono">{stats.loadAvg[0] || '-'}</span>
                        </div>
                        <div class="flex justify-between text-sm">
                            <span class="text-app-text-secondary">5 min</span>
                            <span class="text-app-text font-mono">{stats.loadAvg[1] || '-'}</span>
                        </div>
                        <div class="flex justify-between text-sm">
                            <span class="text-app-text-secondary">15 min</span>
                            <span class="text-app-text font-mono">{stats.loadAvg[2] || '-'}</span>
                        </div>
                    </div>
                </div>

                <!-- Uptime -->
                <div class="bg-app-surface border border-app-border rounded-xl p-4 flex flex-col hover:border-green-500/50 transition-colors">
                    <h3 class="text-xs font-medium text-app-text-secondary uppercase tracking-wider mb-2">运行时间</h3>
                    <div class="flex items-center h-full">
                        <span class="text-xl font-medium text-app-text">{stats.uptime || '-'}</span>
                    </div>
                </div>
            </div>

            <!-- Disk Usage -->
            <div class="bg-app-surface border border-app-border rounded-xl p-6">
                <h3 class="text-sm font-medium text-app-text mb-4 flex items-center gap-2">
                    <svg class="w-4 h-4 text-app-text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20 13V6a2 2 0 00-2-2H6a2 2 0 00-2 2v7m16 0v5a2 2 0 01-2 2H6a2 2 0 01-2-2v-5m16 0h-2.586a1 1 0 00-.707.293l-2.414 2.414a1 1 0 01-.707.293h-3.172a1 1 0 01-.707-.293l-2.414-2.414A1 1 0 006.586 13H4"></path></svg>
                    磁盘使用情况
                </h3>
                <div class="overflow-x-auto">
                    <table class="w-full text-left text-sm">
                        <thead>
                            <tr class="text-app-text-secondary border-b border-app-border">
                                <th class="pb-2 font-medium">挂载点</th>
                                <th class="pb-2 font-medium">总大小</th>
                                <th class="pb-2 font-medium">已用</th>
                                <th class="pb-2 font-medium">可用</th>
                                <th class="pb-2 font-medium w-32">使用率</th>
                            </tr>
                        </thead>
                        <tbody class="divide-y divide-app-border/50">
                            {#each stats.diskUsage as disk}
                                <tr class="group hover:bg-app-bg-hover transition-colors">
                                    <td class="py-3 font-mono text-app-text">{disk.mount}</td>
                                    <td class="py-3 text-app-text-secondary">{disk.size}</td>
                                    <td class="py-3 text-app-text-secondary">{disk.used}</td>
                                    <td class="py-3 text-app-text-secondary">{disk.avail}</td>
                                    <td class="py-3">
                                        <div class="flex items-center gap-2">
                                            <div class="flex-1 h-1.5 bg-app-bg rounded-full overflow-hidden">
                                                <div 
                                                    class="h-full rounded-full {parseInt(disk.usePercent) > 90 ? 'bg-red-500' : parseInt(disk.usePercent) > 70 ? 'bg-yellow-500' : 'bg-green-500'}" 
                                                    style="width: {disk.usePercent}"
                                                ></div>
                                            </div>
                                            <span class="text-xs text-app-text-secondary w-8 text-right">{disk.usePercent}</span>
                                        </div>
                                    </td>
                                </tr>
                            {/each}
                        </tbody>
                    </table>
                </div>
            </div>
        {/if}
    </div>
  </div>
</div>

<style>
  .custom-scrollbar::-webkit-scrollbar {
    width: 6px;
  }
  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: var(--color-border);
    border-radius: 3px;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: var(--color-border-light);
  }
</style>
