<script lang="ts">
  import { showAdvancedModal } from '../lib/store';
  import XIcon from './icons/XIcon.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { save } from '@tauri-apps/plugin-dialog';
  import { writeTextFile } from '@tauri-apps/plugin-fs';
  import { onMount } from 'svelte';

  type LogEntry = {
    timestamp?: string;
    level?: string;
    module?: string;
    message?: string;
    file?: string;
    line?: number;
    raw: string;
  };

  let activeTab: 'logs' | 'tunnels' = 'logs';
  let rawLogs: string[] = [];
  let logs: LogEntry[] = [];
  let logFilePath: string | null = null;
  let loading = false;
  let error = '';
  let filter = '';
  let toast = '';
  let toastTimer: ReturnType<typeof setTimeout> | null = null;

  function setToast(message: string) {
    toast = message;
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = ''), 2500);
  }

  function parseLogLine(line: string): LogEntry {
    try {
      const parsed = JSON.parse(line);
      if (parsed && typeof parsed === 'object') {
        return { ...parsed, raw: line } as LogEntry;
      }
      return { raw: line };
    } catch {
      return { raw: line };
    }
  }

  async function refreshLogs() {
    loading = true;
    error = '';
    try {
      const [lines, path] = await Promise.all([
        invoke<string[]>('get_logs'),
        invoke<string | null>('get_log_file_path')
      ]);
      rawLogs = lines ?? [];
      logFilePath = path ?? null;
      logs = rawLogs.map(parseLogLine).reverse();
    } catch (e) {
      error = `加载日志失败: ${e}`;
    } finally {
      loading = false;
    }
  }

  async function clearLogs() {
    try {
      await invoke('clear_logs');
      await refreshLogs();
      setToast('已清空日志');
    } catch (e) {
      error = `清空日志失败: ${e}`;
    }
  }

  async function exportLogs() {
    try {
      const filePath = await save({
        filters: [{ name: 'JSONL', extensions: ['jsonl', 'log'] }],
        defaultPath: 'app.log.jsonl'
      });
      if (!filePath) return;
      const content = rawLogs.join('\n');
      await writeTextFile(filePath, content);
      setToast('日志已导出');
    } catch (e) {
      error = `导出日志失败: ${e}`;
    }
  }

  $: filteredLogs = filter.trim()
    ? logs.filter(l => {
        const q = filter.trim().toLowerCase();
        return (
          (l.timestamp ?? '').toLowerCase().includes(q) ||
          (l.level ?? '').toLowerCase().includes(q) ||
          (l.module ?? '').toLowerCase().includes(q) ||
          (l.message ?? '').toLowerCase().includes(q) ||
          l.raw.toLowerCase().includes(q)
        );
      })
    : logs;

  onMount(() => {
    refreshLogs();
    return () => {
      if (toastTimer) clearTimeout(toastTimer);
    };
  });

  function handleClose() {
    showAdvancedModal.set(false);
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-app-backdrop backdrop-blur-sm p-4" role="button" tabindex="0" on:click|self={handleClose} on:keydown={(e) => e.key === 'Escape' && handleClose()}>
  <div class="bg-app-surface border border-app-border rounded-xl shadow-2xl w-full max-w-3xl h-[600px] flex flex-col overflow-hidden">
    <!-- Header -->
    <div class="flex items-center justify-between px-6 py-4 border-b border-app-border bg-app-bg">
      <h2 class="text-lg font-semibold text-app-text">高级功能</h2>
      <button 
        class="text-app-text-secondary hover:text-app-text transition-colors p-1 rounded-md hover:bg-app-bg-hover"
        on:click={handleClose}
      >
        <XIcon class="w-5 h-5" />
      </button>
    </div>

    <!-- Content -->
    <div class="flex-1 flex flex-col overflow-hidden bg-app-bg/50">
      <div class="px-6 pt-4 flex items-center gap-2 border-b border-app-border">
        <button
          class="px-3 py-2 text-sm rounded-t-lg border border-b-0 transition-colors {activeTab === 'logs' ? 'bg-app-surface border-app-border text-primary-600 dark:text-primary-400 font-medium' : 'bg-transparent border-transparent text-app-text-secondary hover:text-app-text'}"
          on:click={() => (activeTab = 'logs')}
        >
          日志
        </button>
        <button
          class="px-3 py-2 text-sm rounded-t-lg border border-b-0 transition-colors {activeTab === 'tunnels' ? 'bg-app-surface border-app-border text-primary-600 dark:text-primary-400 font-medium' : 'bg-transparent border-transparent text-app-text-secondary hover:text-app-text'}"
          on:click={() => (activeTab = 'tunnels')}
        >
          隧道
        </button>
      </div>

      {#if activeTab === 'logs'}
        <div class="px-6 py-4 border-b border-app-border flex items-center gap-3">
          <input
            type="text"
            bind:value={filter}
            placeholder="过滤（level/module/message）..."
            class="flex-1 bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text placeholder-app-text-secondary/50 focus:outline-none focus:border-primary-500/60 focus:ring-1 focus:ring-primary-500/30"
          />
          <button
            class="px-3 py-2 rounded-lg bg-app-surface border border-app-border hover:bg-app-bg-hover text-app-text-secondary hover:text-app-text text-sm transition-colors"
            on:click={refreshLogs}
            disabled={loading}
          >
            刷新
          </button>
          <button
            class="px-3 py-2 rounded-lg bg-app-surface border border-app-border hover:bg-app-bg-hover text-app-text-secondary hover:text-app-text text-sm transition-colors"
            on:click={exportLogs}
            disabled={loading || rawLogs.length === 0}
          >
            导出
          </button>
          <button
            class="px-3 py-2 rounded-lg bg-red-50 dark:bg-red-900/20 hover:bg-red-100 dark:hover:bg-red-900/30 text-red-600 dark:text-red-300 text-sm transition-colors border border-red-200 dark:border-red-500/20"
            on:click={clearLogs}
            disabled={loading || rawLogs.length === 0}
          >
            清空
          </button>
        </div>

        <div class="px-6 py-3 text-xs text-app-text-secondary flex items-center justify-between gap-4">
          <div class="truncate">
            {#if logFilePath}
              文件：{logFilePath}
            {:else}
              文件：未初始化或不可用
            {/if}
          </div>
          <div class="shrink-0">
            {filteredLogs.length} 条
          </div>
        </div>

        {#if toast}
          <div class="px-6 pb-2">
            <div class="inline-flex items-center px-3 py-2 rounded-lg bg-green-500/10 border border-green-500/20 text-green-300 text-sm">
              {toast}
            </div>
          </div>
        {/if}

        {#if error}
          <div class="px-6 pb-2">
            <div class="inline-flex items-center px-3 py-2 rounded-lg bg-red-500/10 border border-red-500/20 text-red-300 text-sm">
              {error}
            </div>
          </div>
        {/if}

        <div class="flex-1 overflow-y-auto px-6 pb-6 custom-scrollbar">
          {#if loading && logs.length === 0}
            <div class="h-full flex items-center justify-center text-app-text-secondary">
              加载中...
            </div>
          {:else if filteredLogs.length === 0}
            <div class="h-full flex items-center justify-center text-app-text-secondary">
              暂无日志
            </div>
          {:else}
            <div class="space-y-2">
              {#each filteredLogs as l (l.raw)}
                <div class="bg-app-surface border border-app-border rounded-lg p-3">
                  <div class="flex items-center justify-between gap-3">
                    <div class="text-xs text-app-text-secondary font-mono truncate">
                      {l.timestamp ?? 'unknown'}
                      {#if l.level} [{l.level}]{/if}
                      {#if l.module} [{l.module}]{/if}
                    </div>
                    {#if l.file}
                      <div class="text-xs text-app-text-secondary/70 font-mono truncate">
                        {l.file}{#if typeof l.line === 'number'}:{l.line}{/if}
                      </div>
                    {/if}
                  </div>
                  <div class="mt-2 text-sm text-app-text whitespace-pre-wrap break-words">
                    {l.message ?? l.raw}
                  </div>
                </div>
              {/each}
            </div>
          {/if}
        </div>
      {:else}
        <div class="flex-1 p-6 flex items-center justify-center text-app-text-secondary">
          <div class="text-center">
            <svg class="w-16 h-16 mx-auto mb-4 opacity-50" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19.428 15.428a2 2 0 00-1.022-.547l-2.384-.477a6 6 0 00-3.86.517l-.318.158a6 6 0 01-3.86.517L6.05 15.21a2 2 0 00-1.806.547M8 4h8l-1 1v5.172a2 2 0 00.586 1.414l5 5c1.26 1.26.367 3.414-1.415 3.414H4.828c-1.782 0-2.674-2.154-1.414-3.414l5-5A2 2 0 009 10.172V5L8 4z"></path></svg>
            <h3 class="text-lg font-medium text-app-text mb-2">隧道管理开发中</h3>
            <p class="max-w-md mx-auto">端口转发、跳板机与代理能力将在此处提供。</p>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
