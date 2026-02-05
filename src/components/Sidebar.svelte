<script lang="ts">
  import { onMount } from 'svelte';
  import { connections, showConnectionForm, editingConnection, isSidebarCollapsed, isRightSidebarOpen, activeTerminals, connectionGroups, getGroupIdByPath } from '../lib/store';
  import { deleteConnection, updateConnectionConfig } from '../lib/connectionService';
  import { connectAndOpen, disconnectTerminal } from '../lib/terminalService';
  import SystemMonitorModal from './SystemMonitorModal.svelte';
  import PlusIcon from './icons/PlusIcon.svelte';
  import ServerIcon from './icons/ServerIcon.svelte';
  import TrashIcon from './icons/TrashIcon.svelte';
  import SettingsIcon from './icons/SettingsIcon.svelte';
  import ActivityIcon from './icons/ActivityIcon.svelte';
  import ClockIcon from './icons/ClockIcon.svelte';
  import FolderIcon from './icons/FolderIcon.svelte';
  import UploadIcon from './icons/UploadIcon.svelte';
  import DownloadIcon from './icons/DownloadIcon.svelte';
  import { importConnections, exportConnections } from '../lib/importExportService';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import { connectionHistory, successMessage } from '../lib/store';
  import { v4 as uuidv4 } from 'uuid';

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

  function buildTagTree(items: any[], groups: any[]): TagNode {
    const root: TagNode = { name: '', path: '', children: new Map(), connections: [] };

    // Build tree from connections
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

    // Add empty groups from connectionGroups that don't exist in the tree yet
    const allGroups = groups;
    const existingPaths = new Set<string>();

    // Collect all existing paths from the tree
    function collectPaths(node: TagNode) {
      for (const child of node.children.values()) {
        existingPaths.add(child.path);
        collectPaths(child);
      }
    }
    collectPaths(root);

    // Add "未分组" if it doesn't exist (this is a system group for connections without tags)
    if (!existingPaths.has('未分组')) {
      root.children.set('未分组', { name: '未分组', path: '未分组', children: new Map(), connections: [] });
    }

    // Add missing empty groups from connectionGroups
    for (const group of allGroups) {
      if (!existingPaths.has(group.name)) {
        const parts = group.name === '未分组' ? ['未分组'] : splitTagPath(group.name);
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
      }
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

  $: tagTree = buildTagTree(filteredConnections, $connectionGroups);
  $: {
    if (!didInitExpanded) {
      didInitExpanded = true;
      // Initialize expanded paths with all groups and "未分组"
      const newPaths = new Set(expandedPaths);
      newPaths.add('未分组');
      $connectionGroups.forEach(group => {
        newPaths.add(group.name);
      });
      expandedPaths = newPaths;
    }
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

  async function createNewGroup() {
    const groupName = prompt('请输入分组名称:');
    if (!groupName || !groupName.trim()) return;

    const trimmedName = groupName.trim();

    // Check if group already exists
    const existingGroups = $connectionGroups;
    if (existingGroups.some(g => g.name === trimmedName)) {
      alert('分组已存在');
      return;
    }

    // Create new group with a default tag that matches the group name
    // This ensures the group appears in the sidebar tree structure
    const newGroup = {
      id: uuidv4(),
      name: trimmedName,
      createdAt: Date.now()
    };

    connectionGroups.update(groups => [...groups, newGroup]);

    // Auto-expand to show the new group
    const next = new Set(expandedPaths);
    next.add(trimmedName);
    expandedPaths = next;
  }

  async function deleteGroup(folderPath: string) {
    // Check if group exists in connectionGroups
    const group = $connectionGroups.find(g => g.name === folderPath);
    if (!group) {
      alert('无法删除系统默认分组（如"未分组"）');
      return;
    }

    // Find all connections in this group
    const connectionsInGroup = $connections.filter(c => {
      const tags = c.tags || [];
      const groupTag = tags[0] ? String(tags[0]).trim() : '';
      return groupTag === folderPath;
    });

    if (connectionsInGroup.length > 0) {
      const confirmed = await confirm(
        `分组「${folderPath}」下有 ${connectionsInGroup.length} 个连接。\n删除分组后，这些连接将移至「未分组」。\n是否继续？`,
        { title: '删除分组', kind: 'warning' }
      );
      if (!confirmed) return;

      // Update all connections in this group to move to "未分组"
      for (const conn of connectionsInGroup) {
        const updatedConnection = { ...conn };
        const newTags = ['未分组', ...conn.tags.slice(1)];
        updatedConnection.tags = newTags;
        updatedConnection.group_id = null;
        await updateConnectionConfig(updatedConnection);
      }

      // Update local store
      connections.update(conns =>
        conns.map(c => {
          const tags = c.tags || [];
          const groupTag = tags[0] ? String(tags[0]).trim() : '';
          if (groupTag === folderPath) {
            return { ...c, tags: ['未分组', ...tags.slice(1)], group_id: null };
          }
          return c;
        })
      );
    } else {
      const confirmed = await confirm(`确定要删除分组「${folderPath}」吗？`, { title: '删除分组', kind: 'warning' });
      if (!confirmed) return;
    }

    // Remove group from connectionGroups
    connectionGroups.update(groups => groups.filter(g => g.name !== folderPath));

    // Remove from expandedPaths to trigger UI refresh
    expandedPaths = new Set([...expandedPaths].filter(path => path !== folderPath));

    successMessage.set(`分组「${folderPath}」已删除`);
    setTimeout(() => successMessage.set(null), 3000);
  }

  let draggedConnection: any = null;
  let dragOverFolderPath: string | null = null;

  function handleDragStart(e: DragEvent, connection: any) {
    draggedConnection = connection;
    e.dataTransfer?.setData('text/plain', connection.id);
    e.dataTransfer!.effectAllowed = 'move';
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    e.dataTransfer!.dropEffect = 'move';
  }

  async function handleDrop(e: DragEvent, folderPath: string) {
    e.preventDefault();
    if (!draggedConnection) return;

    // Update connection's tags to set first tag as folder path
    const updatedConnection = { ...draggedConnection };
    const newTags = [folderPath, ...updatedConnection.tags.slice(1)];
    updatedConnection.tags = newTags;

    // Find corresponding group_id based on folder path
    const groupId = getGroupIdByPath($connectionGroups, folderPath);
    updatedConnection.group_id = groupId;

    // Update in backend
    await updateConnectionConfig(updatedConnection);

    // Update local store
    connections.update(conns =>
      conns.map(c => c.id === updatedConnection.id ? updatedConnection : c)
    );

    draggedConnection = null;
    dragOverFolderPath = null;
  }

  function handleDragEnd() {
    draggedConnection = null;
    dragOverFolderPath = null;
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

<aside class="flex flex-col bg-app-bg border-r border-app-border text-app-text-secondary transition-all duration-300 ease-in-out {$isSidebarCollapsed ? 'w-[47px]' : 'w-64'}">
  <!-- Sidebar Header -->
  <div class="{$isSidebarCollapsed ? 'p-2' : 'p-4'} border-b border-app-border flex flex-col gap-4">
    <div class="flex gap-2">
      <button
        class="{$isSidebarCollapsed ? 'w-8 h-8 p-0' : 'flex-1 py-2 px-3'} flex items-center justify-center {$isSidebarCollapsed ? '' : 'gap-2'} bg-primary-600 hover:bg-primary-500 text-white rounded-lg font-medium transition-all shadow-md hover:shadow-primary-900/30 active:scale-95"
        on:click={() => { editingConnection.set(null); showConnectionForm.set(true); }}
        title="新建连接"
      >
        <PlusIcon class="w-4 h-4" />
        {#if !$isSidebarCollapsed}
          <span class="whitespace-nowrap">新建连接</span>
        {/if}
      </button>
    </div>
  </div>

  <!-- Tabs -->
  {#if !$isSidebarCollapsed}
    <div class="px-4 pt-4 flex gap-1">
       <button 
         class="flex-1 py-1.5 text-xs font-medium rounded-md transition-all {activeTab === 'servers' ? 'bg-app-surface text-app-text' : 'text-app-text-secondary hover:text-app-text'}"
         on:click={() => activeTab = 'servers'}
       >
         服务器
       </button>
       <button 
         class="flex-1 py-1.5 text-xs font-medium rounded-md transition-all {activeTab === 'history' ? 'bg-app-surface text-app-text' : 'text-app-text-secondary hover:text-app-text'}"
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
          class="w-full bg-app-surface border border-app-border rounded-lg py-1.5 px-3 pl-9 text-sm text-app-text placeholder-app-text-secondary focus:outline-none focus:border-primary-500/50 focus:ring-1 focus:ring-primary-500/50 transition-all"
        />
        <svg class="absolute left-3 top-2 w-4 h-4 text-app-text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
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
            <div class="px-2 py-1.5 flex justify-between items-center text-xs font-semibold text-app-text-secondary uppercase tracking-wider whitespace-nowrap">
              <span>服务器列表</span>
              <div class="flex gap-1">
                 <button class="p-1 hover:text-app-text transition-colors" title="导入配置" on:click={importConnections}>
                    <UploadIcon className="w-3 h-3" />
                 </button>
                 <button class="p-1 hover:text-app-text transition-colors" title="导出配置" on:click={() => exportConnections()}>
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
                  class="w-full text-left flex items-center {$isSidebarCollapsed ? 'justify-center p-2' : 'gap-2 p-2'} rounded-lg hover:bg-app-surface transition-colors drop-target"
                  class:drag-over={dragOverFolderPath === row.path}
                  on:click={() => toggleFolder(row.path)}
                  on:contextmenu|preventDefault|stopPropagation={(e) => openContextMenu(e, 'folder', row)}
                  title={$isSidebarCollapsed ? row.name : ''}
                  style={!$isSidebarCollapsed ? `padding-left: ${0.5 + row.depth * 0.75}rem;` : ''}
                  on:dragover={handleDragOver}
                  on:dragenter={() => (dragOverFolderPath = row.path)}
                  on:dragleave={() => {
                    if (dragOverFolderPath === row.path) dragOverFolderPath = null;
                  }}
                  on:drop={(e) => handleDrop(e, row.path)}
                >
                  {#if !$isSidebarCollapsed}
                    <span class="text-app-text-secondary w-4 inline-flex justify-center">
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
                      <span class="font-medium text-app-text truncate">{row.name}</span>
                    </span>
                    <span class="text-[10px] text-app-text-secondary bg-app-surface px-1.5 py-0.5 rounded-full">
                      {row.count}
                    </span>
                  {/if}
                </button>
              </div>
            {:else}
              <div class="group relative">
                <button
                  class="w-full text-left flex items-center {$isSidebarCollapsed ? 'justify-center p-2' : 'gap-3 p-2'} rounded-lg hover:bg-app-surface transition-colors group-hover:shadow-sm cursor-move"
                  on:click={() => handleConnect(row.connection)}
                  on:contextmenu|preventDefault|stopPropagation={(e) => openContextMenu(e, 'connection', row)}
                  title={$isSidebarCollapsed ? `${row.connection.name} (${row.connection.username}@${row.connection.host})` : ''}
                  style={!$isSidebarCollapsed ? `padding-left: ${0.5 + row.depth * 0.75}rem;` : ''}
                  draggable="true"
                  on:dragstart={(e) => handleDragStart(e, row.connection)}
                  on:dragend={handleDragEnd}
                >
                  <div class="text-app-text-secondary group-hover:text-primary-500 dark:group-hover:text-primary-400 transition-colors shrink-0">
                    <ServerIcon class="w-4 h-4" />
                  </div>
                  {#if !$isSidebarCollapsed}
                    <div class="flex-1 min-w-0">
                      <div class="font-medium text-app-text truncate group-hover:text-app-text transition-colors flex items-center gap-2">
                        <span class="truncate">{row.connection.name}</span>
                        <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-app-surface text-app-text-secondary shrink-0">
                          {(row.connection as any).protocol === 'Rdp' ? 'RDP' : 'SSH'}
                        </span>
                      </div>
                      <div class="text-xs text-app-text-secondary truncate mt-0.5 font-mono opacity-80">
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
                    class="absolute right-9 top-2.5 p-1.5 rounded-md text-app-text-secondary hover:text-primary-500 dark:hover:text-primary-400 hover:bg-app-border opacity-0 group-hover:opacity-100 transition-all"
                    on:click={(e) => handleEdit(row.connection, e)}
                    title="编辑连接"
                  >
                    <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 4H6a2 2 0 00-2 2v12a2 2 0 002 2h12a2 2 0 002-2v-5M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"></path></svg>
                  </button>
                  <button
                    class="absolute right-2 top-2.5 p-1.5 rounded-md text-app-text-secondary hover:text-red-500 dark:hover:text-red-400 hover:bg-app-border opacity-0 group-hover:opacity-100 transition-all"
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
            <div class="flex flex-col items-center justify-center py-10 text-app-text-secondary">
              <p class="text-sm">未找到连接</p>
            </div>
          {/if}
        {/if}
    {:else if activeTab === 'history'}
        {#if !$isSidebarCollapsed}
            <div class="px-2 py-1.5 flex justify-between items-center text-xs font-semibold text-app-text-secondary uppercase tracking-wider whitespace-nowrap">
              <span>最近连接</span>
            </div>
        {/if}
        {#if filteredHistory.length > 0}
          {#each filteredHistory as item}
            <div class="group relative">
              <button
                class="w-full text-left flex items-center {$isSidebarCollapsed ? 'justify-center p-2' : 'gap-3 p-2.5'} rounded-lg hover:bg-app-surface transition-colors group-hover:shadow-sm"
                on:click={() => handleConnect(item.connection)}
                title={$isSidebarCollapsed ? `${item.connection.name} - ${formatTimeAgo(item.lastConnected)}` : ''}
              >
                <div class="text-app-text-secondary group-hover:text-green-500 dark:group-hover:text-green-400 transition-colors shrink-0">
                  <ClockIcon className="w-4 h-4" />
                </div>
                {#if !$isSidebarCollapsed}
                  <div class="flex-1 min-w-0">
                    <div class="flex justify-between items-center">
                       <div class="font-medium text-app-text truncate group-hover:text-app-text transition-colors flex items-center gap-2">
                         <span class="truncate">{item.connection.name}</span>
                         <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-app-surface text-app-text-secondary shrink-0">
                           {(item.connection as any).protocol === 'Rdp' ? 'RDP' : 'SSH'}
                         </span>
                       </div>
                       <span class="text-[10px] text-app-text-secondary bg-app-surface px-1.5 py-0.5 rounded-full">
                         {formatTimeAgo(item.lastConnected)}
                       </span>
                    </div>
                    <div class="text-xs text-app-text-secondary truncate mt-0.5 font-mono opacity-80">
                      {#if item.connection.username}{item.connection.username}@{/if}{item.connection.host}
                    </div>
                  </div>
                {/if}
              </button>
            </div>
          {/each}
        {:else}
          {#if !$isSidebarCollapsed}
            <div class="flex flex-col items-center justify-center py-10 text-app-text-secondary">
              <p class="text-sm">无历史记录</p>
            </div>
          {/if}
        {/if}
    {/if}
  </div>

  {#if contextMenu.show}
    <div
      class="fixed z-50 w-48 bg-app-surface border border-app-border rounded-lg shadow-xl py-1"
      style="top: {contextMenu.y}px; left: {contextMenu.x}px"
    >
      {#if contextMenuConnectionRow}
        <button
          class="w-full text-left px-4 py-2 text-sm text-app-text hover:bg-app-bg-hover flex items-center gap-2"
          on:click={() => {
            closeContextMenu();
            handleConnect(contextMenuConnectionRow.connection);
          }}
        >
          <div class="w-4 h-4 flex items-center justify-center">
             <ActivityIcon class="w-3.5 h-3.5" />
          </div>
          连接
        </button>
        {#if activeConnectionIds.has(contextMenuConnectionRow.connection.id)}
          <button
            class="w-full text-left px-4 py-2 text-sm text-app-text hover:bg-app-bg-hover flex items-center gap-2"
            on:click|stopPropagation={() => { closeContextMenu(); handleDisconnect(contextMenuConnectionRow.connection); }}
          >
            <div class="w-4 h-4"></div>
            断开连接
          </button>
          <button
            class="w-full text-left px-4 py-2 text-sm text-app-text hover:bg-app-bg-hover flex items-center gap-2"
            on:click|stopPropagation={(e) => { closeContextMenu(); openMonitor(contextMenuConnectionRow.connection, e as any); }}
          >
            <div class="w-4 h-4 flex items-center justify-center">
               <ActivityIcon class="w-3.5 h-3.5" />
            </div>
            系统监控
          </button>
        {/if}
        <div class="my-1 border-t border-app-border"></div>
        <button
          class="w-full text-left px-4 py-2 text-sm text-app-text hover:bg-app-bg-hover flex items-center gap-2"
          on:click|stopPropagation={(e) => { closeContextMenu(); handleEdit(contextMenuConnectionRow.connection, e as any); }}
        >
          <div class="w-4 h-4 flex items-center justify-center">
            <SettingsIcon class="w-3.5 h-3.5" />
          </div>
          编辑
        </button>
        <button
          class="w-full text-left px-4 py-2 text-sm text-red-500 hover:bg-app-bg-hover flex items-center gap-2"
          on:click|stopPropagation={(e) => { closeContextMenu(); handleDelete(contextMenuConnectionRow.connection.id, e as any); }}
        >
          <div class="w-4 h-4 flex items-center justify-center">
            <TrashIcon class="w-3.5 h-3.5" />
          </div>
          删除
        </button>
      {:else if contextMenuFolderRow}
         <button
          class="w-full text-left px-4 py-2 text-sm text-red-500 hover:bg-app-bg-hover flex items-center gap-2"
          on:click={() => {
            closeContextMenu();
            deleteGroup(contextMenuFolderRow.path);
          }}
        >
          <div class="w-4 h-4 flex items-center justify-center">
            <TrashIcon class="w-3.5 h-3.5" />
          </div>
          删除分组
        </button>
      {:else}
         <button
          class="w-full text-left px-4 py-2 text-sm text-app-text hover:bg-app-bg-hover flex items-center gap-2"
          on:click={() => {
            closeContextMenu();
            editingConnection.set(null);
            showConnectionForm.set(true);
          }}
        >
          <div class="w-4 h-4 flex items-center justify-center">
             <PlusIcon class="w-3.5 h-3.5" />
          </div>
          新建连接
        </button>
         <button
          class="w-full text-left px-4 py-2 text-sm text-app-text hover:bg-app-bg-hover flex items-center gap-2"
          on:click={() => {
            closeContextMenu();
            createNewGroup();
          }}
        >
          <div class="w-4 h-4 flex items-center justify-center">
             <FolderIcon class="w-3.5 h-3.5" />
          </div>
          新建分组
        </button>
      {/if}
    </div>
  {/if}

  <!-- Sidebar Footer -->
  <div class="{$isSidebarCollapsed ? 'p-2' : 'p-4'} border-t border-app-border flex flex-col gap-2">
    <button 
      class="flex items-center {$isSidebarCollapsed ? 'justify-center' : 'gap-2'} text-sm {$isRightSidebarOpen ? 'text-primary-600 dark:text-primary-400 bg-primary-50 dark:bg-primary-900/20' : 'text-app-text-secondary hover:text-app-text hover:bg-app-bg-hover'} transition-colors w-full px-2 py-1.5 rounded-lg"
      on:click={() => isRightSidebarOpen.update(v => !v)}
      title="文件浏览器"
    >
      <FolderIcon class="w-5 h-5" />
      {#if !$isSidebarCollapsed}
        <span>文件</span>
      {/if}
    </button>
    
    <!-- Settings button removed -->
    
    <div class="text-xs text-app-text-secondary text-center mt-1 opacity-50">
      {#if !$isSidebarCollapsed}
        v0.1.0
      {/if}
    </div>
  </div>
</aside>

<style>
  .drop-target.drag-over {
    background-color: rgba(59, 130, 246, 0.1) !important;
    border-color: #3b82f6 !important;
  }
  .cursor-move {
    cursor: move;
  }
</style>
