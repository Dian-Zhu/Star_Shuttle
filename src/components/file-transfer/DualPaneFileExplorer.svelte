<script lang="ts">
import LocalFileExplorer from './LocalFileExplorer.svelte';
import FileExplorer from './FileExplorer.svelte';

  export let sessionId: string = ''; // SSH session ID for remote panel

  let localPath = '~';
  let remotePath = '.';

  // 本地文件浏览器面板展开状态（默认不展开）
  let isLocalPanelExpanded = false;

  function toggleLocalPanel() {
    isLocalPanelExpanded = !isLocalPanelExpanded;
  }
</script>

<div class="flex flex-col h-full bg-white dark:bg-gray-900 text-slate-900 dark:text-white">
  <!-- Toolbar -->
  <div class="flex items-center p-2 border-b border-slate-200 dark:border-gray-700 space-x-2">
    <button
      class="p-1 hover:bg-slate-200 dark:hover:bg-gray-700 rounded text-slate-600 dark:text-gray-300 transition-colors"
      on:click={toggleLocalPanel}
      title={isLocalPanelExpanded ? "隐藏本地文件浏览器" : "显示本地文件浏览器"}
      aria-label={isLocalPanelExpanded ? "隐藏本地文件浏览器" : "显示本地文件浏览器"}
    >
      {#if isLocalPanelExpanded}
        <!-- 隐藏图标 -->
        <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
          <path fill-rule="evenodd" d="M3 5a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zM3 10a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zM3 15a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1z" clip-rule="evenodd" />
        </svg>
      {:else}
        <!-- 显示图标 -->
        <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
          <path fill-rule="evenodd" d="M3 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zm0 4a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1z" clip-rule="evenodd" />
        </svg>
      {/if}
    </button>
    <span class="text-xs text-slate-500 dark:text-gray-400">
      {isLocalPanelExpanded ? "本地文件浏览器" : "本地文件浏览器（点击展开）"}
    </span>
  </div>

  <!-- Dual Panes -->
  <div class="flex-1 flex overflow-hidden">
    <!-- Local File Panel -->
    {#if isLocalPanelExpanded}
      <div class="flex-1 border-r border-slate-200 dark:border-gray-700 overflow-hidden">
        <div class="h-full">
          <LocalFileExplorer initialPath={localPath} />
        </div>
      </div>
    {/if}

    <!-- Remote File Panel -->
    <div class="flex-1 overflow-hidden">
      <div class="h-full">
        {#if sessionId}
          <FileExplorer {sessionId} initialPath={remotePath} />
        {:else}
          <div class="h-full flex items-center justify-center bg-slate-100/50 dark:bg-gray-800/30">
            <div class="text-center p-8">
              <div class="text-4xl mb-4">🔒</div>
              <h3 class="text-lg font-medium mb-2">未连接到远程服务器</h3>
              <p class="text-sm text-slate-500 dark:text-gray-400 max-w-sm">
                请先建立 SSH 连接以启用远程文件浏览器。
                连接后，远程文件系统将在此显示。
              </p>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>
