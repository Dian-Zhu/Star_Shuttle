<script lang="ts">
  import { fly } from 'svelte/transition';
  import {
    uploadTasks,
    removeUploadTask,
    clearFinishedUploads,
    type UploadTask,
  } from '../../lib/uploadManager';

  let collapsed = false;

  function formatSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }

  function percent(task: UploadTask): number {
    if (task.status === 'success') return 100;
    if (task.total <= 0) return task.status === 'uploading' ? 0 : 100;
    return Math.min(100, Math.round((task.transferred / task.total) * 100));
  }

  $: tasks = $uploadTasks;
  $: activeCount = tasks.filter((t) => t.status === 'uploading').length;
  $: hasFinished = tasks.some((t) => t.status !== 'uploading');
</script>

{#if tasks.length > 0}
  <div
    transition:fly={{ y: 20, duration: 200 }}
    class="pointer-events-auto fixed bottom-4 right-4 z-[1100] w-80 rounded-xl border border-white/10 bg-app-surface/70 shadow-2xl backdrop-blur-md overflow-hidden"
  >
    <!-- 头部 -->
    <div class="flex items-center justify-between px-3 py-2 border-b border-app-border/40 text-xs text-app-text">
      <div class="flex items-center gap-2 font-medium">
        {#if activeCount > 0}
          <div class="animate-spin rounded-full h-3.5 w-3.5 border-b-2 border-primary-500"></div>
          <span>正在上传 {activeCount} 个文件</span>
        {:else}
          <svg class="w-3.5 h-3.5 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
          </svg>
          <span>上传完成</span>
        {/if}
      </div>
      <div class="flex items-center gap-1">
        {#if hasFinished}
          <button
            class="px-1.5 py-0.5 rounded hover:bg-app-bg-hover text-app-text-secondary hover:text-app-text transition-colors"
            on:click={clearFinishedUploads}
            title="清除已完成"
          >清除</button>
        {/if}
        <button
          class="p-1 rounded hover:bg-app-bg-hover text-app-text-secondary hover:text-app-text transition-colors"
          on:click={() => (collapsed = !collapsed)}
          title={collapsed ? '展开' : '收起'}
        >
          <svg class="w-3.5 h-3.5 transition-transform {collapsed ? '' : 'rotate-180'}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
          </svg>
        </button>
      </div>
    </div>

    <!-- 任务列表 -->
    {#if !collapsed}
      <div class="max-h-64 overflow-y-auto divide-y divide-app-border/30">
        {#each tasks as task (task.id)}
          <div class="px-3 py-2 text-xs">
            <div class="flex items-center justify-between gap-2">
              <span class="truncate text-app-text flex-1" title={task.fileName}>{task.fileName}</span>
              <div class="flex items-center gap-1.5 flex-none">
                {#if task.status === 'uploading'}
                  <span class="text-app-text-secondary font-mono">{percent(task)}%</span>
                {:else if task.status === 'success'}
                  <svg class="w-3.5 h-3.5 text-green-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
                  </svg>
                {:else}
                  <svg class="w-3.5 h-3.5 text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                {/if}
                <button
                  class="p-0.5 rounded hover:bg-app-bg-hover text-app-text-secondary hover:text-app-text transition-colors"
                  on:click={() => removeUploadTask(task.id)}
                  title="移除"
                >
                  <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            </div>

            <!-- 进度条 -->
            <div class="mt-1.5 h-1 w-full rounded-full bg-app-bg overflow-hidden">
              <div
                class="h-full rounded-full transition-all duration-200
                  {task.status === 'error' ? 'bg-red-500' : task.status === 'success' ? 'bg-green-500' : 'bg-primary-500'}"
                style:width="{percent(task)}%"
              ></div>
            </div>

            <div class="mt-1 flex items-center justify-between text-[10px] text-app-text-secondary">
              {#if task.status === 'error'}
                <span class="text-red-500 truncate" title={task.error}>{task.error}</span>
              {:else}
                <span class="font-mono">
                  {formatSize(task.transferred)}{#if task.total > 0} / {formatSize(task.total)}{/if}
                </span>
                <span class="truncate ml-2 opacity-70" title={task.targetPath}>{task.targetPath}</span>
              {/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}
