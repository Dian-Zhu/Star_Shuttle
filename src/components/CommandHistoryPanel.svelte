<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { get } from 'svelte/store';
  import { showCommandHistory, activeTerminals, selectedTerminalIndex } from '../lib/store';
  import { commandHistoryService } from '../lib/commandHistoryService';
  import { handleTerminalInputSingle } from '../lib/terminalService';
  import type { CommandHistoryEntry } from '../types';

  let inputElement: HTMLInputElement;
  let searchTerm = '';
  let selectedIndex = 0;
  let container: HTMLDivElement;
  let entries: CommandHistoryEntry[] = [];
  let loading = true;

  const NO_CONNECTION_LABEL = '未关联连接';

  $: filtered = (() => {
    const q = searchTerm.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter(
      e =>
        e.command.toLowerCase().includes(q) ||
        (e.connection_name && e.connection_name.toLowerCase().includes(q))
    );
  })();

  // 按连接机器分组：每台机器的命令归到自己的分组，没有关联连接的命令单独成组。
  // 分组按“最近使用过的机器”排在前，未关联连接分组始终排在最后。
  // 组内保持后端返回的时间倒序。
  $: groups = (() => {
    const map = new Map<string, CommandHistoryEntry[]>();
    for (const entry of filtered) {
      const key = entry.connection_name?.trim() || NO_CONNECTION_LABEL;
      const list = map.get(key);
      if (list) list.push(entry);
      else map.set(key, [entry]);
    }
    const result = Array.from(map.entries()).map(([name, items]) => ({ name, items }));
    // filtered 已按时间倒序，Map 的插入顺序即“最近使用的机器优先”；
    // 仅把未关联分组挪到末尾。
    result.sort((a, b) => {
      if (a.name === NO_CONNECTION_LABEL) return 1;
      if (b.name === NO_CONNECTION_LABEL) return -1;
      return 0;
    });
    return result;
  })();

  // 分组渲染时的扁平索引：键盘导航仍作用于 filtered 的线性下标，
  // 因此需要把每条命令映射到它在 filtered 中的全局位置。
  $: flatOrder = groups.flatMap(g => g.items);

  function flatIndexOf(entry: CommandHistoryEntry): number {
    return flatOrder.indexOf(entry);
  }

  // 搜索词变化时把选中项复位到第一条
  $: if (searchTerm !== undefined) selectedIndex = 0;

  async function loadHistory() {
    loading = true;
    try {
      entries = await commandHistoryService.getRecent(500);
    } catch {
      entries = [];
    } finally {
      loading = false;
    }
  }

  onMount(async () => {
    await loadHistory();
    await tick();
    inputElement?.focus();
  });

  function close() {
    showCommandHistory.set(false);
  }

  function hasActiveTerminal(): boolean {
    const terminal = get(activeTerminals)[get(selectedTerminalIndex)];
    return !!terminal;
  }

  // 填充命令到当前终端（不执行）；execute=true 时追加回车立即执行。
  function useCommand(entry: CommandHistoryEntry | undefined, execute: boolean) {
    if (!entry) return;
    const terminal = get(activeTerminals)[get(selectedTerminalIndex)];
    if (!terminal) return;
    handleTerminalInputSingle(terminal.sessionId, execute ? `${entry.command}\r` : entry.command);
    close();
  }

  async function deleteEntry(entry: CommandHistoryEntry, e: MouseEvent) {
    e.stopPropagation();
    try {
      await commandHistoryService.delete(entry.id);
      entries = entries.filter(x => x.id !== entry.id);
    } catch {
      // ignore
    }
  }

  async function clearAll() {
    try {
      await commandHistoryService.clear();
      entries = [];
    } catch {
      // ignore
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (flatOrder.length === 0) return;
      selectedIndex = (selectedIndex + 1) % flatOrder.length;
      scrollToSelected();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (flatOrder.length === 0) return;
      selectedIndex = (selectedIndex - 1 + flatOrder.length) % flatOrder.length;
      scrollToSelected();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      // Ctrl/Cmd+Enter 立即执行，否则仅填充到终端
      useCommand(flatOrder[selectedIndex], e.ctrlKey || e.metaKey);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  }

  function scrollToSelected() {
    const item = document.getElementById(`history-item-${selectedIndex}`);
    item?.scrollIntoView({ block: 'nearest' });
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === container) close();
  }

  function formatTime(seconds: number): string {
    const d = new Date(seconds * 1000);
    return d.toLocaleString();
  }
</script>

<div
  bind:this={container}
  class="fixed inset-0 z-50 flex items-start justify-center pt-[15vh] bg-black/20 dark:bg-black/50 backdrop-blur-sm transition-all"
  on:click={handleBackdropClick}
  on:keydown={() => {}}
  role="presentation"
  transition:fade={{ duration: 150 }}
>
  <div
    class="w-full max-w-2xl bg-app-surface rounded-xl shadow-2xl border border-app-border flex flex-col overflow-hidden max-h-[60vh]"
    transition:fly={{ y: -20, duration: 200 }}
  >
    <!-- Search Input -->
    <div class="p-3 border-b border-app-border flex items-center gap-3">
      <svg class="w-5 h-5 text-app-text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"></path>
      </svg>
      <input
        bind:this={inputElement}
        bind:value={searchTerm}
        on:keydown={handleKeydown}
        type="text"
        placeholder="搜索历史命令..."
        class="flex-1 bg-transparent border-none outline-none text-app-text text-lg placeholder-app-text-secondary/50"
        autocomplete="off"
      />
      {#if entries.length > 0}
        <button
          class="text-xs px-2 py-1 rounded text-app-text-secondary hover:text-red-500 border border-app-border hover:border-red-500/50 transition-colors whitespace-nowrap"
          on:click={clearAll}
          title="清空全部历史"
        >
          清空
        </button>
      {/if}
      <kbd class="px-2 py-1 bg-app-bg rounded text-xs text-app-text-secondary border border-app-border">ESC</kbd>
    </div>

    <!-- History List -->
    <div id="history-list" class="flex-1 overflow-y-auto p-2 scrollbar-thin scrollbar-thumb-app-border">
      {#if loading}
        <div class="p-8 text-center text-app-text-secondary">加载中...</div>
      {:else if flatOrder.length === 0}
        <div class="p-8 text-center text-app-text-secondary">
          <p>{entries.length === 0 ? '暂无历史命令' : '未找到匹配的命令'}</p>
        </div>
      {:else}
        {#each groups as group (group.name)}
          <div class="mb-2">
            <div class="sticky top-0 z-10 flex items-center gap-2 px-2 py-1 bg-app-surface/95 backdrop-blur-sm">
              <span class="text-xs font-semibold text-app-text-secondary uppercase tracking-wide truncate">
                {group.name}
              </span>
              <span class="text-xs text-app-text-secondary/70">({group.items.length})</span>
              <div class="flex-1 border-t border-app-border/60"></div>
            </div>
            {#each group.items as entry (entry.id)}
              {@const index = flatIndexOf(entry)}
              <button
                id="history-item-{index}"
                class="w-full text-left px-4 py-2.5 rounded-lg flex items-center justify-between group transition-colors {index === selectedIndex ? 'bg-primary-50 dark:bg-primary-900/20' : 'hover:bg-app-bg-hover'}"
                on:click={() => useCommand(entry, false)}
                on:dblclick={() => useCommand(entry, true)}
                on:mousemove={() => (selectedIndex = index)}
              >
                <div class="flex flex-col overflow-hidden min-w-0 flex-1">
                  <span class="font-mono text-sm text-app-text truncate {index === selectedIndex ? 'text-primary-700 dark:text-primary-300' : ''}">
                    {entry.command}
                  </span>
                  <span class="text-xs text-app-text-secondary truncate">
                    {formatTime(entry.executed_at)}
                  </span>
                </div>
                <span
                  class="ml-2 shrink-0 opacity-0 group-hover:opacity-100 text-app-text-secondary hover:text-red-500 transition-opacity"
                  role="button"
                  tabindex="-1"
                  title="删除此条"
                  on:click={(e) => deleteEntry(entry, e)}
                  on:keydown={() => {}}
                >
                  <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
                  </svg>
                </span>
              </button>
            {/each}
          </div>
        {/each}
      {/if}
    </div>

    <div class="px-4 py-2 bg-app-bg border-t border-app-border text-xs text-app-text-secondary flex justify-between">
      <div class="flex gap-4">
        <span><kbd>↑</kbd> <kbd>↓</kbd> 导航</span>
        <span><kbd>Enter</kbd> 填充到终端</span>
        <span><kbd>Ctrl+Enter</kbd> 填充并执行</span>
      </div>
      <div>
        {#if !hasActiveTerminal()}
          <span class="text-amber-500">无活动终端</span>
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  kbd {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  }
</style>
