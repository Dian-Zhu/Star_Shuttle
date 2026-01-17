<script lang="ts">
import LocalFileExplorer from './LocalFileExplorer.svelte';
import FileExplorer from './FileExplorer.svelte';
import { successMessage } from '../../lib/store';
import { transferQueueService, transfers, isTransferring } from '../../lib/transferQueueService';

  export let sessionId: string = ''; // SSH session ID for remote panel

  let localPath = '.';
  let remotePath = '.';


  // sessionId must be provided via prop or derived from context





  // Queue a transfer (add to global queue)
  function queueTransfer(type: 'upload' | 'download', localPath: string, remotePath: string, size: number) {
    transferQueueService.addTransfer(type, sessionId, localPath, remotePath, size);
  }

  // Cancel current transfer
  function cancelTransfer() {
    // Cancel all active transfers (simplified)
    $transfers.active.forEach(t => transferQueueService.cancelTransfer(t.id));
    successMessage.set('传输已取消');
    setTimeout(() => successMessage.set(null), 3000);
  }

  // Clear queue
  function clearQueue() {
    transferQueueService.clearQueue();
    successMessage.set('传输队列已清空');
    setTimeout(() => successMessage.set(null), 3000);
  }
</script>

<div class="flex flex-col h-full bg-gray-900 text-white">
  <!-- Header -->
  <div class="p-4 border-b border-gray-700">
    <h2 class="text-xl font-bold">双面板文件浏览器</h2>
    <p class="text-sm text-gray-400 mt-1">本地文件系统 ↔ 远程 SSH 文件系统</p>
  </div>

  <!-- Transfer Controls -->
  <div class="p-4 border-b border-gray-700 bg-gray-800/50">
    <div class="flex items-center justify-between">
      <div class="flex items-center space-x-4">
        <button 
          class="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded text-sm font-medium"
          on:click={() => {
            // Example: Upload from local to remote (needs selection logic)
            // For demo, add a dummy transfer
            queueTransfer('upload', '/tmp/local.txt', '/tmp/remote.txt', 1024);
            successMessage.set('已添加上传任务到队列');
            setTimeout(() => successMessage.set(null), 3000);
          }}
        >
          上传到远程
        </button>
        <button 
          class="px-4 py-2 bg-green-600 hover:bg-green-700 rounded text-sm font-medium"
          on:click={() => {
            // Example: Download from remote to local
            // For demo, add a dummy transfer
            queueTransfer('download', '/tmp/local.txt', '/tmp/remote.txt', 2048);
            successMessage.set('已添加下载任务到队列');
            setTimeout(() => successMessage.set(null), 3000);
          }}
        >
          下载到本地
        </button>
        <button 
          class="px-4 py-2 bg-red-600 hover:bg-red-700 rounded text-sm font-medium"
          on:click={cancelTransfer}
          disabled={!$isTransferring}
          class:opacity-50={!$isTransferring}
        >
          取消传输
        </button>
        <button 
          class="px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded text-sm font-medium"
          on:click={clearQueue}
          disabled={$transfers.queue.length === 0}
          class:opacity-50={$transfers.queue.length === 0}
        >
          清空队列
        </button>
      </div>

      <div class="text-sm text-gray-300">
        {#if $isTransferring}
          {#each $transfers.active as transfer (transfer.id)}
            <div class="flex items-center space-x-2">
              <span>传输中: {transfer.type === 'upload' ? '上传' : '下载'} {transfer.localPath} → {transfer.remotePath}</span>
              <div class="w-32 h-2 bg-gray-700 rounded overflow-hidden">
                <div 
                  class="h-full bg-blue-500 transition-all duration-300" 
                  style="width: {transfer.progress}%"
                ></div>
              </div>
              <span>{transfer.progress}%</span>
            </div>
          {/each}
        {:else if $transfers.queue.length > 0}
          <span>队列等待: {$transfers.queue.length} 个文件</span>
        {:else}
          <span class="text-gray-500">空闲</span>
        {/if}
      </div>
    </div>
  </div>

  <!-- Dual Panes -->
  <div class="flex-1 flex overflow-hidden">
    <!-- Local File Panel -->
    <div class="flex-1 border-r border-gray-700 overflow-hidden">
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
          <div class="h-full flex items-center justify-center bg-gray-800/30">
            <div class="text-center p-8">
              <div class="text-4xl mb-4">🔒</div>
              <h3 class="text-lg font-medium mb-2">未连接到远程服务器</h3>
              <p class="text-sm text-gray-400 max-w-sm">
                请先建立 SSH 连接以启用远程文件浏览器。
                连接后，远程文件系统将在此显示。
              </p>
            </div>
          </div>
        {/if}
      </div>
    </div>
  </div>

  <!-- Footer Status -->
  <div class="p-3 border-t border-gray-700 bg-gray-800 text-sm text-gray-400 flex justify-between">
    <div>
      本地: <span class="text-gray-300">{localPath}</span>
    </div>
    <div>
      远程: <span class="text-gray-300">{remotePath}</span>
    </div>
  </div>
</div>