<script lang="ts">
  import { onMount } from 'svelte';
  import { connections, showConnectionForm, editingConnection, isSidebarCollapsed, showSettings, activeTerminals } from '../lib/store';
  import { deleteConnection } from '../lib/connectionService';
  import { connectAndOpen, disconnectTerminal } from '../lib/terminalService';
  import PlusIcon from './icons/PlusIcon.svelte';
  import ServerIcon from './icons/ServerIcon.svelte';
  import TrashIcon from './icons/TrashIcon.svelte';
  import SettingsIcon from './icons/SettingsIcon.svelte';
  import ChevronLeftIcon from './icons/ChevronLeftIcon.svelte';
  import ChevronRightIcon from './icons/ChevronRightIcon.svelte';
  import UploadIcon from './icons/UploadIcon.svelte';
  import DownloadIcon from './icons/DownloadIcon.svelte';
  import { importConnections, exportConnections } from '../lib/importExportService';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import { connectionHistory } from '../lib/store';
  import ClockIcon from './icons/ClockIcon.svelte';
  import ActivityIcon from './icons/ActivityIcon.svelte';
  import SystemMonitorModal from './SystemMonitorModal.svelte';

  let searchTerm = '';
  let activeTab: 'servers' | 'history' = 'servers';
  let showMonitor: string | null = null; // Session ID for monitor
  let monitorConnection: any = null;
  let expandedPaths = new Set<string>();
  let didInitExpanded = false;
  let contextMenu = {
    x: 0,
    y: 0,
    show: false,
    kind: 'blank' as 'blank' | 'folder' | 'connection',
    row: null as TagRow | null
  };

  // Filter connections based on search term
  $: filteredConnections = $connections.filter(c => {
    const term = searchTerm.toLowerCase();
    if (!term) return true;
    const tags = (c.tags || []).join(',').toLowerCase();
    return (
      c.name.toLowerCase().includes(term) ||
      c.host.toLowerCase().includes(term) ||
      c.username.toLowerCase().includes(term) ||
      tags.includes(term)
    );
  });

  type TagNode = {
    name: string;
    path: string;
    children: Map<string, TagNode>;
    connections: any[];
  };

  type TagRow =
    | { kind: 'folder'; id: string; depth: number; name: string; path: string; count: number; hasChildren: boolean }
    | { kind: 'connection'; id: string; depth: number; connection: any };

  type FolderRow = Extract<TagRow, { kind: 'folder' }>;
  type ConnectionRow = Extract<TagRow, { kind: 'connection' }>;

  $: contextMenuFolderRow =
    contextMenu.kind === 'folder' && contextMenu.row?.kind === 'folder' ? (contextMenu.row as FolderRow) : null;
  $: contextMenuConnectionRow =
    contextMenu.kind === 'connection' && contextMenu.row?.kind === 'connection'
      ? (contextMenu.row as ConnectionRow)
      : null;

  function normalizeTags(value: unknown): string[] {
    if (Array.isArray(value)) {
      return value.map(v => String(v).trim()).filter(Boolean);
    }
    if (typeof value === 'string') {
      return value
        .split(',')
        .map(s => s.trim())
        .filter(Boolean);
    }
    return [];
  }

  function splitTagPath(tag: string): string[] {
    return tag
      .split('/')
      .map(s => s.trim())
      .filter(Boolean);
  }

  function buildTagTree(items: any[]): TagNode {
    const root: TagNode = { name: '', path: '', children: new Map(), connections: [] };
    for (const connection of items) {
      const tags: string[] = normalizeTags(connection?.tags);
      const groupTag = tags[0] ? String(tags[0]).trim() : '未分组';
      const parts = groupTag === '未分组' ? ['未分组'] : splitTagPath(groupTag);
      if (parts.length === 0) continue;
      let node = root;
      for (const part of parts) {
        const nextPath = node.path ? `${node.path}/${part}` : part;
        const existing = node.children.get(part);
        if (existing) {
          node = existing;
        } else {
          const created: TagNode = { name: part, path: nextPath, children: new Map(), connections: [] };
          node.children.set(part, created);
          node = created;
        }
      }
      node.connections.push(connection);
    }
    return root;
  }

  function flattenTagTree(node: TagNode, expanded: Set<string>, depth = 0): TagRow[] {
    const rows: TagRow[] = [];
    const children = Array.from(node.children.values()).sort((a, b) => a.name.localeCompare(b.name, 'zh-Hans-CN'));
    for (const child of children) {
      const hasChildren = child.children.size > 0;
      const count = child.connections.length;
      rows.push({
        kind: 'folder',
        id: `folder:${child.path}`,
        depth,
        name: child.name,
        path: child.path,
        count,
        hasChildren,
      });
      if (expanded.has(child.path)) {
        rows.push(...flattenTagTree(child, expanded, depth + 1));
        const sortedConnections = [...child.connections].sort((a, b) =>
          String(a.name ?? '').localeCompare(String(b.name ?? ''), 'zh-Hans-CN')
        );
        for (const connection of sortedConnections) {
          rows.push({
            kind: 'connection',
            id: `connection:${child.path}:${connection.id}`,
            depth: depth + 1,
            connection,
          });
        }
      }
    }
    return rows;
  }

  function toggleFolder(path: string) {
    const next = new Set(expandedPaths);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    expandedPaths = next;
  }

  $: tagTree = buildTagTree(filteredConnections);
  $: {
    if (!didInitExpanded) didInitExpanded = true;
  }
  $: tagRows = flattenTagTree(tagTree, expandedPaths);

  $: filteredHistory = $connectionHistory.filter(h => 
    h.connection.name.toLowerCase().includes(searchTerm.toLowerCase()) || 
    h.connection.host.toLowerCase().includes(searchTerm.toLowerCase())
  );

  $: activeConnectionIds = new Set($activeTerminals.map(t => t.connection.id));

  function formatTimeAgo(timestamp: number) {
    const seconds = Math.floor((Date.now() - timestamp) / 1000);
    
    let interval = seconds / 31536000;
    if (interval > 1) return Math.floor(interval) + "年前";
    
    interval = seconds / 2592000;
    if (interval > 1) return Math.floor(interval) + "月前";
    
    interval = seconds / 86400;
    if (interval > 1) return Math.floor(interval) + "天前";
    
    interval = seconds / 3600;
    if (interval > 1) return Math.floor(interval) + "小时前";
    
    interval = seconds / 60;
    if (interval > 1) return Math.floor(interval) + "分钟前";
    
    return "刚刚";
  }

  function handleConnect(connection: any) {
    connectAndOpen(connection);
  }

  async function handleDelete(id: string, event: MouseEvent) {
    event.stopPropagation();
    const confirmed = await confirm('确定要删除此连接吗？', { title: '删除连接', kind: 'warning' });
    if (!confirmed) return;
    deleteConnection(id);
  }

  function handleEdit(connection: any, event: MouseEvent) {
    event.stopPropagation();
    editingConnection.set(connection);
    showConnectionForm.set(true);
  }

  function toggleSidebar() {
    isSidebarCollapsed.update(v => !v);
  }

  function openMonitor(connection: any, event: MouseEvent) {
      event.stopPropagation();
      // Find active session
      const terminal = $activeTerminals.find(t => t.connection.id === connection.id);
      
      if (terminal) {
          showMonitor = terminal.sessionId;
          monitorConnection = connection;
      } else {
          alert('请先连接到服务器才能打开监控面板');
      }
  }

  function openContextMenu(e: MouseEvent, kind: 'blank' | 'folder' | 'connection', row: TagRow | null) {
    e.preventDefault();
    contextMenu = {
      x: e.clientX,
      y: e.clientY,
      show: true,
      kind,
      row
    };
  }

  function closeContextMenu() {
    contextMenu.show = false;
  }

  async function handleDisconnect(connection: any) {
    const terminal = $activeTerminals.find(t => t.connection.id === connection.id);
    if (!terminal) return;
    await disconnectTerminal(terminal.sessionId);
  }

  onMount(() => {
    window.addEventListener('click', closeContextMenu);
    return () => window.removeEventListener('click', closeContextMenu);
  });
</script>

{#if showMonitor && monitorConnection}
    <SystemMonitorModal 
        sessionId={showMonitor} 
        connection={monitorConnection} 
        onClose={() => { showMonitor = null; monitorConnection = null; }} 
    />
{/if}

<aside class="flex flex-col bg-white dark:bg-slate-900 border-r border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-300 transition-all duration-300 ease-in-out {$isSidebarCollapsed ? 'w-16' : 'w-64'}">
  <!-- Sidebar Header -->
  <div class="p-4 border-b border-slate-200 dark:border-slate-800 flex flex-col gap-4">
    <div class="flex items-center {$isSidebarCollapsed ? 'justify-center' : 'gap-3'}">
      <div class="w-8 h-8 bg-blue-600 rounded-lg flex items-center justify-center shadow-lg shadow-blue-900/20 shrink-0">
        <span class="font-bold text-xs text-white">SSH</span>
      </div>
      {#if !$isSidebarCollapsed}
        <h1 class="text-base font-bold text-slate-800 dark:text-slate-100 tracking-wide whitespace-nowrap overflow-hidden">
          Remote Manager
        </h1>
      {/if}
    </div>
    
    <div class="flex gap-2">
      <button
        class="flex-1 flex items-center justify-center gap-2 bg-blue-600 hover:bg-blue-500 text-white py-2 px-3 rounded-lg font-medium transition-all shadow-md hover:shadow-blue-900/30 active:scale-95"
        on:click={() => { editingConnection.set(null); showConnectionForm.set(true); }}
        title="新建连接"
      >
        <PlusIcon class="w-4 h-4" />
        {#if !$isSidebarCollapsed}
          <span class="whitespace-nowrap">新建</span>
        {/if}
      </button>
    </div>
  </div>

  <!-- Tabs -->
  {#if !$isSidebarCollapsed}
    <div class="px-4 pt-4 flex gap-1">
       <button 
         class="flex-1 py-1.5 text-xs font-medium rounded-md transition-all {activeTab === 'servers' ? 'bg-slate-200 dark:bg-slate-800 text-slate-900 dark:text-slate-100' : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'}"
         on:click={() => activeTab = 'servers'}
       >
         服务器
       </button>
       <button 
         class="flex-1 py-1.5 text-xs font-medium rounded-md transition-all {activeTab === 'history' ? 'bg-slate-200 dark:bg-slate-800 text-slate-900 dark:text-slate-100' : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'}"
         on:click={() => activeTab = 'history'}
       >
         历史
       </button>
    </div>
  {/if}

  <!-- Search -->
  {#if !$isSidebarCollapsed}
    <div class="px-4 py-3">
      <div class="relative">
        <input
          type="text"
          placeholder="搜索连接..."
          bind:value={searchTerm}
          class="w-full bg-slate-100 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-lg py-1.5 px-3 pl-9 text-sm text-slate-900 dark:text-slate-200 placeholder-slate-400 dark:placeholder-slate-500 focus:outline-none focus:border-blue-500/50 focus:ring-1 focus:ring-blue-500/50 transition-all"
        />
        <svg class="absolute left-3 top-2 w-4 h-4 text-slate-400 dark:text-slate-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"></path>
        </svg>
      </div>
    </div>
  {/if}

  <!-- Connection List -->
  <div
    class="flex-1 overflow-y-auto px-2 py-2 space-y-0.5 custom-scrollbar"
    on:contextmenu|preventDefault={(e) => openContextMenu(e, 'blank', null)}
    role="presentation"
  >
    {#if activeTab === 'servers'}
        {#if !$isSidebarCollapsed}
            <div class="px-2 py-1.5 flex justify-between items-center text-xs font-semibold text-slate-400 dark:text-slate-500 uppercase tracking-wider whitespace-nowrap">
              <span>服务器列表</span>
              <div class="flex gap-1">
                 <button class="p-1 hover:text-slate-200 transition-colors" title="导入配置" on:click={importConnections}>
                    <UploadIcon className="w-3 h-3" />
                 </button>
                 <button class="p-1 hover:text-slate-200 transition-colors" title="导出配置" on:click={() => exportConnections()}>
                    <DownloadIcon className="w-3 h-3" />
                 </button>
              </div>
            </div>
        {/if}
        {#if tagRows.length > 0}
          {#each tagRows as row (row.id)}
            {#if row.kind === 'folder'}
              <div class="group relative">
                <button
                  class="w-full text-left flex items-center {$isSidebarCollapsed ? 'justify-center p-2' : 'gap-2 p-2'} rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors"
                  on:click={() => toggleFolder(row.path)}
                  on:contextmenu|preventDefault|stopPropagation={(e) => openContextMenu(e, 'folder', row)}
                  title={$isSidebarCollapsed ? row.name : ''}
                  style={!$isSidebarCollapsed ? `padding-left: ${0.5 + row.depth * 0.75}rem;` : ''}
                >
                  {#if !$isSidebarCollapsed}
                    <span class="text-slate-400 dark:text-slate-500 w-4 inline-flex justify-center">
                      {#if expandedPaths.has(row.path)}
                        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
                        </svg>
                      {:else}
                        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"></path>
                        </svg>
                      {/if}
                    </span>
                    <span class="flex-1 min-w-0">
                      <span class="font-medium text-slate-700 dark:text-slate-200 truncate">{row.name}</span>
                    </span>
                    <span class="text-[10px] text-slate-400 dark:text-slate-500 bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded-full">
                      {row.count}
                    </span>
                  {/if}
                </button>
              </div>
            {:else}
              <div class="group relative">
                <button
                  class="w-full text-left flex items-center {$isSidebarCollapsed ? 'justify-center p-2' : 'gap-3 p-2'} rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors group-hover:shadow-sm"
                  on:click={() => handleConnect(row.connection)}
                  on:contextmenu|preventDefault|stopPropagation={(e) => openContextMenu(e, 'connection', row)}
                  title={$isSidebarCollapsed ? `${row.connection.name} (${row.connection.username}@${row.connection.host})` : ''}
                  style={!$isSidebarCollapsed ? `padding-left: ${0.5 + row.depth * 0.75}rem;` : ''}
                >
                  <div class="text-slate-400 group-hover:text-blue-500 dark:group-hover:text-blue-400 transition-colors shrink-0">
                    <ServerIcon class="w-4 h-4" />
                  </div>
                  {#if !$isSidebarCollapsed}
                    <div class="flex-1 min-w-0">
                      <div class="font-medium text-slate-700 dark:text-slate-200 truncate group-hover:text-slate-900 dark:group-hover:text-white transition-colors flex items-center gap-2">
                        <span class="truncate">{row.connection.name}</span>
                        <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400 shrink-0">
                          {(row.connection as any).protocol === 'Rdp' ? 'RDP' : 'SSH'}
                        </span>
                      </div>
                      <div class="text-xs text-slate-500 truncate mt-0.5 font-mono opacity-80">
                        {#if row.connection.username}{row.connection.username}@{/if}{row.connection.host}
                      </div>
                    </div>
                  {/if}
                </button>

                {#if !$isSidebarCollapsed}
                  {#if activeConnectionIds.has(row.connection.id)}
                    <button
                      class="absolute right-[4.25rem] top-2.5 p-1.5 rounded-md text-green-500 hover:text-green-600 hover:bg-green-100 dark:hover:bg-green-900/30 transition-all"
                      on:click={(e) => openMonitor(row.connection, e)}
                      title="系统监控"
                    >
                      <ActivityIcon class="w-3.5 h-3.5" />
                    </button>
                  {/if}
                  <button
                    class="absolute right-9 top-2.5 p-1.5 rounded-md text-slate-400 dark:text-slate-500 hover:text-blue-500 dark:hover:text-blue-400 hover:bg-slate-200 dark:hover:bg-slate-700/50 opacity-0 group-hover:opacity-100 transition-all"
                    on:click={(e) => handleEdit(row.connection, e)}
                    title="编辑连接"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 4H6a2 2 0 00-2 2v12a2 2 0 002 2h12a2 2 0 002-2v-5M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"></path></svg>
                  </button>
                  <button
                    class="absolute right-2 top-2.5 p-1.5 rounded-md text-slate-400 dark:text-slate-500 hover:text-red-500 dark:hover:text-red-400 hover:bg-slate-200 dark:hover:bg-slate-700/50 opacity-0 group-hover:opacity-100 transition-all"
                    on:click={(e) => handleDelete(row.connection.id, e)}
                    title="删除连接"
                  >
                    <TrashIcon class="w-3.5 h-3.5" />
                  </button>
                {/if}
              </div>
            {/if}
          {/each}
        {:else}
          {#if !$isSidebarCollapsed}
            <div class="flex flex-col items-center justify-center py-10 text-slate-400 dark:text-slate-500">
              <p class="text-sm">未找到连接</p>
            </div>
          {/if}
        {/if}
    {:else if activeTab === 'history'}
        {#if !$isSidebarCollapsed}
            <div class="px-2 py-1.5 flex justify-between items-center text-xs font-semibold text-slate-400 dark:text-slate-500 uppercase tracking-wider whitespace-nowrap">
              <span>最近连接</span>
            </div>
        {/if}
        {#if filteredHistory.length > 0}
          {#each filteredHistory as item}
            <div class="group relative">
              <button
                class="w-full text-left flex items-center {$isSidebarCollapsed ? 'justify-center p-2' : 'gap-3 p-2.5'} rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors group-hover:shadow-sm"
                on:click={() => handleConnect(item.connection)}
                title={$isSidebarCollapsed ? `${item.connection.name} - ${formatTimeAgo(item.lastConnected)}` : ''}
              >
                <div class="text-slate-400 group-hover:text-green-500 dark:group-hover:text-green-400 transition-colors shrink-0">
                  <ClockIcon className="w-4 h-4" />
                </div>
                {#if !$isSidebarCollapsed}
                  <div class="flex-1 min-w-0">
                    <div class="flex justify-between items-center">
                       <div class="font-medium text-slate-700 dark:text-slate-200 truncate group-hover:text-slate-900 dark:group-hover:text-white transition-colors flex items-center gap-2">
                         <span class="truncate">{item.connection.name}</span>
                         <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-slate-100 dark:bg-slate-800 text-slate-500 dark:text-slate-400 shrink-0">
                           {(item.connection as any).protocol === 'Rdp' ? 'RDP' : 'SSH'}
                         </span>
                       </div>
                       <span class="text-[10px] text-slate-400 bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded-full">
                         {formatTimeAgo(item.lastConnected)}
                       </span>
                    </div>
                    <div class="text-xs text-slate-500 truncate mt-0.5 font-mono opacity-80">
                      {#if item.connection.username}{item.connection.username}@{/if}{item.connection.host}
                    </div>
                  </div>
                {/if}
              </button>
            </div>
          {/each}
        {:else}
          {#if !$isSidebarCollapsed}
            <div class="flex flex-col items-center justify-center py-10 text-slate-400 dark:text-slate-500">
              <p class="text-sm">无历史记录</p>
            </div>
          {/if}
        {/if}
    {/if}
  </div>

  {#if contextMenu.show}
    <div
      class="fixed bg-white dark:bg-gray-800 border border-slate-200 dark:border-gray-700 rounded shadow-lg py-1 z-50 text-sm min-w-[180px]"
      style="top: {contextMenu.y}px; left: {contextMenu.x}px"
      role="menu"
      tabindex="-1"
      on:click|stopPropagation={() => {}}
      on:keydown|stopPropagation={() => {}}
    >
      {#if contextMenuConnectionRow}
        <button
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
          on:click|stopPropagation={() => { closeContextMenu(); handleConnect(contextMenuConnectionRow.connection); }}
        >
          连接
        </button>
        {#if activeConnectionIds.has(contextMenuConnectionRow.connection.id)}
          <button
            class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
            on:click|stopPropagation={() => { closeContextMenu(); handleDisconnect(contextMenuConnectionRow.connection); }}
          >
            断开连接
          </button>
          <button
            class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
            on:click|stopPropagation={(e) => { closeContextMenu(); openMonitor(contextMenuConnectionRow.connection, e as any); }}
          >
            系统监控
          </button>
        {/if}
        <div class="border-t border-slate-200 dark:border-gray-700 my-1"></div>
        <button
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
          on:click|stopPropagation={(e) => { closeContextMenu(); handleEdit(contextMenuConnectionRow.connection, e as any); }}
        >
          编辑
        </button>
        <button
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-red-600 dark:text-red-400 hover:text-red-500 dark:hover:text-red-300"
          on:click|stopPropagation={(e) => { closeContextMenu(); handleDelete(contextMenuConnectionRow.connection.id, e as any); }}
        >
          删除
        </button>
      {:else if contextMenuFolderRow}
        <button
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
          on:click|stopPropagation={() => { closeContextMenu(); toggleFolder(contextMenuFolderRow.path); }}
        >
          {expandedPaths.has(contextMenuFolderRow.path) ? '折叠' : '展开'}
        </button>
      {:else}
        <button
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
          on:click|stopPropagation={() => { closeContextMenu(); editingConnection.set(null); showConnectionForm.set(true); }}
        >
          新建连接
        </button>
        <div class="border-t border-slate-200 dark:border-gray-700 my-1"></div>
        <button
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
          on:click|stopPropagation={() => { closeContextMenu(); importConnections(); }}
        >
          导入配置
        </button>
        <button
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
          on:click|stopPropagation={() => { closeContextMenu(); exportConnections(); }}
        >
          导出配置
        </button>
      {/if}
    </div>
  {/if}

  <!-- Sidebar Footer -->
  <div class="p-4 border-t border-slate-200 dark:border-slate-800 flex flex-col gap-2">
    <button 
      class="flex items-center {$isSidebarCollapsed ? 'justify-center' : 'gap-2'} text-sm text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200 transition-colors w-full px-2 py-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800"
      on:click={() => showSettings.set(true)}
      title="设置"
    >
      <SettingsIcon class="w-4 h-4" />
      {#if !$isSidebarCollapsed}
        <span>设置</span>
      {/if}
    </button>
    
    <button 
      class="flex items-center {$isSidebarCollapsed ? 'justify-center' : 'gap-2'} text-sm text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200 transition-colors w-full px-2 py-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 mt-1"
      on:click={toggleSidebar}
      title={$isSidebarCollapsed ? "展开侧边栏" : "折叠侧边栏"}
    >
      {#if $isSidebarCollapsed}
        <ChevronRightIcon class="w-4 h-4" />
      {:else}
        <ChevronLeftIcon class="w-4 h-4" />
        <span>折叠侧边栏</span>
      {/if}
    </button>
  </div>
</aside>
