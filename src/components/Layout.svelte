<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import Sidebar from './Sidebar.svelte';
  import TerminalManager from './TerminalManager.svelte';
  import ConnectionModal from './ConnectionModal.svelte';
  import SettingsModal from './SettingsModal.svelte';
  import CommandPalette from './CommandPalette.svelte';
  import AppLockOverlay from './AppLockOverlay.svelte';
  import AdvancedModal from './AdvancedModal.svelte';
  import { showConnectionForm, showSettings, successMessage, errorMessage, settings, activeTerminals, selectedTerminalIndex, showCommandPalette, isLocked, showAdvancedModal } from '../lib/store';
  import { closeTerminal } from '../lib/terminalService';
  import { fade, fly } from 'svelte/transition';

  let isCheckingLock = true;
  let idleTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(async () => {
    try {
      const enabled = await invoke('is_app_lock_enabled');
      if (enabled) {
        isLocked.set(true);
      }
    } catch (e) {
      console.error('Failed to check app lock status:', e);
    } finally {
      isCheckingLock = false;
    }

    // Event listeners for auto lock
    window.addEventListener('blur', handleWindowBlur);
    window.addEventListener('mousemove', resetIdleTimer);
    window.addEventListener('keydown', resetIdleTimer);
    window.addEventListener('click', resetIdleTimer);
    
    resetIdleTimer();

    return () => {
        window.removeEventListener('blur', handleWindowBlur);
        window.removeEventListener('mousemove', resetIdleTimer);
        window.removeEventListener('keydown', resetIdleTimer);
        window.removeEventListener('click', resetIdleTimer);
        if (idleTimer) clearTimeout(idleTimer);
    };
  });

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
               } catch (e) {}
          }, minutes * 60 * 1000);
      }
  }
  
  // React to settings change to update timer
  $: if ($settings.security.autoLockMinutes) {
      resetIdleTimer();
  }

  // Apply theme class to document element
  $: {
    if ($settings.theme === 'dark') {
      document.documentElement.classList.add('dark');
    } else {
      document.documentElement.classList.remove('dark');
    }
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

    // New Connection
    if (checkShortcut(event, shortcuts.newConnection)) {
      event.preventDefault();
      showConnectionForm.update(v => !v);
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
      <div class="absolute top-4 right-4 z-50 flex flex-col gap-2 pointer-events-none">
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
