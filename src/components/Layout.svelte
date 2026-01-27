<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import Sidebar from './Sidebar.svelte';
  import TerminalManager from './TerminalManager.svelte';
  import ConnectionModal from './ConnectionModal.svelte';
  import SettingsModal from './SettingsModal.svelte';
  import CommandPalette from './CommandPalette.svelte';
  import AppLockOverlay from './AppLockOverlay.svelte';
  import AdvancedModal from './AdvancedModal.svelte';
  import { showConnectionForm, editingConnection, showSettings, successMessage, errorMessage, settings, isSidebarCollapsed, activeTerminals, selectedTerminalIndex, showCommandPalette, isLocked, showAdvancedModal } from '../lib/store';
  import { closeAllTerminals, closeTerminal, restoreActiveSessions } from '../lib/terminalService';
  import { loadConnections } from '../lib/connectionService';
  import { fade, fly } from 'svelte/transition';

  let isCheckingLock = true;
  let idleTimer: ReturnType<typeof setTimeout> | null = null;
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

  // Apply theme class to document element
  function updateTheme() {
    const theme = $settings.theme;
    const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    
    if (theme === 'dark' || (theme === 'system' && systemDark)) {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
  }

  onMount(() => {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleSystemThemeChange = () => {
      if ($settings.theme === 'system') {
        updateTheme();
      }
    };
    
    mediaQuery.addEventListener('change', handleSystemThemeChange);

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

    return () => {
        mediaQuery.removeEventListener('change', handleSystemThemeChange);
        window.removeEventListener('blur', handleWindowBlur);
        window.removeEventListener('mousemove', resetIdleTimer);
        window.removeEventListener('keydown', resetIdleTimer);
        window.removeEventListener('click', resetIdleTimer);
        window.removeEventListener('beforeunload', handleBeforeUnload);
        if (idleTimer) clearTimeout(idleTimer);
        if (unlistenKeyboardInteractive) unlistenKeyboardInteractive();
    };
  });

  // React to theme setting changes
  $: $settings.theme, updateTheme();

  async function handleWindowBlur() {
    // Only lock if enabled and not already locked
    if ($settings.security.lockOnBlur && !$isLocked) {
         // Check if app lock is actually enabled on backend
         try {
             const enabled = await invoke('is_app_lock_enabled');
             if (enabled) {
                 isLocked.set(true);
             }
         } catch (e) {
             console.error('Failed to check lock status on blur', e);
         }
    }
  }

  function resetIdleTimer() {
      if (idleTimer) clearTimeout(idleTimer);
      
      const minutes = $settings.security.autoLockMinutes;
      if (minutes > 0 && !$isLocked) {
          idleTimer = setTimeout(async () => {
               // Double check lock status
               try {
                   const enabled = await invoke('is_app_lock_enabled');
                   if (enabled && !$isLocked) {
                       isLocked.set(true);
                   }
               } catch (e) {
                   console.error('Failed to check lock status in idle timer', e);
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

  function checkShortcut(event: KeyboardEvent, shortcut: string): boolean {
    if (!shortcut) return false;
    const parts = shortcut.toLowerCase().split('+');
    const key = parts.pop();
    
    if (!key) return false;

    // Check modifiers
    const ctrl = parts.includes('ctrl') || parts.includes('control');
    const shift = parts.includes('shift');
    const alt = parts.includes('alt') || parts.includes('option');
    const meta = parts.includes('meta') || parts.includes('cmd') || parts.includes('command');

    if (ctrl && !event.ctrlKey) return false;
    if (shift && !event.shiftKey) return false;
    if (alt && !event.altKey) return false;
    if (meta && !event.metaKey) return false;

    // Check key
    // event.code is like 'KeyN', 'BracketLeft'. shortcut key usually is 'n', '[', etc.
    // This is a simple implementation, might need refinement for special keys.
    const eventKey = event.key.toLowerCase();
    if (eventKey === key) return true;

    // Fallback for code matching if key doesn't match directly (e.g. for BracketLeft)
    if (key === '[' && event.code === 'BracketLeft') return true;
    if (key === ']' && event.code === 'BracketRight') return true;

    return false;
  }

  function handleKeydown(event: KeyboardEvent) {
    const shortcuts = $settings.shortcuts;

    // Command Palette
    if (checkShortcut(event, shortcuts.commandPalette)) {
      event.preventDefault();
      showCommandPalette.update(v => !v);
      return;
    }

    if (checkShortcut(event, shortcuts.toggleSidebar)) {
      event.preventDefault();
      isSidebarCollapsed.update(v => !v);
      return;
    }

    // New Connection
    if (checkShortcut(event, shortcuts.newConnection)) {
      event.preventDefault();
      editingConnection.set(null);
      showConnectionForm.set(true);
      return;
    }

    // Settings
    if (checkShortcut(event, shortcuts.settings)) {
      event.preventDefault();
      showSettings.update(v => !v);
      return;
    }

    // Close Current Terminal
    if (checkShortcut(event, shortcuts.closeTerminal)) {
      event.preventDefault();
      if ($activeTerminals.length > 0 && $selectedTerminalIndex >= 0 && $selectedTerminalIndex < $activeTerminals.length) {
         const session = $activeTerminals[$selectedTerminalIndex];
         if (session) closeTerminal(session.sessionId);
      }
      return;
    }

    // Previous Tab
    if (checkShortcut(event, shortcuts.prevTab)) {
      event.preventDefault();
      if ($activeTerminals.length > 1) {
        selectedTerminalIndex.update(idx => (idx - 1 + $activeTerminals.length) % $activeTerminals.length);
      }
      return;
    }

    // Next Tab
    if (checkShortcut(event, shortcuts.nextTab)) {
      event.preventDefault();
      if ($activeTerminals.length > 1) {
        selectedTerminalIndex.update(idx => (idx + 1) % $activeTerminals.length);
      }
      return;
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if isCheckingLock}
  <div class="h-screen w-screen flex items-center justify-center bg-slate-950 text-slate-400">
    <div class="flex flex-col items-center gap-4">
      <div class="w-12 h-12 border-4 border-blue-600 border-t-transparent rounded-full animate-spin"></div>
      <p>Loading...</p>
    </div>
  </div>
{:else}
  <div class="h-screen w-screen flex bg-slate-50 dark:bg-slate-950 text-slate-900 dark:text-slate-200 overflow-hidden font-sans antialiased selection:bg-blue-500/30">
    <Sidebar />
    
    <main class="flex-1 flex flex-col min-w-0 relative">
      <TerminalManager />
      
      <!-- Toast Messages -->
      <div class="fixed top-4 right-4 z-[1000] flex flex-col gap-2 pointer-events-none">
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
    </main>
  </div>
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

{#if $showCommandPalette}
  <CommandPalette />
{/if}

{#if $isLocked}
  <AppLockOverlay />
{/if}

{#if keyboardInteractiveActive}
  <div class="fixed inset-0 z-[60] flex items-center justify-center bg-slate-500/50 dark:bg-slate-900/60 backdrop-blur-sm">
    <div class="w-full max-w-lg rounded-xl border border-slate-200/50 dark:border-slate-800/50 bg-white dark:bg-slate-950 shadow-2xl p-6">
      <div class="flex items-start justify-between gap-4">
        <div class="min-w-0">
          <div class="text-sm font-medium text-slate-900 dark:text-slate-200">
            交互式认证
          </div>
          <div class="text-xs text-slate-500 dark:text-slate-400 mt-1">
            {keyboardInteractiveActive.host}:{keyboardInteractiveActive.port} / {keyboardInteractiveActive.username}
          </div>
        </div>
        <button
          class="text-slate-400 hover:text-slate-600 dark:hover:text-slate-200 transition-colors"
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
            <div class="text-sm text-slate-700 dark:text-slate-300">{keyboardInteractiveActive.name}</div>
          {/if}
          {#if keyboardInteractiveActive.instructions}
            <div class="text-xs text-slate-500 dark:text-slate-400 whitespace-pre-wrap">{keyboardInteractiveActive.instructions}</div>
          {/if}
        </div>
      {/if}

      <div class="mt-4 space-y-3">
        {#each keyboardInteractiveActive.prompts as p, i (i)}
          <div>
            <label
              class="block text-xs font-medium text-slate-600 dark:text-slate-400 mb-1.5"
              for={`keyboard-interactive-${keyboardInteractiveActive.request_id}-${i}`}
            >
              {p.prompt}
            </label>
            <input
              id={`keyboard-interactive-${keyboardInteractiveActive.request_id}-${i}`}
              type={p.echo ? 'text' : 'password'}
              bind:value={keyboardInteractiveResponses[i]}
              class="w-full bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all"
              autocomplete="off"
            />
          </div>
        {/each}
      </div>

      <div class="mt-6 flex items-center justify-end gap-3">
        <button
          class="px-4 py-2 rounded-lg border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-200 hover:bg-slate-50 dark:hover:bg-slate-900 transition-colors disabled:opacity-50"
          on:click={cancelKeyboardInteractive}
          disabled={keyboardInteractiveSubmitting}
        >
          取消
        </button>
        <button
          class="px-4 py-2 rounded-lg bg-blue-600 text-white hover:bg-blue-500 transition-colors disabled:opacity-50"
          on:click={submitKeyboardInteractive}
          disabled={keyboardInteractiveSubmitting}
        >
          确认
        </button>
      </div>
    </div>
  </div>
{/if}
