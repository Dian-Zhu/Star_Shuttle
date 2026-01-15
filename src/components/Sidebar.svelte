<script lang="ts">
  import { onMount } from 'svelte';
  import { connections, showConnectionForm } from '../lib/store';
  import { loadConnections, deleteConnection } from '../lib/connectionService';
  import { connectAndOpen } from '../lib/terminalService';
  import PlusIcon from './icons/PlusIcon.svelte';
  import ServerIcon from './icons/ServerIcon.svelte';
  import TrashIcon from './icons/TrashIcon.svelte';
  import SettingsIcon from './icons/SettingsIcon.svelte';

  let searchTerm = '';
  
  // Filter connections based on search term
  $: filteredConnections = $connections.filter(c => 
    c.name.toLowerCase().includes(searchTerm.toLowerCase()) || 
    c.host.toLowerCase().includes(searchTerm.toLowerCase())
  );

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
</script>

<aside class="w-64 flex flex-col bg-slate-900 border-r border-slate-800 text-slate-300">
  <!-- Sidebar Header -->
  <div class="p-4 border-b border-slate-800">
    <div class="flex items-center gap-3 mb-4">
      <div class="w-8 h-8 bg-blue-600 rounded-lg flex items-center justify-center shadow-lg shadow-blue-900/20">
        <span class="font-bold text-xs text-white">SSH</span>
      </div>
      <h1 class="text-base font-bold text-slate-100 tracking-wide">
        Remote Manager
      </h1>
    </div>
    
    <button
      class="w-full flex items-center justify-center gap-2 bg-blue-600 hover:bg-blue-500 text-white py-2 px-3 rounded-lg font-medium transition-all shadow-md hover:shadow-blue-900/30 active:scale-95"
      on:click={() => showConnectionForm.set(true)}
    >
      <PlusIcon class="w-4 h-4" />
      <span>新建连接</span>
    </button>
  </div>

  <!-- Search -->
  <div class="px-4 py-3">
    <div class="relative">
      <input
        type="text"
        placeholder="搜索连接..."
        bind:value={searchTerm}
        class="w-full bg-slate-950 border border-slate-800 rounded-lg py-1.5 px-3 pl-9 text-sm text-slate-200 placeholder-slate-500 focus:outline-none focus:border-blue-500/50 focus:ring-1 focus:ring-blue-500/50 transition-all"
      />
      <svg class="absolute left-3 top-2 w-4 h-4 text-slate-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"></path>
      </svg>
    </div>
  </div>

  <!-- Connection List -->
  <div class="flex-1 overflow-y-auto px-2 py-2 space-y-0.5 custom-scrollbar">
    {#if filteredConnections.length > 0}
      <div class="px-2 py-1.5 text-xs font-semibold text-slate-500 uppercase tracking-wider">
        服务器列表
      </div>
      {#each filteredConnections as connection}
        <div class="group relative">
          <button
            class="w-full text-left flex items-start gap-3 p-2.5 rounded-lg hover:bg-slate-800 transition-colors group-hover:shadow-sm"
            on:click={() => handleConnect(connection)}
          >
            <div class="mt-0.5 text-slate-400 group-hover:text-blue-400 transition-colors">
              <ServerIcon class="w-4 h-4" />
            </div>
            <div class="flex-1 min-w-0">
              <div class="font-medium text-slate-200 truncate group-hover:text-white transition-colors">
                {connection.name}
              </div>
              <div class="text-xs text-slate-500 truncate mt-0.5 font-mono opacity-80">
                {connection.username}@{connection.host}
              </div>
            </div>
          </button>
          
          <button
            class="absolute right-2 top-2.5 p-1.5 rounded-md text-slate-500 hover:text-red-400 hover:bg-slate-700/50 opacity-0 group-hover:opacity-100 transition-all"
            on:click={(e) => handleDelete(connection.id, e)}
            title="删除连接"
          >
            <TrashIcon class="w-3.5 h-3.5" />
          </button>
        </div>
      {/each}
    {:else}
      <div class="flex flex-col items-center justify-center py-10 text-slate-500">
        <p class="text-sm">未找到连接</p>
      </div>
    {/if}
  </div>

  <!-- Sidebar Footer -->
  <div class="p-4 border-t border-slate-800">
    <button class="flex items-center gap-2 text-sm text-slate-400 hover:text-slate-200 transition-colors w-full px-2 py-1.5 rounded-lg hover:bg-slate-800">
      <SettingsIcon class="w-4 h-4" />
      <span>设置</span>
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
