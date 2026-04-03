<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Sidebar from './Sidebar.svelte';
  import RightSidebar from './RightSidebar.svelte';
  import AiChatPanel from './ai/AiChatPanel.svelte';
  let isAiPanelOpen = false;
  let aiPanelWidth = 400;
  let aiPanelResizing = false;

  $: aiSessionId = (() => {
    const t = $activeTerminals[$selectedTerminalIndex];
    return t?.sessionId ?? null;
  })();

  function startAiResize() {
    aiPanelResizing = true;
    window.addEventListener('mousemove', handleAiMouseMove);
    window.addEventListener('mouseup', stopAiResize);
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }
  function handleAiMouseMove(e: MouseEvent) {
    if (!aiPanelResizing) return;
    const newWidth = window.innerWidth - e.clientX;
    aiPanelWidth = Math.max(280, Math.min(newWidth, 700));
  }
  function stopAiResize() {
    aiPanelResizing = false;
    window.removeEventListener('mousemove', handleAiMouseMove);
    window.removeEventListener('mouseup', stopAiResize);
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  }
  import TitleBar from './TitleBar.svelte';
  import TerminalManager from './TerminalManager.svelte';
  import ConnectionModal from './ConnectionModal.svelte';
  import SettingsModal from './SettingsModal.svelte';
  import CommandPalette from './CommandPalette.svelte';
  import AppLockOverlay from './AppLockOverlay.svelte';
  import AdvancedModal from './AdvancedModal.svelte';
  import PasswordPromptModal from './PasswordPromptModal.svelte';
  import { showConnectionForm, editingConnection, showSettings, successMessage, errorMessage, settings, isSidebarCollapsed, isRightSidebarOpen, activeTerminals, selectedTerminalIndex, showCommandPalette, isLocked, showAdvancedModal, passwordPromptRequest } from '../lib/store';
  import { closeAllTerminals, disconnectTerminal, restoreActiveSessions, sendTerminalData } from '../lib/terminalService';
  import { loadConnections } from '../lib/connectionService';
  import { themeColors, type ThemeColorKey } from '../lib/themeColors';
  import { isEditableShortcutTarget, matchShortcut } from '../lib/shortcuts';
  import { fade, fly } from 'svelte/transition';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import CloseActionModal from './CloseActionModal.svelte';

  let isCheckingLock = true;
  let idleTimer: ReturnType<typeof setTimeout> | null = null;

  let showCloseModal = false;
  let preventClose = true;
  type KeyboardInteractivePrompt = { prompt: string; echo: boolean };
  type KeyboardInteractivePayload = {
    request_id: string;
    host: string;
    port: number;
    username: string;
    name: string;
    instructions: string;
    prompts: KeyboardInteractivePrompt[];
  };

  let keyboardInteractiveQueue: KeyboardInteractivePayload[] = [];
  let keyboardInteractiveActive: KeyboardInteractivePayload | null = null;
  let keyboardInteractiveResponses: string[] = [];
  let keyboardInteractiveSubmitting = false;
  let unlistenKeyboardInteractive: null | (() => void) = null;

  async function applyAppLock() {
    try {
      const enabled = await invoke<boolean>('is_app_lock_enabled');
      if (!enabled) return;

      await invoke('lock_app');
      isLocked.set(true);
    } catch (e) {
      console.error('Failed to lock app', e);
    }
  }

  function showNextKeyboardInteractive() {
    if (keyboardInteractiveActive) return;
    const next = keyboardInteractiveQueue.shift() ?? null;
    if (!next) return;
    keyboardInteractiveActive = next;
    keyboardInteractiveResponses = new Array(next.prompts.length).fill('');
  }

  async function submitKeyboardInteractive() {
    if (!keyboardInteractiveActive || keyboardInteractiveSubmitting) return;
    keyboardInteractiveSubmitting = true;
    const requestId = keyboardInteractiveActive.request_id;
    const responses = keyboardInteractiveActive.prompts.map((_, i) => keyboardInteractiveResponses[i] ?? '');
    try {
      await invoke('keyboard_interactive_respond', { requestId, responses });
    } finally {
      keyboardInteractiveSubmitting = false;
      keyboardInteractiveActive = null;
      keyboardInteractiveResponses = [];
      showNextKeyboardInteractive();
    }
  }

  async function cancelKeyboardInteractive() {
    if (!keyboardInteractiveActive || keyboardInteractiveSubmitting) return;
    keyboardInteractiveSubmitting = true;
    const requestId = keyboardInteractiveActive.request_id;
    try {
      await invoke('keyboard_interactive_cancel', { requestId });
    } finally {
      keyboardInteractiveSubmitting = false;
      keyboardInteractiveActive = null;
      keyboardInteractiveResponses = [];
      showNextKeyboardInteractive();
    }
  }

  // Helper to convert hex to rgba
  function hexToRgba(hex: string, alpha: number) {
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    return `rgba(${r}, ${g}, ${b}, ${alpha})`;
  }

  // Apply theme class to document element
  function updateTheme() {
    const theme = $settings.theme;
    const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const isDark = theme === 'dark' || (theme === 'system' && systemDark);
    const root = document.documentElement;
    const hasBackgroundImage = Boolean($settings.appearance.backgroundImage);

    // Get Opacity
    // Default to 1 (opaque) if not set. 
    // If background image is present, SettingsModal defaults to 0.5, but here we just read the value.
    // If value is undefined, we use 1 for no-image, and 0.5 for image (matching SettingsModal logic)
    const opacity = $settings.appearance.backgroundOpacity ?? ($settings.appearance.backgroundImage ? 0.5 : 1);

    // 1. Determine Base Background Color
    let bgHex = isDark ? '#0f172a' : '#ffffff';
    let statusBarHex = isDark ? '#1e293b' : '#f1f5f9';

    // Clear custom properties first (except --color-bg which we will overwrite)
    const customProps = [
        '--color-surface', '--color-surface-light', 
        '--color-status-bar', '--color-text', '--color-text-secondary', 
        '--color-border', '--color-border-light',
        '--color-sidebar-border'
    ];
    customProps.forEach(p => root.style.removeProperty(p));
    
    if (theme === 'custom' && $settings.appearance.customUITheme) {
       const custom = $settings.appearance.customUITheme;
       bgHex = custom.backgroundColor;
       statusBarHex = custom.statusBarColor || custom.surfaceColor;
       
       root.style.setProperty('--color-surface', custom.surfaceColor);
       root.style.setProperty('--color-surface-light', custom.surfaceLightColor);
       root.style.setProperty('--color-text', custom.textColor);
       root.style.setProperty('--color-text-secondary', custom.secondaryTextColor);
       root.style.setProperty('--color-border', custom.borderColor);
       root.style.setProperty('--color-border-light', custom.borderLightColor);
       root.style.setProperty('--color-sidebar-border', custom.borderColor);
       
       // Default to dark mode base for custom theme
       root.classList.add('dark');
    } else if (isDark) {
      root.classList.add('dark');
    } else {
      root.classList.remove('dark');
    }

    // 2. Apply --color-bg with transparency
    root.style.setProperty('--color-bg', hexToRgba(bgHex, opacity));
    if (theme === 'custom' || hasBackgroundImage) {
      root.style.setProperty(
        '--color-status-bar',
        hasBackgroundImage ? hexToRgba(statusBarHex, opacity) : statusBarHex
      );
    }
  }

  function updateAccentColor() {
    const colorKey = ($settings.appearance.accentColor || 'blue') as ThemeColorKey;
    const colors = themeColors[colorKey] || themeColors.blue;
    
    const root = document.documentElement;
    Object.entries(colors).forEach(([shade, value]) => {
      root.style.setProperty(`--color-primary-${shade}`, value);
    });
  }

  // React to theme setting changes
  $: $settings.theme, $settings.appearance.customUITheme, updateTheme();
  $: $settings.appearance.accentColor, updateAccentColor();

  onMount(() => {
    const appWindow = getCurrentWindow();
    appWindow.onCloseRequested(async (event) => {
        if (preventClose) {
            event.preventDefault();
            showCloseModal = true;
        }
    });

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleSystemThemeChange = () => {
      if ($settings.theme === 'system') {
        updateTheme();
      }
    };
    
    mediaQuery.addEventListener('change', handleSystemThemeChange);
    
    updateAccentColor(); // Initial accent color application

    (async () => {
      try {
        const enabled = await invoke('is_app_lock_enabled');
        if (enabled) {
          isLocked.set(true);
        }
        unlistenKeyboardInteractive = await listen(
          'ssh-keyboard-interactive-request',
          (event: any) => {
            const payload = event.payload as KeyboardInteractivePayload;
            if (!payload?.request_id) return;
            if (keyboardInteractiveActive) {
              keyboardInteractiveQueue = [...keyboardInteractiveQueue, payload];
              return;
            }
            keyboardInteractiveActive = payload;
            keyboardInteractiveResponses = new Array(payload.prompts?.length ?? 0).fill('');
          }
        );
      } catch (e) {
        console.error('Failed to check app lock status:', e);
      } finally {
        isCheckingLock = false;
      }
    })();

    (async () => {
      try {
        await loadConnections();
        await restoreActiveSessions();
      } catch (e) {
        console.error('Failed to restore sessions:', e);
      }
    })();

    // Event listeners for auto lock
    window.addEventListener('blur', handleWindowBlur);
    window.addEventListener('mousemove', resetIdleTimer);
    window.addEventListener('keydown', resetIdleTimer);
    window.addEventListener('click', resetIdleTimer);
    window.addEventListener('beforeunload', handleBeforeUnload);
    
    resetIdleTimer();
    updateTheme(); // Initial theme application

    // TitleBar AI button listener
    const handleOpenAi = () => { isAiPanelOpen = !isAiPanelOpen; };
    window.addEventListener('titlebar:open-ai', handleOpenAi);
    window.addEventListener('ai:run-command', handleAiRunCommand as EventListener);

    return () => {
        mediaQuery.removeEventListener('change', handleSystemThemeChange);
        window.removeEventListener('blur', handleWindowBlur);
        window.removeEventListener('mousemove', resetIdleTimer);
        window.removeEventListener('keydown', resetIdleTimer);
        window.removeEventListener('click', resetIdleTimer);
        window.removeEventListener('beforeunload', handleBeforeUnload);
        if (idleTimer) clearTimeout(idleTimer);
        if (unlistenKeyboardInteractive) unlistenKeyboardInteractive();
        window.removeEventListener('titlebar:open-ai', handleOpenAi);
        window.removeEventListener('ai:run-command', handleAiRunCommand as EventListener);
    };
  });

  async function handleWindowBlur() {
    // Only lock if enabled and not already locked
    if ($settings.security.lockOnBlur && !$isLocked) {
         await applyAppLock();
    }
  }

  function resetIdleTimer() {
      if (idleTimer) clearTimeout(idleTimer);
      
      const minutes = $settings.security.autoLockMinutes;
      if (minutes > 0 && !$isLocked) {
          idleTimer = setTimeout(async () => {
               if (!$isLocked) {
                   await applyAppLock();
               }
          }, minutes * 60 * 1000);
      }
  }
  
  // React to settings change to update timer
  $: $settings.security.autoLockMinutes, resetIdleTimer();
  $: $isLocked, resetIdleTimer();

  function handleBeforeUnload() {
    void closeAllTerminals();
  }

  async function handleAiRunCommand(event: Event) {
    const customEvent = event as CustomEvent<string>;
    const command = customEvent.detail?.trim();
    if (!command || !aiSessionId) return;
    await sendTerminalData(aiSessionId, `${command}\n`);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (isEditableShortcutTarget(event.target, { allowTerminalTextarea: true })) {
      return;
    }

    if ($settings.security.disableDevToolsShortcuts) {
      const key = event.key.toLowerCase();
      const isWindowsLikeDevTools =
        event.ctrlKey && event.shiftKey && (key === 'i' || key === 'j' || key === 'c');
      const isMacLikeDevTools =
        event.metaKey && event.altKey && (key === 'i' || key === 'j' || key === 'c');

      // Prevent the webview from opening DevTools. For Ctrl+Shift+C / Cmd+Alt+C we only
      // suppress the default inspect-element behavior and still allow app-level shortcut
      // handlers to run.
      if (event.key === 'F12' || isWindowsLikeDevTools || isMacLikeDevTools) {
        event.preventDefault();
        if (key !== 'c') {
          return;
        }
      }
    }

    const shortcuts = $settings.shortcuts;

    // Command Palette
    if (matchShortcut(event, shortcuts.commandPalette)) {
      event.preventDefault();
      showCommandPalette.update(v => !v);
      return;
    }

    if (matchShortcut(event, shortcuts.toggleSidebar)) {
      event.preventDefault();
      isSidebarCollapsed.update(v => !v);
      return;
    }

    // Toggle File Browser
    if (matchShortcut(event, shortcuts.toggleFileBrowser)) {
      event.preventDefault();
      isRightSidebarOpen.update(v => !v);
      return;
    }

    // Toggle AI Panel (Ctrl+Shift+A)
    if (event.ctrlKey && event.shiftKey && event.key === 'A') {
      event.preventDefault();
      isAiPanelOpen = !isAiPanelOpen;
      return;
    }

    // New Connection
    if (matchShortcut(event, shortcuts.newConnection)) {
      event.preventDefault();
      editingConnection.set(null);
      showConnectionForm.set(true);
      return;
    }

    // Settings
    if (matchShortcut(event, shortcuts.settings)) {
      event.preventDefault();
      showSettings.update(v => !v);
      return;
    }

    // Close Current Terminal
    if (matchShortcut(event, shortcuts.closeTerminal)) {
      event.preventDefault();
      if ($activeTerminals.length > 0 && $selectedTerminalIndex >= 0 && $selectedTerminalIndex < $activeTerminals.length) {
         const session = $activeTerminals[$selectedTerminalIndex];
         if (session) void disconnectTerminal(session.sessionId);
      }
      return;
    }

    // Previous Tab
    if (matchShortcut(event, shortcuts.prevTab)) {
      event.preventDefault();
      if ($activeTerminals.length > 1) {
        selectedTerminalIndex.update(idx => (idx - 1 + $activeTerminals.length) % $activeTerminals.length);
      }
      return;
    }

    // Next Tab
    if (matchShortcut(event, shortcuts.nextTab)) {
      event.preventDefault();
      if ($activeTerminals.length > 1) {
        selectedTerminalIndex.update(idx => (idx + 1) % $activeTerminals.length);
      }
      return;
    }
  }
  async function handleMinimizeApp() {
    showCloseModal = false;
    const appWindow = getCurrentWindow();
    await appWindow.hide();
  }

  async function handleQuitApp() {
    showCloseModal = false;
    preventClose = false;
    const appWindow = getCurrentWindow();
    await appWindow.close();
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if isCheckingLock}
  <div class="h-screen w-screen flex items-center justify-center bg-app-bg text-app-text-secondary">
    <div class="flex flex-col items-center gap-4">
      <div class="w-12 h-12 border-4 border-primary-600 border-t-transparent rounded-full animate-spin"></div>
      <p>Loading...</p>
    </div>
  </div>
{:else}
  <div class="h-screen w-screen bg-app-bg text-app-text overflow-hidden font-sans antialiased selection:bg-blue-500/30 relative">
    {#if $settings.appearance.backgroundImage}
      <div
        class="absolute inset-0 z-0 bg-cover bg-center bg-no-repeat pointer-events-none"
        style:background-image="url('{$settings.appearance.backgroundImage}')"
        style:opacity={$settings.appearance.backgroundOpacity ?? 0.5}
        style:filter="blur({$settings.appearance.backgroundBlur ?? 0}px)"
      ></div>
    {/if}

    <TitleBar />

    <div class="relative z-10 flex h-full w-full pt-[35px]">
      <Sidebar />
      
      <main class="flex-1 flex overflow-hidden relative">
        <div class="flex-1 flex flex-col min-w-0 relative">
          <TerminalManager />
          
          <!-- Toast Messages -->
          <div class="fixed top-12 right-4 z-[1000] flex flex-col gap-2 pointer-events-none">
            {#if $successMessage}
              <div 
                transition:fly={{ y: -20, duration: 300 }}
                class="bg-green-500/10 border border-green-500/20 text-green-400 px-4 py-3 rounded-lg shadow-xl backdrop-blur-md flex items-center gap-2 pointer-events-auto min-w-[300px]"
              >
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path></svg>
                <span class="text-sm font-medium">{$successMessage}</span>
              </div>
            {/if}
            
            {#if $errorMessage}
              <div 
                transition:fly={{ y: -20, duration: 300 }}
                class="bg-red-500/10 border border-red-500/20 text-red-400 px-4 py-3 rounded-lg shadow-xl backdrop-blur-md flex items-center gap-2 pointer-events-auto min-w-[300px]"
              >
                <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
                <span class="text-sm font-medium">{$errorMessage}</span>
              </div>
            {/if}
          </div>
        </div>

        {#if isAiPanelOpen}
          <div transition:fly={{ x: 420, duration: 200 }}
               class="h-full flex-shrink-0 z-20 shadow-lg border-l border-app-border bg-app-bg flex flex-col relative"
               style="width: {aiPanelWidth}px; min-width: 280px; max-width: 700px;">
            <!-- Resize handle -->
            <!-- svelte-ignore a11y-no-static-element-interactions -->
            <div class="absolute left-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-primary-500 transition-colors z-50 -ml-0.5"
                 on:mousedown={startAiResize}></div>
            <!-- Header -->
            <div class="flex items-center justify-between px-3 py-2 border-b border-app-border bg-app-surface flex-shrink-0">
              <div class="flex items-center gap-1.5">
                <svg class="w-4 h-4 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
                </svg>
                <span class="text-sm font-semibold text-app-text">AI 助手</span>
              </div>
              <button class="p-1 rounded hover:bg-app-bg-hover text-app-text-secondary hover:text-app-text transition-colors"
                      on:click={() => (isAiPanelOpen = false)} title="关闭">
                <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                </svg>
              </button>
            </div>
            <!-- Content -->
            <div class="flex-1 overflow-hidden">
              <AiChatPanel sessionId={aiSessionId} />
            </div>
          </div>
        {/if}
        {#if $isRightSidebarOpen}
          <div transition:fly={{ x: $settings.ui.rightSidebarWidth || 400, duration: 200 }} class="h-full flex-shrink-0 z-20 shadow-lg">
            <RightSidebar />
          </div>
        {/if}
      </main>
    </div>
  </div>
{/if}

{#if showCloseModal}
  <CloseActionModal 
    on:close={() => showCloseModal = false}
    on:minimize={handleMinimizeApp}
    on:quit={handleQuitApp}
  />
{/if}

{#if $showConnectionForm}
  <div transition:fade={{ duration: 200 }}>
    <ConnectionModal />
  </div>
{/if}

{#if $showSettings}
  <div transition:fade={{ duration: 200 }}>
    <SettingsModal />
  </div>
{/if}

{#if $showAdvancedModal}
  <AdvancedModal />
{/if}

{#if $passwordPromptRequest}
  <PasswordPromptModal />
{/if}

{#if $showCommandPalette}
  <CommandPalette />
{/if}

{#if $isLocked}
  <AppLockOverlay />
{/if}

{#if keyboardInteractiveActive}
  <div class="fixed inset-0 z-[60] flex items-center justify-center bg-app-backdrop backdrop-blur-sm">
    <div class="w-full max-w-lg rounded-xl border border-app-border bg-app-surface shadow-2xl p-6">
      <div class="flex items-start justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm font-medium text-app-text">
            交互式认证
          </div>
          <div class="text-xs text-app-text-secondary mt-1">
            {keyboardInteractiveActive.host}:{keyboardInteractiveActive.port} / {keyboardInteractiveActive.username}
          </div>
        </div>
        <button
          class="text-app-text-secondary hover:text-app-text transition-colors p-1 rounded-md hover:bg-app-bg-hover"
          aria-label="取消"
          on:click={cancelKeyboardInteractive}
          disabled={keyboardInteractiveSubmitting}
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path></svg>
        </button>
      </div>

      {#if keyboardInteractiveActive.name || keyboardInteractiveActive.instructions}
        <div class="mt-4 space-y-1">
          {#if keyboardInteractiveActive.name}
            <div class="text-sm text-app-text">{keyboardInteractiveActive.name}</div>
          {/if}
          {#if keyboardInteractiveActive.instructions}
            <div class="text-xs text-app-text-secondary whitespace-pre-wrap">{keyboardInteractiveActive.instructions}</div>
          {/if}
        </div>
      {/if}

      <div class="mt-4 space-y-3">
        {#each keyboardInteractiveActive.prompts as p, i (i)}
          <div>
            <label
              class="block text-xs font-medium text-app-text-secondary mb-1.5"
              for={`keyboard-interactive-${keyboardInteractiveActive.request_id}-${i}`}
            >
              {p.prompt}
            </label>
            <input
              id={`keyboard-interactive-${keyboardInteractiveActive.request_id}-${i}`}
              type={p.echo ? 'text' : 'password'}
              bind:value={keyboardInteractiveResponses[i]}
              class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
              autocomplete="off"
            />
          </div>
        {/each}
      </div>

      <div class="mt-6 flex items-center justify-end gap-3">
        <button
          class="px-4 py-2 rounded-lg border border-app-border text-app-text-secondary hover:text-app-text bg-app-bg hover:bg-app-bg-hover transition-colors disabled:opacity-50"
          on:click={cancelKeyboardInteractive}
          disabled={keyboardInteractiveSubmitting}
        >
          取消
        </button>
        <button
          class="px-4 py-2 rounded-lg bg-primary-600 text-white hover:bg-primary-500 transition-colors disabled:opacity-50"
          on:click={submitKeyboardInteractive}
          disabled={keyboardInteractiveSubmitting}
        >
          确认
        </button>
      </div>
    </div>
  </div>
{/if}
