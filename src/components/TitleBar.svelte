<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { showSettings, activeTerminals, selectedTerminalIndex, broadcastInputEnabled, broadcastSessionIds, terminalSessionMap, showCommandPalette, showConnectionForm, editingConnection, isRightSidebarOpen } from '../lib/store';
  import { disconnectTerminal } from '../lib/terminalService';
  import { startScreenshot } from '../lib/screenshotService';
  import { toggleRecording, isRecording, refreshRecordingState } from '../lib/recordingService';
  import { onMount } from 'svelte';
  import SettingsIcon from './icons/SettingsIcon.svelte';
  import FileBrowserIcon from './icons/FileBrowserIcon.svelte';
  import BroadcastIcon from './icons/BroadcastIcon.svelte';
  import TerminalIcon from './icons/TerminalIcon.svelte';
  import PlusIcon from './icons/PlusIcon.svelte';
  import XIcon from './icons/XIcon.svelte';

  const appWindow = getCurrentWindow();

  // Sync the recording indicator with the backend on load (e.g. after reload).
  onMount(() => {
    void refreshRecordingState();
  });

  $: selectedBroadcastSet = new Set($broadcastSessionIds);
  $: rootSessions = $activeTerminals.filter(t => $terminalSessionMap.has(t.sessionId));

  function handleMouseDown(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      getCurrentWindow().startDragging();
    }
  }

  $: currentActiveRootId = (() => {
    const selected = $activeTerminals[$selectedTerminalIndex];
    if (!selected) return null;
    
    for (const [rootId, group] of $terminalSessionMap) {
      if (group.has(selected.sessionId)) {
        return rootId;
      }
    }
    return null;
  })();

  // Keep track of the last active terminal for each session group
  const lastActiveChild = new Map<string, string>();

  $: if ($activeTerminals[$selectedTerminalIndex] && currentActiveRootId) {
    lastActiveChild.set(currentActiveRootId, $activeTerminals[$selectedTerminalIndex].sessionId);
  }

  function handleMinimize() {
    console.log('Minimize button clicked');
    appWindow.minimize();
  }
  
  function handleMaximize() {
    console.log('Maximize button clicked');
    appWindow.toggleMaximize();
  }
  
  function handleClose() {
    console.log('Close button clicked');
    appWindow.close();
  }

  function handleSessionClick(root: any, event: MouseEvent) {
    if (!root) return;

    // Check if we are already active in this session
    const group = $terminalSessionMap.get(root.sessionId);
    const currentActive = $activeTerminals[$selectedTerminalIndex];
    
    // If clicking on already active session, do nothing unless modifier keys
    if (currentActive && group && group.has(currentActive.sessionId)) {
       // already active
    } else {
       // Switch to last active terminal of this session, or root if none
       let targetId = root.sessionId;
       const lastChild = lastActiveChild.get(root.sessionId);
       if (lastChild) {
         // Verify it still exists
         const exists = $activeTerminals.some(t => t.sessionId === lastChild);
         if (exists) targetId = lastChild;
       }

       const index = $activeTerminals.findIndex(t => t.sessionId === targetId);
       if (index !== -1) {
         $selectedTerminalIndex = index;
       }
    }

    if ($broadcastInputEnabled && (event.ctrlKey || event.metaKey)) {
      const sessionId = root.sessionId;
      if (selectedBroadcastSet.has(sessionId)) {
        broadcastSessionIds.update(ids => ids.filter(id => id !== sessionId));
      } else {
        broadcastSessionIds.update(ids => [...ids, sessionId]);
      }
      return;
    }
  }

  async function handleCloseSession(rootId: string, event: MouseEvent) {
    event.stopPropagation();
    const group = $terminalSessionMap.get(rootId);
    if (!group || group.size === 0) {
      await disconnectTerminal(rootId);
      return;
    }

    // Close non-root sessions first, then close root session.
    const allIds = Array.from(group);
    const childIds = allIds.filter(id => id !== rootId);
    for (const childId of childIds) {
      await disconnectTerminal(childId);
    }
    await disconnectTerminal(rootId);
  }

  function toggleBroadcast() {
    broadcastInputEnabled.update(v => {
      const next = !v;
      if (!next) {
        broadcastSessionIds.set([]);
        return next;
      }

      const current = $activeTerminals[$selectedTerminalIndex];
      if (current) {
        broadcastSessionIds.set([current.sessionId]);
      }
      return next;
    });
  }

  function openCommandPalette() {
    showCommandPalette.set(true);
  }

  function openNewConnection() {
    editingConnection.set(null);
    showConnectionForm.set(true);
  }
</script>

<div class="titlebar" on:mousedown={handleMouseDown} role="button" tabindex="-1">
  <div class="titlebar-app-section">
    <div class="titlebar-icon">
      <svg class="w-5 h-5" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg">
        <path d="M981.5 215.2c-0.1-55.3-30.9-85.3-86.4-85.4-255.6-0.1-511.3-0.1-766.9 0-55.1 0-85.6 30.4-85.7 86.1-0.3 197.2-0.2 394.3 0 591.5 0.1 57 30.5 86.6 87.8 86.7 126.5 0.1 253 0 379.5 0 128.3 0 256.5 0.1 384.8 0 55.9 0 86.9-29.7 87-84.8 0.2-198 0.2-396-0.1-594.1z m-65.4 586.9c0.1 21.3-6.6 26.6-26.9 26.5-125.7-0.8-251.5-0.4-377.2-0.4-124.9 0-249.7-0.5-374.6 0.5-22.7 0.2-29.7-5.7-29.6-29.2 0.9-192.1 0.9-384.2 0-576.2-0.1-22.3 6.9-27.8 28.2-27.8 250.6 0.6 501.2 0.6 751.7 0 21.3-0.1 28.3 5.5 28.2 27.8-0.7 192.9-0.6 385.8 0.2 578.8z" fill="currentColor"></path>
        <path d="M249.5 349.1c11.3-0.3 20 5.5 28 12.4 46.9 40.2 94.1 80.2 140.5 121 23.3 20.4 23.3 38.8 0.2 59.1-47.2 41.3-94.8 82-142.7 122.5-18.5 15.6-37.9 15-50.2-1.3-13.8-18.2-7.9-34.4 7.8-48 34.9-30.2 69.6-60.6 105.2-89.9 11.7-9.6 12.7-15 0.4-25.1-36.3-29.9-71.5-61-107.2-91.6-12.7-10.9-17.2-24.3-10.6-39.7 5.1-12.2 15.1-18.9 28.6-19.4zM635.1 653.7c43.6 0 87.3-0.2 130.9 0.1 26.5 0.2 41.3 12.7 40 33.9-1.5 24.4-17.8 32-39.5 32-89 0-178 0.2-267-0.1-27-0.1-41.4-12-41.5-32.8-0.1-21 14.3-32.9 41.1-33 45.3-0.3 90.6-0.1 136-0.1z" fill="currentColor"></path>
      </svg>
    </div>
    <div class="titlebar-app-name">Star Shuttle</div>
  </div>
  
  <div class="titlebar-tabs no-scrollbar">
    {#each rootSessions as root (root.sessionId)}
      {@const isActive = root.sessionId === currentActiveRootId}
      
      <!-- Session Tab Container -->
      <div 
        class="flex items-center h-[calc(100%-2px)] mt-[2px] rounded-t-md transition-colors group/session
        {isActive ? 'bg-transparent border-t border-x border-b-0 border-app-border' : 'border border-transparent hover:bg-app-surface-light/50'}"
      >
          <button
            class="group/term flex items-center gap-1.5 px-2 py-0.5 max-w-[150px] text-xs transition-colors relative h-full rounded-md mx-0.5
            {isActive 
              ? 'text-primary-600 dark:text-primary-400 font-medium' 
              : 'text-app-text-secondary hover:text-app-text'}"
            on:click={(e) => handleSessionClick(root, e)}
            title={root.connection.name}
          >
            {#if $broadcastInputEnabled && selectedBroadcastSet.has(root.sessionId)}
              <div class="absolute inset-y-1 left-0.5 w-0.5 rounded bg-primary-500"></div>
            {/if}
            
            <TerminalIcon class="w-3.5 h-3.5 opacity-70 flex-shrink-0" />
            <span class="truncate">{root.connection.name}</span>
          </button>

        <!-- Session Close Button -->
        <button
          class="opacity-0 group-hover/session:opacity-100 ml-1 mr-1 p-1 rounded-md hover:bg-red-500 hover:text-white text-app-text-secondary transition-all"
          on:click={(e) => handleCloseSession(root.sessionId, e)}
          title="关闭会话"
        >
          <XIcon class="w-3.5 h-3.5" />
        </button>
      </div>
    {/each}
  </div>
  
  <div class="titlebar-drag-region" data-tauri-drag-region></div>
  
  <div class="titlebar-controls">
    <button
      class="quick-action-btn"
      on:click={openCommandPalette}
      title="命令面板"
    >
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-4.35-4.35M10.5 18a7.5 7.5 0 100-15 7.5 7.5 0 000 15z" />
      </svg>
    </button>
    <button
      class="quick-action-btn"
      on:click={openNewConnection}
      title="新建连接"
    >
      <PlusIcon class="w-4 h-4" />
    </button>
    <button
      class="broadcast-btn {$broadcastInputEnabled ? 'text-primary-500' : ''}"
      on:click={toggleBroadcast}
      title="广播输入：Ctrl/⌘ 点击 Tab 选择多个会话{$broadcastInputEnabled ? `（当前: ${Math.max($broadcastSessionIds.length, 1)}）` : ''}"
    >
      <div class="relative">
        <BroadcastIcon className="w-4 h-4" />
        {#if $broadcastInputEnabled}
           <span class="absolute -top-1 -right-1 flex h-3 w-3 items-center justify-center rounded-full bg-red-500 text-[8px] font-bold text-white shadow-sm">
             {Math.max($broadcastSessionIds.length, 1)}
           </span>
        {/if}
      </div>
    </button>
    <!-- 截图按钮 -->
    <button
      class="quick-action-btn"
      on:click={() => startScreenshot()}
      title="截图 (框选后可钉在屏幕上)"
    >
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M3 9a2 2 0 012-2h.93a2 2 0 001.664-.89l.812-1.22A2 2 0 0110.07 4h3.86a2 2 0 011.664.89l.812 1.22A2 2 0 0018.07 7H19a2 2 0 012 2v9a2 2 0 01-2 2H5a2 2 0 01-2-2V9z" />
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M15 13a3 3 0 11-6 0 3 3 0 016 0z" />
      </svg>
    </button>
    <!-- 录屏按钮 -->
    <button
      class="quick-action-btn {$isRecording ? 'recording-active' : ''}"
      on:click={() => toggleRecording()}
      title={$isRecording ? '停止录屏并保存' : '录屏（录制本窗口）'}
    >
      {#if $isRecording}
        <span class="rec-dot"></span>
      {:else}
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <circle cx="12" cy="12" r="7" stroke-width="2" />
          <circle cx="12" cy="12" r="3" fill="currentColor" stroke="none" />
        </svg>
      {/if}
    </button>
    <!-- AI 助手按钮 -->
    <button
      class="ai-btn"
      on:click={() => window.dispatchEvent(new CustomEvent('titlebar:open-ai'))}
      title="AI 助手"
    >
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
      </svg>
    </button>
    <button
      class="file-browser-btn {$isRightSidebarOpen ? 'active' : ''}"
      on:click={() => isRightSidebarOpen.update((v) => !v)}
      title="文件浏览器"
    >
      <FileBrowserIcon class="w-4 h-4" />
    </button>
    <button on:click={() => showSettings.set(true)} title="设置" class="settings-btn">
      <SettingsIcon class="w-4 h-4" />
    </button>
    <button on:click={handleMinimize} title="最小化" class="minimize-btn">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path fill="currentColor" d="M19 13H5v-2h14z" />
      </svg>
    </button>
    <button on:click={handleMaximize} title="最大化/还原" class="maximize-btn">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path fill="currentColor" d="M4 4h16v16H4zm2 4v10h12V8z" />
      </svg>
    </button>
    <button on:click={handleClose} title="关闭" class="close-btn">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path fill="currentColor" d="M13.46 12L19 17.54V19h-1.46L12 13.46L6.46 19H5v-1.46L10.54 12L5 6.46V5h1.46L12 10.54L17.54 5H19v1.46z" />
      </svg>
    </button>
  </div>
</div>

<style>
  .titlebar {
    height: 35px;
    background: var(--color-bg);
    user-select: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 1000;
    color: var(--color-text-secondary);
    -webkit-app-region: drag;
    pointer-events: none;
  }
  
  .titlebar-app-section {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 8px;
    height: 100%;
    -webkit-app-region: drag;
    pointer-events: auto;
  }
  
  .titlebar-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-secondary);
  }
  
  .titlebar-app-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text);
    white-space: nowrap;
  }
  
  .titlebar-tabs {
    display: flex;
    align-items: center;
    overflow-x: auto;
    height: 100%;
    margin-left: 10px;
    margin-right: 10px;
    -webkit-app-region: no-drag;
    pointer-events: auto;
    flex: 0 1 auto;
    max-width: 60%;
    gap: 3px; /* Gap between session tabs */
  }

  .titlebar-tabs::-webkit-scrollbar {
    display: none;
  }
  
  .titlebar-tabs {
    -ms-overflow-style: none;
    scrollbar-width: none;
  }

  .titlebar-drag-region {
    flex: 1;
    height: 100%;
    -webkit-app-region: drag;
    cursor: move;
    pointer-events: auto;
  }
  
  .titlebar-controls {
    display: flex;
    height: 100%;
    -webkit-app-region: no-drag;
  }
  
  .titlebar-controls button {
    appearance: none;
    padding: 0;
    margin: 0;
    border: none;
    display: inline-flex;
    justify-content: center;
    align-items: center;
    width: 30px;
    background-color: transparent;
    color: var(--color-text-secondary);
    transition: all 0.15s ease;
    cursor: pointer;
    pointer-events: auto;
  }

  .titlebar-controls .quick-action-btn {
    width: 34px;
  }

  /* File browser button reflects the right-sidebar open state. */
  .titlebar-controls .file-browser-btn.active {
    color: var(--color-primary, #3b82f6);
  }
  
  .titlebar-controls button:hover {
    background: var(--color-surface-light);
    color: var(--color-text);
  }
  
  .close-btn:hover {
    background: #ef4444 !important;
    color: white !important;
  }

  /* Recording state: red tint on the button + pulsing stop indicator. */
  .titlebar-controls .quick-action-btn.recording-active {
    color: #ef4444;
  }

  .rec-dot {
    display: inline-block;
    width: 12px;
    height: 12px;
    border-radius: 3px;
    background: #ef4444;
    animation: rec-pulse 1.2s ease-in-out infinite;
  }

  @keyframes rec-pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }
</style>
