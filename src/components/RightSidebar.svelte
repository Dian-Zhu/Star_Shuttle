<script lang="ts">
  import { activeTerminals, selectedTerminalIndex, settings } from '../lib/store';
  import FileExplorer from './file-transfer/FileExplorer.svelte';
  
  let width = $settings.ui.rightSidebarWidth || 400;
  let isResizing = false;

  // Sync width from settings only when not resizing
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
    // Add global cursor style to body to prevent cursor flickering
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  function handleMouseMove(e: MouseEvent) {
    if (!isResizing) return;
    // Calculate width based on distance from right edge of window
    const newWidth = window.innerWidth - e.clientX;
    // Clamp width between 250px and 800px (or 80% of window width)
    const maxWidth = Math.min(800, window.innerWidth * 0.8);
    width = Math.max(250, Math.min(newWidth, maxWidth));
  }

  function stopResize() {
    isResizing = false;
    window.removeEventListener('mousemove', handleMouseMove);
    window.removeEventListener('mouseup', stopResize);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    
    // Save to settings
    settings.update(s => ({
        ...s,
        ui: { ...s.ui, rightSidebarWidth: width }
    }));
  }
</script>

<div 
  class="h-full border-l border-app-border bg-app-bg flex flex-col relative"
  style="width: {width}px; min-width: 250px;"
>
  <!-- Resize Handle -->
  <!-- svelte-ignore a11y-no-static-element-interactions -->
  <div
    class="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-primary-500 active:bg-primary-600 transition-colors z-50 -ml-0.5"
    on:mousedown={startResize}
  ></div>

  {#if activeSession}
    <FileExplorer sessionId={activeSession.sessionId} />
  {:else}
    <div class="h-full flex items-center justify-center p-4 text-app-text-secondary text-sm text-center select-none">
      请选择或连接一个终端<br>以使用文件浏览器
    </div>
  {/if}
</div>
