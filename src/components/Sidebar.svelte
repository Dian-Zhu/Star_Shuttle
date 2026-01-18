<script lang="ts">
  import { onMount } from 'svelte';
  import { connections, showConnectionForm, editingConnection, isSidebarCollapsed, showSettings, activeTerminals, connectionGroups } from '../lib/store';
  import { loadConnections, deleteConnection, updateConnectionConfig } from '../lib/connectionService';
  import { connectAndOpen } from '../lib/terminalService';
  import PlusIcon from './icons/PlusIcon.svelte';
  import ServerIcon from './icons/ServerIcon.svelte';
  import TrashIcon from './icons/TrashIcon.svelte';
  import SettingsIcon from './icons/SettingsIcon.svelte';
  import ChevronLeftIcon from './icons/ChevronLeftIcon.svelte';
  import ChevronRightIcon from './icons/ChevronRightIcon.svelte';
  import UploadIcon from './icons/UploadIcon.svelte';
  import DownloadIcon from './icons/DownloadIcon.svelte';
  import AdvancedIcon from './icons/AdvancedIcon.svelte';
  import { importConnections, exportConnections } from '../lib/importExportService';
  import { showAdvancedModal, connectionHistory } from '../lib/store';
  import ClockIcon from './icons/ClockIcon.svelte';
  import ActivityIcon from './icons/ActivityIcon.svelte';
  import SystemMonitorModal from './SystemMonitorModal.svelte';
  import { v4 as uuidv4 } from 'uuid';

  let searchTerm = '';
  let activeTab: 'servers' | 'history' = 'servers';
  let showMonitor: string | null = null; // Session ID for monitor
  let monitorConnection: any = null;
  let selectedGroupId: 'all' | 'ungrouped' | string = 'all';

  $: groupNameById = new Map($connectionGroups.map(g => [g.id, g.name]));

  // Filter connections based on search term
  $: filteredConnections = $connections
    .filter(c => {
      if (selectedGroupId === 'all') return true;
      if (selectedGroupId === 'ungrouped') return !c.group_id;
      return c.group_id === selectedGroupId;
    })
    .filter(c => {
      const term = searchTerm.toLowerCase();
      const tags = (c.tags || []).join(',').toLowerCase();
      return (
        c.name.toLowerCase().includes(term) ||
        c.host.toLowerCase().includes(term) ||
        c.username.toLowerCase().includes(term) ||
        tags.includes(term)
      );
    });

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

  onMount(() => {
    loadConnections();
  });

  function handleConnect(connection: any) {
    connectAndOpen(connection);
  }

  function handleDelete(id: string, event: MouseEvent) {
    event.stopPropagation();
    if (confirm('确定要删除此连接吗？')) {
      deleteConnection(id);
    }
  }

  function handleEdit(connection: any, event: MouseEvent) {
    event.stopPropagation();
    editingConnection.set(connection);
    showConnectionForm.set(true);
  }

  function toggleSidebar() {
    isSidebarCollapsed.update(v => !v);
  }

  function addGroup() {
    const name = window.prompt('请输入分组名称');
    if (!name) return;
    const trimmed = name.trim();
    if (!trimmed) return;
    const id = uuidv4();
    connectionGroups.update(groups => [...groups, { id, name: trimmed, createdAt: Date.now() }]);
    selectedGroupId = id;
  }

  function renameSelectedGroup() {
    if (selectedGroupId === 'all' || selectedGroupId === 'ungrouped') return;
    const currentName = groupNameById.get(selectedGroupId) || '';
    const name = window.prompt('请输入新的分组名称', currentName);
    if (!name) return;
    const trimmed = name.trim();
    if (!trimmed) return;
    connectionGroups.update(groups =>
      groups.map(g => (g.id === selectedGroupId ? { ...g, name: trimmed } : g))
    );
  }

  async function deleteSelectedGroup() {
    if (selectedGroupId === 'all' || selectedGroupId === 'ungrouped') return;
    const name = groupNameById.get(selectedGroupId) || '该分组';
    if (!confirm(`确定要删除分组「${name}」吗？分组内连接将被设为未分组。`)) return;

    const groupId = selectedGroupId;
    selectedGroupId = 'all';
    connectionGroups.update(groups => groups.filter(g => g.id !== groupId));

    const affected = $connections.filter(c => c.group_id === groupId);
    for (const c of affected) {
      await updateConnectionConfig({ ...c, group_id: null });
    }
    await loadConnections();
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

      <button
        class="flex-1 flex items-center justify-center gap-2 bg-slate-100 dark:bg-slate-800 hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-600 dark:text-slate-300 py-2 px-3 rounded-lg font-medium transition-all"
        on:click={() => showAdvancedModal.set(true)}
        title="高级功能"
      >
        <AdvancedIcon className="w-4 h-4" />
        {#if !$isSidebarCollapsed}
          <span class="whitespace-nowrap">高级</span>
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
      {#if activeTab === 'servers'}
        <div class="flex items-center gap-2 mb-3">
          <select
            bind:value={selectedGroupId}
            class="flex-1 bg-slate-100 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 rounded-lg py-1.5 px-3 text-sm text-slate-900 dark:text-slate-200 focus:outline-none focus:border-blue-500/50 focus:ring-1 focus:ring-blue-500/50 transition-all"
          >
            <option value="all">所有分组</option>
            <option value="ungrouped">未分组</option>
            {#each $connectionGroups as group}
              <option value={group.id}>{group.name}</option>
            {/each}
          </select>
          <button
            class="px-2 py-1.5 rounded-lg bg-slate-100 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 hover:bg-slate-200 dark:hover:bg-slate-800 text-xs"
            on:click={addGroup}
            title="新建分组"
            type="button"
          >
            新建
          </button>
          <button
            class="px-2 py-1.5 rounded-lg bg-slate-100 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 hover:bg-slate-200 dark:hover:bg-slate-800 text-xs disabled:opacity-50"
            on:click={renameSelectedGroup}
            disabled={selectedGroupId === 'all' || selectedGroupId === 'ungrouped'}
            title="重命名分组"
            type="button"
          >
            重命名
          </button>
          <button
            class="px-2 py-1.5 rounded-lg bg-slate-100 dark:bg-slate-950 border border-slate-200 dark:border-slate-800 hover:bg-red-100 dark:hover:bg-red-900/30 text-xs text-red-600 dark:text-red-400 disabled:opacity-50"
            on:click={deleteSelectedGroup}
            disabled={selectedGroupId === 'all' || selectedGroupId === 'ungrouped'}
            title="删除分组"
            type="button"
          >
            删除
          </button>
        </div>
      {/if}
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
  <div class="flex-1 overflow-y-auto px-2 py-2 space-y-0.5 custom-scrollbar">
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
        {#if filteredConnections.length > 0}
          {#each filteredConnections as connection}
            <div class="group relative">
              <button
                class="w-full text-left flex items-center {$isSidebarCollapsed ? 'justify-center p-2' : 'gap-3 p-2.5'} rounded-lg hover:bg-slate-100 dark:hover:bg-slate-800 transition-colors group-hover:shadow-sm"
                on:click={() => handleConnect(connection)}
                title={$isSidebarCollapsed ? `${connection.name} (${connection.username}@${connection.host})` : ''}
              >
                <div class="text-slate-400 group-hover:text-blue-500 dark:group-hover:text-blue-400 transition-colors shrink-0">
                  <ServerIcon class="w-4 h-4" />
                </div>
                {#if !$isSidebarCollapsed}
                  <div class="flex-1 min-w-0">
                    <div class="font-medium text-slate-700 dark:text-slate-200 truncate group-hover:text-slate-900 dark:group-hover:text-white transition-colors">
                      {connection.name}
                    </div>
                    <div class="text-xs text-slate-500 truncate mt-0.5 font-mono opacity-80">
                      {connection.username}@{connection.host}{groupNameById.get(connection.group_id || '') ? ` · ${groupNameById.get(connection.group_id || '')}` : ''}
                    </div>
                  </div>
                {/if}
              </button>
              
              {#if !$isSidebarCollapsed}
                {#if activeConnectionIds.has(connection.id)}
                  <button
                    class="absolute right-[4.25rem] top-2.5 p-1.5 rounded-md text-green-500 hover:text-green-600 hover:bg-green-100 dark:hover:bg-green-900/30 transition-all"
                    on:click={(e) => openMonitor(connection, e)}
                    title="系统监控"
                  >
                    <ActivityIcon class="w-3.5 h-3.5" />
                  </button>
                {/if}
                <button
                  class="absolute right-9 top-2.5 p-1.5 rounded-md text-slate-400 dark:text-slate-500 hover:text-blue-500 dark:hover:text-blue-400 hover:bg-slate-200 dark:hover:bg-slate-700/50 opacity-0 group-hover:opacity-100 transition-all"
                  on:click={(e) => handleEdit(connection, e)}
                  title="编辑连接"
                >
                  <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 4H6a2 2 0 00-2 2v12a2 2 0 002 2h12a2 2 0 002-2v-5M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"></path></svg>
                </button>
                <button
                  class="absolute right-2 top-2.5 p-1.5 rounded-md text-slate-400 dark:text-slate-500 hover:text-red-500 dark:hover:text-red-400 hover:bg-slate-200 dark:hover:bg-slate-700/50 opacity-0 group-hover:opacity-100 transition-all"
                  on:click={(e) => handleDelete(connection.id, e)}
                  title="删除连接"
                >
                  <TrashIcon class="w-3.5 h-3.5" />
                </button>
              {/if}
            </div>
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
                       <div class="font-medium text-slate-700 dark:text-slate-200 truncate group-hover:text-slate-900 dark:group-hover:text-white transition-colors">
                         {item.connection.name}
                       </div>
                       <span class="text-[10px] text-slate-400 bg-slate-100 dark:bg-slate-800 px-1.5 py-0.5 rounded-full">
                         {formatTimeAgo(item.lastConnected)}
                       </span>
                    </div>
                    <div class="text-xs text-slate-500 truncate mt-0.5 font-mono opacity-80">
                      {item.connection.username}@{item.connection.host}
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

<style>
  .custom-scrollbar::-webkit-scrollbar {
    width: 4px;
  }
  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: #334155;
    border-radius: 2px;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: #475569;
  }
</style>
