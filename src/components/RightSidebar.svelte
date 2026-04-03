<script lang="ts">
  import { activeTerminals, selectedTerminalIndex, settings } from '../lib/store';
  import FileExplorer from './file-transfer/FileExplorer.svelte';
  import AiChatPanel from './ai/AiChatPanel.svelte';

  let width = $settings.ui.rightSidebarWidth || 400;
  let isResizing = false;
  let activeTab: 'files' | 'ai' = 'ai';

  // Sync width from settings only when not resizing
  $: if (!isResizing && $settings.ui.rightSidebarWidth) {
    width = $settings.ui.rightSidebarWidth;
  }

  $: activeSession = $activeTerminals.length > 0 && $selectedTerminalIndex >= 0
    ? $activeTerminals[$selectedTerminalIndex]
    : null;

  $: sessionId = activeSession?.sessionId ?? null;

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

  // Allow external code to switch to AI tab (e.g. Ctrl+Shift+A)
  export function switchToAi() {
    activeTab = 'ai';
  }
  export function switchToFiles() {
    activeTab = 'files';
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

  <!-- Tab Bar -->
  <div class="flex items-center border-b border-app-border bg-app-surface flex-shrink-0 px-1 pt-1 gap-0.5">
    <button
      class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-t-md transition-colors
        {activeTab === 'ai'
          ? 'bg-app-bg text-app-text border border-b-0 border-app-border -mb-px'
          : 'text-app-text-secondary hover:text-app-text hover:bg-app-bg-hover'}"
      on:click={() => (activeTab = 'ai')}
    >
      <!-- Sparkle icon -->
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
      </svg>
      AI 助手
    </button>

    <button
      class="flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium rounded-t-md transition-colors
        {activeTab === 'files'
          ? 'bg-app-bg text-app-text border border-b-0 border-app-border -mb-px'
          : 'text-app-text-secondary hover:text-app-text hover:bg-app-bg-hover'}"
      on:click={() => (activeTab = 'files')}
    >
      <!-- Folder icon -->
      <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M3 7a2 2 0 012-2h4l2 2h8a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V7z" />
      </svg>
      文件浏览器
    </button>
  </div>

  <!-- Tab Content -->
  <div class="flex-1 overflow-hidden relative">
    <!-- AI Chat Panel -->
    <div class="absolute inset-0 {activeTab === 'ai' ? 'flex flex-col' : 'hidden'}">
      <AiChatPanel {sessionId} />
    </div>

    <!-- File Explorer -->
    <div class="absolute inset-0 {activeTab === 'files' ? 'flex flex-col' : 'hidden'}">
      {#if activeSession}
        <FileExplorer sessionId={activeSession.sessionId} />
      {:else}
        <div class="h-full flex items-center justify-center p-4 text-app-text-secondary text-sm text-center select-none">
          请选择或连接一个终端<br>以使用文件浏览器
        </div>
      {/if}
    </div>
  </div>
</div>
