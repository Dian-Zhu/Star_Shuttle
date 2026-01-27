<script lang="ts">
import LocalFileExplorer from './LocalFileExplorer.svelte';
import FileExplorer from './FileExplorer.svelte';

  export let sessionId: string = ''; // SSH session ID for remote panel

  let localPath = '~';
  let remotePath = '.';
</script>

<div class="flex flex-col h-full bg-white dark:bg-gray-900 text-slate-900 dark:text-white">
  <!-- Dual Panes -->
  <div class="flex-1 flex overflow-hidden">
    <!-- Local File Panel -->
    <div class="flex-1 border-r border-slate-200 dark:border-gray-700 overflow-hidden">
      <div class="h-full">
        <LocalFileExplorer initialPath={localPath} />
      </div>
    </div>

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
