<script lang="ts">
  import { onMount, onDestroy, createEventDispatcher } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { SearchAddon } from '@xterm/addon-search';
  import { settings, getXtermTheme, type Connection } from '../../lib/store';
  import { 
    initDetachedTerminal, 
    handleTerminalInput, 
    sendTerminalResize
  } from '../../lib/terminalService';

  export let sessionId: string;
  export let connection: Connection;
  export let isRoot: boolean = false;
  // If provided, use existing instance (for root terminal that is already initialized)
  export let existingTerminal: Terminal | null = null;
  export let existingFitAddon: FitAddon | null = null;
  export let existingSearchAddon: SearchAddon | null = null;
  export let onInit: ((term: Terminal, fit: FitAddon, search: SearchAddon) => void) | undefined = undefined;
  export let onFocus: (() => void) | undefined = undefined;
  
  export let isVisible: boolean = true;

  const dispatch = createEventDispatcher<{
    split: { direction: 'horizontal' | 'vertical' };
    close: void;
    active: void;
  }>();
  
  let container: HTMLElement;
  let terminal: Terminal | null = null;
  let fitAddon: FitAddon | null = null;
  let searchAddon: SearchAddon | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let isInitialized = false;
  
  // Search state
  let showSearch = false;
  let searchTerm = '';
  $: searchInputId = `search-input-${sessionId}`;

  // Context Menu state
  let contextMenu = {
    show: false,
    x: 0,
    y: 0
  };

  function isCopyShortcut(e: KeyboardEvent) {
    const key = e.key.toLowerCase();
    if (e.metaKey && key === 'c') return true;
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && key === 'c') return true;
    return false;
  }

  function handlePaste(e: ClipboardEvent) {
    const text = e.clipboardData?.getData('text/plain') ?? '';
    if (!text) return;
    e.preventDefault();
    e.stopPropagation();
    
    if (!sessionId) return;
    handleTerminalInput(sessionId, text, connection);
    setTimeout(() => terminal?.focus(), 0);
  }

  function attachTerminalKeybindings(term: Terminal) {
    term.attachCustomKeyEventHandler((e: KeyboardEvent) => {
      if (e.type === 'keydown' && isCopyShortcut(e)) {
        const selection = term.getSelection() ?? '';
        if (!selection) return true;
        if (!navigator.clipboard?.writeText) return true;
        void navigator.clipboard.writeText(selection);
        return false;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === 'f' && e.type === 'keydown') {
        showSearch = !showSearch;
        if (showSearch) {
          setTimeout(() => document.getElementById(searchInputId)?.focus(), 0);
        } else {
          term.focus();
        }
        return false;
      }
      return true;
    });
  }

  // Initialization
  onMount(async () => {
    resizeObserver = new ResizeObserver(() => {
      if (isVisible && fitAddon && terminal) {
        fitAddon.fit();
        sendTerminalResize(sessionId, terminal.cols, terminal.rows);
      }
    });

    if (existingTerminal) {
      terminal = existingTerminal;
      fitAddon = existingFitAddon;
      searchAddon = existingSearchAddon;
      
      // Ensure terminal is opened in the new container
      if (terminal.element?.parentElement !== container) {
          terminal.open(container);
      }
      
      fitAddon?.fit();
      attachTerminalKeybindings(terminal);
      
      // Re-bind onData listener if needed? 
      // xterm onData listeners persist, but we might want to ensure our input handling is correct.
      // Actually, since we don't dispose the terminal, the listeners attached in initDetachedTerminal or initTerminal are still valid.
      
      if (terminal && fitAddon && searchAddon && onInit) {
        onInit(terminal, fitAddon, searchAddon);
      }
    } else {
      // Initialize new detached terminal
      const result = await initDetachedTerminal(container, sessionId, connection);
      if (result) {
        terminal = result.terminal;
        fitAddon = result.fitAddon;
        searchAddon = result.searchAddon;
        attachTerminalKeybindings(terminal);
        
        if (onInit) {
          onInit(terminal, fitAddon, searchAddon);
        }
      }
    }

    if (terminal) {
        terminal.onTitleChange(() => {
             // Use title change as a proxy for activity/focus if needed, 
             // but better to use onFocus event from xterm textarea
        });
        // We can listen to the textarea focus
        terminal.textarea?.addEventListener('focus', () => {
            if (onFocus) onFocus();
            dispatch('active');
        });
        // Also click on container
        container.addEventListener('mousedown', () => {
             if (onFocus) onFocus();
             dispatch('active');
        });
    }

    if (container) {
      resizeObserver.observe(container);
      container.addEventListener('paste', handlePaste, true);
    }
    
    document.addEventListener('click', handleDocumentClick);
    isInitialized = true;
  });

  onDestroy(async () => {
    if (resizeObserver) {
      resizeObserver.disconnect();
    }
    document.removeEventListener('click', handleDocumentClick);
    container?.removeEventListener('paste', handlePaste, true);
    
    // We NO LONGER dispose/disconnect here. 
    // Session cleanup is now managed explicitly by the parent view or closeSplitSession.
    // This allows the component to be unmounted/remounted during layout changes without killing the session.
  });

  // Reactive settings updates
  $: if (terminal && isInitialized) {
      terminal.options.fontSize = $settings.terminal.fontSize;
      terminal.options.fontFamily = $settings.terminal.fontFamily;
      terminal.options.cursorBlink = $settings.terminal.cursorBlink;
      terminal.options.cursorStyle = $settings.terminal.cursorStyle;
      (terminal.options as any).cursorWidth = 1;
      terminal.options.scrollback = $settings.terminal.scrollback;
      terminal.options.theme = getXtermTheme($settings);
      
      // Force redraw to ensure transparency takes effect
      setTimeout(() => terminal?.refresh(0, terminal.rows - 1), 10);
      
      if (isVisible && fitAddon) {
        setTimeout(() => fitAddon?.fit(), 10);
      }
  }

  $: if (isVisible && isInitialized && fitAddon && terminal) {
      // Re-fit when becoming visible
      setTimeout(() => {
          fitAddon?.fit();
          if (terminal) {
            sendTerminalResize(sessionId, terminal.cols, terminal.rows);
            terminal.focus();
          }
      }, 50);
  }

  // Context Menu Handlers
  function openContextMenu(e: MouseEvent) {
    e.preventDefault();
    contextMenu.x = e.clientX;
    contextMenu.y = e.clientY;
    contextMenu.show = true;
  }

  function closeContextMenu() {
    contextMenu.show = false;
  }

  function handleDocumentClick() {
    if (contextMenu.show) {
      closeContextMenu();
    }
  }

  function handleMenuCopy() {
    if (!terminal) return;
    const selection = terminal.getSelection();
    if (selection && navigator.clipboard?.writeText) {
      navigator.clipboard.writeText(selection);
    }
    closeContextMenu();
  }

  async function handleMenuPaste() {
    try {
      const text = await navigator.clipboard.readText();
      if (text && sessionId) {
        handleTerminalInput(sessionId, text, connection);
      }
    } catch (err) {
      console.error('Failed to paste:', err);
    }
    closeContextMenu();
    setTimeout(() => terminal?.focus(), 0);
  }

  function handleClearScreen() {
    terminal?.clear();
    closeContextMenu();
  }

  function handleSelectAll() {
    terminal?.selectAll();
    closeContextMenu();
  }

  function handleFind() {
    showSearch = true;
    setTimeout(() => document.getElementById(searchInputId)?.focus(), 0);
    closeContextMenu();
  }

  function handleClearScrollback() {
    terminal?.clear();
    closeContextMenu();
  }

  function handleReset() {
    terminal?.reset();
    closeContextMenu();
  }

  function handleSplitHorizontal() {
    dispatch('split', { direction: 'horizontal' });
    closeContextMenu();
  }

  function handleSplitVertical() {
    dispatch('split', { direction: 'vertical' });
    closeContextMenu();
  }

  function handleClosePane() {
    dispatch('close');
    closeContextMenu();
  }

  // Search Handlers
  function handleSearchInput() {
    if (searchAddon) {
      searchAddon.findNext(searchTerm, {
        regex: false,
        wholeWord: false,
        caseSensitive: false,
        incremental: true
      });
    }
  }

  function handleSearchPrevious() {
    searchAddon?.findPrevious(searchTerm, { regex: false, wholeWord: false, caseSensitive: false });
  }

  function handleSearchNext() {
    searchAddon?.findNext(searchTerm, { regex: false, wholeWord: false, caseSensitive: false });
  }

  function closeSearch() {
    showSearch = false;
    terminal?.focus();
  }

  function handleSearchKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      if (e.shiftKey) handleSearchPrevious();
      else handleSearchNext();
    } else if (e.key === 'Escape') {
      closeSearch();
    }
  }
</script>

<div class="relative w-full h-full overflow-hidden group">
  <!-- Background Image Layer -->
  {#if $settings.appearance.backgroundImage}
    <div 
      class="absolute inset-0 z-0 bg-cover bg-center bg-no-repeat pointer-events-none"
      style:background-image="url('{$settings.appearance.backgroundImage}')"
      style:opacity={$settings.appearance.backgroundOpacity ?? 0.5}
      style:filter="blur({$settings.appearance.backgroundBlur ?? 0}px)"
    ></div>
  {/if}

  <div
    bind:this={container}
    class="relative z-0 w-full h-full overflow-hidden"
    on:contextmenu|preventDefault={openContextMenu}
    role="button"
    tabindex="0"
  ></div>

  <!-- Search Bar -->
  {#if showSearch}
    <div class="absolute top-2 right-2 z-10 bg-app-surface border border-app-border shadow-lg rounded-md p-1.5 flex items-center gap-1.5">
      <input 
        id={searchInputId}
        type="text" 
        bind:value={searchTerm} 
        on:input={handleSearchInput}
        on:keydown={handleSearchKeydown}
        placeholder="Find..."
        class="w-48 px-2 py-1 text-xs bg-app-bg border border-app-border rounded text-app-text focus:outline-none focus:border-primary-500"
      />
      <button class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary" aria-label="上一个匹配" on:click={handleSearchPrevious}>
        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7"></path></svg>
      </button>
      <button class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary" aria-label="下一个匹配" on:click={handleSearchNext}>
        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path></svg>
      </button>
      <button class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary" aria-label="关闭查找" on:click={closeSearch}>
        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path></svg>
      </button>
    </div>
  {/if}

  <!-- Context Menu -->
  {#if contextMenu.show}
    <div 
      class="fixed z-50 w-48 bg-app-surface rounded-md shadow-xl border border-app-border py-1 text-sm overflow-hidden"
      style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
    >
      <button class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text" on:click|stopPropagation={handleMenuCopy}>复制</button>
      <button class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text" on:click|stopPropagation={handleMenuPaste}>粘贴</button>
      <div class="border-t border-app-border my-1"></div>
      <button class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text" on:click|stopPropagation={handleClearScreen}>清屏</button>
      <button class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text" on:click|stopPropagation={handleSelectAll}>全选</button>
      <button class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text" on:click|stopPropagation={handleFind}>查找</button>
      <div class="border-t border-app-border my-1"></div>
      <button class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text flex items-center gap-2" on:click|stopPropagation={handleSplitHorizontal}>
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"></path></svg>
        上下分屏
      </button>
      <button class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text flex items-center gap-2" on:click|stopPropagation={handleSplitVertical}>
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 4h13M8 20h13M3 4h.01M3 20h.01"></path></svg>
        左右分屏
      </button>
      {#if !isRoot}
      <button class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-red-600 dark:text-red-400" on:click|stopPropagation={handleClosePane}>
        关闭分屏
      </button>
      {/if}
      <div class="border-t border-app-border my-1"></div>
      <button class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text" on:click|stopPropagation={handleClearScrollback}>清除滚动缓冲区</button>
      <button class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-red-600 dark:text-red-400" on:click|stopPropagation={handleReset}>重置终端</button>
    </div>
  {/if}
</div>
