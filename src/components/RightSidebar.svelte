<script lang="ts">
  import { activeTerminals, selectedTerminalIndex, settings } from '../lib/store';
  import FileExplorer from './file-transfer/FileExplorer.svelte';

  let width = $settings.ui.rightSidebarWidth || 400;
  let isResizing = false;

  $: if (!isResizing && $settings.ui.rightSidebarWidth) {
    width = $settings.ui.rightSidebarWidth;
  }

  $: activeSession = $activeTerminals.length > 0 && $selectedTerminalIndex >= 0
    ? $activeTerminals[$selectedTerminalIndex]
    : null;

  function startResize() {
    isResizing = true;
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('mouseup', stopResize);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  function handleMouseMove(e: MouseEvent) {
    if (!isResizing) return;
    const newWidth = window.innerWidth - e.clientX;
    const maxWidth = Math.min(800, window.innerWidth * 0.8);
    width = Math.max(280, Math.min(newWidth, maxWidth));
  }

  function stopResize() {
    isResizing = false;
    window.removeEventListener('mousemove', handleMouseMove);
    window.removeEventListener('mouseup', stopResize);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    settings.update(s => ({
      ...s,
      ui: { ...s.ui, rightSidebarWidth: width },
    }));
  }
</script>

<div
  class="h-full border-l border-app-border bg-app-bg flex flex-col relative"
  style="width: {width}px; min-width: 280px;"
>
  <!-- Resize Handle -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div
    class="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-primary-500 active:bg-primary-600 transition-colors z-50 -ml-0.5"
    on:mousedown={startResize}
  ></div>

  <!-- Header -->
  <div class="flex items-center gap-1.5 px-3 py-2.5 border-b border-app-border bg-app-surface flex-shrink-0">
    <svg class="w-4 h-4 text-app-text-secondary" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
        d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V7z" />
    </svg>
    <span class="text-sm font-medium text-app-text">文件浏览器</span>
  </div>

  <!-- File Explorer -->
  <div class="flex-1 overflow-hidden">
    {#if activeSession}
      <FileExplorer sessionId={activeSession.sessionId} />
    {:else}
      <div class="h-full flex items-center justify-center p-4 text-app-text-secondary text-sm text-center select-none">
        请选择或连接一个终端<br>以使用文件浏览器
      </div>
    {/if}
  </div>
</div>
