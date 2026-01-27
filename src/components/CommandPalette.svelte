<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { 
    showCommandPalette, 
    showConnectionForm, 
    showSettings, 
    connections, 
    editingConnection,
    isSidebarCollapsed,
    settings as appSettings
  } from '../lib/store';
  import { connectAndOpen } from '../lib/terminalService';
  import { importConnections, exportConnections } from '../lib/importExportService';
  
  import ServerIcon from './icons/ServerIcon.svelte';
  import SettingsIcon from './icons/SettingsIcon.svelte';
  import PlusIcon from './icons/PlusIcon.svelte';
  import UploadIcon from './icons/UploadIcon.svelte';
  import DownloadIcon from './icons/DownloadIcon.svelte';

  interface Command {
    id: string;
    title: string;
    description?: string;
    category: 'General' | 'Connection' | 'Settings' | 'Data';
    icon?: any;
    action: () => void;
    shortcut?: string;
  }

  let inputElement: HTMLInputElement;
  let searchTerm = '';
  let selectedIndex = 0;
  let container: HTMLDivElement;

  // Base commands
  let baseCommands: Command[] = [];

  $: baseCommands = [
    {
      id: 'new-connection',
      title: '新建连接',
      description: '创建一个新的 SSH 连接配置',
      category: 'General',
      icon: PlusIcon,
      action: () => {
        editingConnection.set(null);
        showConnectionForm.set(true);
      },
      shortcut: $appSettings.shortcuts.newConnection
    },
    {
      id: 'toggle-sidebar',
      title: '切换侧边栏',
      description: '显示或隐藏侧边栏',
      category: 'General',
      action: () => {
        isSidebarCollapsed.update(v => !v);
      },
      shortcut: $appSettings.shortcuts.toggleSidebar
    },
    {
      id: 'open-settings',
      title: '设置',
      description: '打开应用设置',
      category: 'Settings',
      icon: SettingsIcon,
      action: () => {
        showSettings.set(true);
      },
      shortcut: $appSettings.shortcuts.settings
    },
    {
      id: 'import-config',
      title: '导入配置',
      description: '从 JSON 文件导入连接配置',
      category: 'Data',
      icon: UploadIcon,
      action: () => {
        importConnections();
      }
    },
    {
      id: 'export-config',
      title: '导出配置',
      description: '导出为 JSON（默认不含密码/口令）',
      category: 'Data',
      icon: DownloadIcon,
      action: () => {
        exportConnections();
      }
    },
    {
      id: 'export-config-with-secrets',
      title: '导出配置（含敏感信息）',
      description: '导出为 JSON（包含明文密码/口令）',
      category: 'Data',
      icon: DownloadIcon,
      action: () => {
        exportConnections({ includeSensitive: true });
      }
    }
  ];

  // Derived commands combining base commands and connections
  $: connectionCommands = $connections.map(conn => ({
    id: `connect-${conn.id}`,
    title: `连接: ${conn.name}`,
    description: `${conn.username}@${conn.host}:${conn.port}`,
    category: 'Connection' as const,
    icon: ServerIcon,
    action: () => {
      connectAndOpen(conn);
    },
    shortcut: undefined
  }));

  $: allCommands = [...baseCommands, ...connectionCommands];

  $: filteredCommands = allCommands.filter(cmd => {
    const term = searchTerm.toLowerCase();
    return (
      cmd.title.toLowerCase().includes(term) ||
      (cmd.description && cmd.description.toLowerCase().includes(term)) ||
      cmd.category.toLowerCase().includes(term)
    );
  });

  // Reset selection when search changes
  $: {
    searchTerm;
    selectedIndex = 0;
  }

  onMount(async () => {
    await tick();
    inputElement?.focus();
  });

  function close() {
    showCommandPalette.set(false);
  }

  function checkShortcut(event: KeyboardEvent, shortcut: string): boolean {
    if (!shortcut) return false;
    const parts = shortcut.toLowerCase().split('+');
    const key = parts.pop();
    if (!key) return false;

    const ctrl = parts.includes('ctrl') || parts.includes('control');
    const shift = parts.includes('shift');
    const alt = parts.includes('alt') || parts.includes('option');
    const meta = parts.includes('meta') || parts.includes('cmd') || parts.includes('command');

    if (ctrl && !event.ctrlKey) return false;
    if (shift && !event.shiftKey) return false;
    if (alt && !event.altKey) return false;
    if (meta && !event.metaKey) return false;

    const eventKey = event.key.toLowerCase();
    if (eventKey === key) return true;
    if (key === '[' && event.code === 'BracketLeft') return true;
    if (key === ']' && event.code === 'BracketRight') return true;
    return false;
  }

  function handleKeydown(e: KeyboardEvent) {
    const shortcuts = $appSettings.shortcuts;

    if (checkShortcut(e, shortcuts.commandPalette)) {
      e.preventDefault();
      e.stopPropagation();
      close();
      return;
    }

    if (checkShortcut(e, shortcuts.newConnection)) {
      e.preventDefault();
      e.stopPropagation();
      executeCommand(allCommands.find(c => c.id === 'new-connection'));
      return;
    }

    if (checkShortcut(e, shortcuts.settings)) {
      e.preventDefault();
      e.stopPropagation();
      executeCommand(allCommands.find(c => c.id === 'open-settings'));
      return;
    }

    if (checkShortcut(e, shortcuts.toggleSidebar)) {
      e.preventDefault();
      e.stopPropagation();
      executeCommand(allCommands.find(c => c.id === 'toggle-sidebar'));
      return;
    }

    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIndex = (selectedIndex + 1) % filteredCommands.length;
      scrollToSelected();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIndex = (selectedIndex - 1 + filteredCommands.length) % filteredCommands.length;
      scrollToSelected();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      executeCommand(filteredCommands[selectedIndex]);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      close();
    }
  }

  function executeCommand(command: Command | undefined) {
    if (!command) return;
    command.action();
    close();
  }

  function scrollToSelected() {
    // Simple logic to keep selected item in view
    // In a real app, might need more complex calculation
    const list = document.getElementById('command-list');
    const item = document.getElementById(`command-item-${selectedIndex}`);
    if (list && item) {
      const listRect = list.getBoundingClientRect();
      const itemRect = item.getBoundingClientRect();
      
      if (itemRect.bottom > listRect.bottom) {
        item.scrollIntoView({ block: 'nearest' });
      } else if (itemRect.top < listRect.top) {
        item.scrollIntoView({ block: 'nearest' });
      }
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === container) {
      close();
    }
  }
</script>

<div 
  bind:this={container}
  class="fixed inset-0 z-50 flex items-start justify-center pt-[15vh] bg-slate-900/20 dark:bg-black/50 backdrop-blur-sm transition-all"
  on:click={handleBackdropClick}
  on:keydown={() => {}} 
  role="presentation"
  transition:fade={{ duration: 150 }}
>
  <div 
    class="w-full max-w-2xl bg-white dark:bg-slate-900 rounded-xl shadow-2xl border border-slate-200 dark:border-slate-800 flex flex-col overflow-hidden max-h-[60vh]"
    transition:fly={{ y: -20, duration: 200 }}
  >
    <!-- Search Input -->
    <div class="p-3 border-b border-slate-200 dark:border-slate-800 flex items-center gap-3">
      <svg class="w-5 h-5 text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"></path>
      </svg>
      <input
        bind:this={inputElement}
        bind:value={searchTerm}
        on:keydown={handleKeydown}
        type="text"
        placeholder="搜索命令或连接..."
        class="flex-1 bg-transparent border-none outline-none text-slate-800 dark:text-slate-100 text-lg placeholder-slate-400"
        autocomplete="off"
      />
      <div class="flex gap-1">
        <kbd class="px-2 py-1 bg-slate-100 dark:bg-slate-800 rounded text-xs text-slate-500 border border-slate-200 dark:border-slate-700">ESC</kbd>
      </div>
    </div>

    <!-- Command List -->
    <div 
      id="command-list"
      class="flex-1 overflow-y-auto p-2 scrollbar-thin scrollbar-thumb-slate-300 dark:scrollbar-thumb-slate-700"
    >
      {#if filteredCommands.length === 0}
        <div class="p-8 text-center text-slate-500 dark:text-slate-400">
          <p>未找到匹配的命令</p>
        </div>
      {:else}
        {#each filteredCommands as command, index}
          <button
            id="command-item-{index}"
            class="w-full text-left px-4 py-3 rounded-lg flex items-center justify-between group transition-colors {index === selectedIndex ? 'bg-blue-50 dark:bg-blue-900/20' : 'hover:bg-slate-50 dark:hover:bg-slate-800/50'}"
            on:click={() => executeCommand(command)}
            on:mousemove={() => selectedIndex = index}
          >
            <div class="flex items-center gap-3 overflow-hidden">
              {#if command.icon}
                <div class="{index === selectedIndex ? 'text-blue-600 dark:text-blue-400' : 'text-slate-400 dark:text-slate-500'}">
                  <svelte:component this={command.icon} class="w-5 h-5" />
                </div>
              {:else}
                <div class="w-5 h-5"></div>
              {/if}
              
              <div class="flex flex-col overflow-hidden">
                <span class="font-medium text-slate-800 dark:text-slate-200 truncate {index === selectedIndex ? 'text-blue-700 dark:text-blue-300' : ''}">
                  {command.title}
                </span>
                {#if command.description}
                  <span class="text-xs text-slate-500 dark:text-slate-400 truncate">
                    {command.description}
                  </span>
                {/if}
              </div>
            </div>

            <div class="flex items-center gap-3">
              {#if command.category}
                <span class="text-xs px-2 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400 border border-slate-200 dark:border-slate-700">
                  {command.category}
                </span>
              {/if}
              {#if command.shortcut}
                <span class="text-xs font-mono text-slate-400">
                  {command.shortcut}
                </span>
              {/if}
            </div>
          </button>
        {/each}
      {/if}
    </div>
    
    <div class="px-4 py-2 bg-slate-50 dark:bg-slate-800/50 border-t border-slate-200 dark:border-slate-800 text-xs text-slate-400 flex justify-between">
      <div class="flex gap-4">
        <span><kbd>↑</kbd> <kbd>↓</kbd> 导航</span>
        <span><kbd>Enter</kbd> 选择</span>
      </div>
      <div>
        <span>Star Shuttle Command Palette</span>
      </div>
    </div>
  </div>
</div>

<style>
  kbd {
    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  }
</style>
