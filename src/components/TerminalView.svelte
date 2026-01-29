<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { handleTerminalInput, initTerminal, sendTerminalResize, setupTerminalListeners } from '../lib/terminalService';
  import { getXtermTheme, settings, type ActiveTerminal } from '../lib/store';
  import DualPaneFileExplorer from './file-transfer/DualPaneFileExplorer.svelte';
  import { FitAddon } from 'xterm-addon-fit';
  import { SearchAddon } from 'xterm-addon-search';
  import { Terminal } from 'xterm';

  // Props using Svelte 4 syntax for compatibility
  export let terminalData: ActiveTerminal;
  export let isVisible: boolean = false;

  let container: HTMLElement;
  let mode: 'terminal' | 'sftp' = 'terminal';
  let showSearch = false;
  let searchTerm = '';
  let resizeObserver: ResizeObserver;
  let splitContainer: HTMLElement;
  let primaryContainerRef: HTMLElement | null = null;
  let secondaryContainerRef: HTMLElement | null = null;

  // 右键菜单状态
  let contextMenu = {
    show: false,
    x: 0,
    y: 0
  };

  // 分屏状态
  let splitMode: 'none' | 'horizontal' | 'vertical' = 'none';
  let splitRatio = 0.5;
  let isResizing = false;
  let activePane: 'primary' | 'secondary' = 'primary';

  // 副面板终端
  let secondaryContainer: HTMLElement;
  let secondaryTerminal: Terminal | null = null;
  let secondaryFitAddon: FitAddon | null = null;
  let secondarySearchAddon: SearchAddon | null = null;
  let secondarySessionId: string | null = null;
  let secondaryResizeObserver: ResizeObserver | null = null;
  let secondaryPasteHandler: ((e: ClipboardEvent) => void) | null = null;

  $: searchInputId = `search-input-${terminalData.sessionId}`;

  function isCopyShortcut(e: KeyboardEvent) {
    const key = e.key.toLowerCase();
    if (e.metaKey && key === 'c') return true;
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && key === 'c') return true;
    return false;
  }

  function handleDomPaste(e: ClipboardEvent, pane: 'primary' | 'secondary') {
    const text = e.clipboardData?.getData('text/plain') ?? '';
    if (!text) return;
    e.preventDefault();
    e.stopPropagation();
    activePane = pane;
    const sessionId = pane === 'secondary' ? secondarySessionId : terminalData.sessionId;
    if (!sessionId) return;
    handleTerminalInput(sessionId, text, terminalData.connection);
    const term = pane === 'secondary' ? secondaryTerminal : terminalData.terminal;
    setTimeout(() => term?.focus(), 0);
  }

  function handlePrimaryPaste(e: ClipboardEvent) {
    handleDomPaste(e, 'primary');
  }

  function handleSecondaryPaste(e: ClipboardEvent) {
    handleDomPaste(e, 'secondary');
  }

  function attachTerminalKeybindings(term: Terminal, pane: 'primary' | 'secondary') {
    term.attachCustomKeyEventHandler((e: KeyboardEvent) => {
      if (e.type === 'keydown' && isCopyShortcut(e)) {
        const selection = term.getSelection() ?? '';
        if (!selection) return true;
        if (!navigator.clipboard?.writeText) return true;
        void navigator.clipboard.writeText(selection);
        return false;
      }
      if ((e.ctrlKey || e.metaKey) && e.key === 'f' && e.type === 'keydown') {
        activePane = pane;
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
  
  function handleDocumentClick() {
    if (contextMenu.show) {
      closeContextMenu();
    }
  }

  onMount(async () => {
      resizeObserver = new ResizeObserver(() => {
          if (isVisible && mode === 'terminal' && terminalData.fitAddon) {
              terminalData.fitAddon.fit();
              if (terminalData.terminal) {
                  sendTerminalResize(terminalData.sessionId, terminalData.terminal.cols, terminalData.terminal.rows);
              }
          }
      });

      // If terminal instance doesn't exist, create it
      if (!terminalData.terminal) {
          const result = await initTerminal(container, terminalData.sessionId, terminalData.connection);
          if (result) {
              // Update the reference in the object
              // Note: This mutation won't trigger store updates automatically, which is fine here
              terminalData.terminal = result.terminal;
              terminalData.fitAddon = result.fitAddon;
              terminalData.searchAddon = result.searchAddon;
              attachTerminalKeybindings(result.terminal, 'primary');
              primaryContainerRef = container;
          }
      } else {
          // If terminal already exists, open it in this container
          terminalData.terminal.open(container);
          terminalData.fitAddon.fit();
          attachTerminalKeybindings(terminalData.terminal, 'primary');
          primaryContainerRef = container;
      }

      resizeObserver.observe(container);
      container.addEventListener('paste', handlePrimaryPaste, true);

      document.addEventListener('click', handleDocumentClick);
  });

  onDestroy(() => {
      if (resizeObserver) {
          resizeObserver.disconnect();
      }
      document.removeEventListener('click', handleDocumentClick);
      primaryContainerRef?.removeEventListener('paste', handlePrimaryPaste, true);

      // 清理副终端
      void cleanupSecondaryTerminal();
  });

  function getActiveTerminal() {
      if (activePane === 'secondary' && secondaryTerminal) return secondaryTerminal;
      return terminalData.terminal;
  }

  function getActiveSearchAddon() {
      if (activePane === 'secondary' && secondarySearchAddon) return secondarySearchAddon;
      return terminalData.searchAddon;
  }

  function setActivePane(pane: 'primary' | 'secondary') {
      if (pane === 'secondary' && !secondaryTerminal) {
          activePane = 'primary';
          terminalData.terminal?.focus();
          return;
      }
      activePane = pane;
      const term = pane === 'secondary' ? secondaryTerminal : terminalData.terminal;
      term?.focus();
  }

  function attachPrimaryContainer(next: HTMLElement) {
      if (!terminalData.terminal) return;
      if (primaryContainerRef && primaryContainerRef !== next) {
          primaryContainerRef.removeEventListener('paste', handlePrimaryPaste, true);
          resizeObserver?.unobserve(primaryContainerRef);
          primaryContainerRef.innerHTML = '';
      }
      if (primaryContainerRef !== next) {
          next.innerHTML = '';
          terminalData.terminal.open(next);
          terminalData.fitAddon?.fit();
          sendTerminalResize(terminalData.sessionId, terminalData.terminal.cols, terminalData.terminal.rows);
          resizeObserver?.observe(next);
          next.addEventListener('paste', handlePrimaryPaste, true);
          primaryContainerRef = next;
      }
  }

  function handleSearch() {
      const addon = getActiveSearchAddon();
      if (addon) {
          addon.findNext(searchTerm, {
            regex: false,
            wholeWord: false,
            caseSensitive: false,
            incremental: true
          });
      }
  }

  function handleSearchPrevious() {
      const addon = getActiveSearchAddon();
      if (addon) {
          addon.findPrevious(searchTerm, {
            regex: false,
            wholeWord: false,
            caseSensitive: false,
          });
      }
  }

  function handleSearchNext() {
      const addon = getActiveSearchAddon();
      if (addon) {
          addon.findNext(searchTerm, {
            regex: false,
            wholeWord: false,
            caseSensitive: false,
          });
      }
  }

  function closeSearch() {
      showSearch = false;
      getActiveTerminal()?.focus();
  }

  function handleKeydown(e: KeyboardEvent) {
      if (e.key === 'Enter') {
          if (e.shiftKey) {
              handleSearchPrevious();
          } else {
              handleSearchNext();
          }
      } else if (e.key === 'Escape') {
          closeSearch();
      }
  }

  // 右键菜单处理函数
  function openContextMenu(e: MouseEvent, pane: 'primary' | 'secondary') {
    e.preventDefault();
    setActivePane(pane);
    contextMenu.x = e.clientX;
    contextMenu.y = e.clientY;
    contextMenu.show = true;
  }

  function closeContextMenu() {
    contextMenu.show = false;
  }

  function handleCopy() {
    const term = getActiveTerminal();
    if (!term) return;
    const selection = term.getSelection();
    if (selection && navigator.clipboard?.writeText) {
      navigator.clipboard.writeText(selection);
    }
    closeContextMenu();
  }

  async function handlePaste() {
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        const sessionId = activePane === 'secondary' ? secondarySessionId : terminalData.sessionId;
        if (sessionId) {
          handleTerminalInput(sessionId, text, terminalData.connection);
        }
      }
    } catch (err) {
      console.error('Failed to paste:', err);
    }
    closeContextMenu();
    setTimeout(() => getActiveTerminal()?.focus(), 0);
  }

  function handleClearScreen() {
    const term = getActiveTerminal();
    if (term) {
      term.clear();
    }
    closeContextMenu();
  }

  function handleSelectAll() {
    const term = getActiveTerminal();
    if (term) {
      term.selectAll();
    }
    closeContextMenu();
  }

  function handleFind() {
    showSearch = true;
    setTimeout(() => document.getElementById(searchInputId)?.focus(), 0);
    closeContextMenu();
  }

  function handleClearScrollback() {
    const term = getActiveTerminal();
    if (term) {
      term.clear();
    }
    closeContextMenu();
  }

  function handleReset() {
    const term = getActiveTerminal();
    if (term) {
      term.reset();
    }
    closeContextMenu();
  }

  // 分屏功能
  async function handleSplitHorizontal() {
    splitMode = 'horizontal';
    splitRatio = 0.5;
    await tick();
    await initSecondaryTerminal();
    setActivePane('primary');
    closeContextMenu();
  }

  async function handleSplitVertical() {
    splitMode = 'vertical';
    splitRatio = 0.5;
    await tick();
    await initSecondaryTerminal();
    setActivePane('primary');
    closeContextMenu();
  }

  async function handleCancelSplit() {
    splitMode = 'none';
    await cleanupSecondaryTerminal();
    setActivePane('primary');
    closeContextMenu();
  }

  async function initSecondaryTerminal() {
    if (secondaryTerminal) return;

    try {
      // 创建新的 SSH 连接
      const newSessionId = await invoke('connect', { config: terminalData.connection }) as string;
      secondarySessionId = newSessionId;

      // 获取设置
      const appSettings = getXtermTheme($settings);

      // 创建新的终端实例
      const term = new Terminal({
        cursorBlink: $settings.terminal.cursorBlink,
        cursorStyle: $settings.terminal.cursorStyle,
        cursorWidth: 1,
        fontSize: $settings.terminal.fontSize,
        fontFamily: $settings.terminal.fontFamily,
        theme: appSettings,
        scrollback: $settings.terminal.scrollback,
        allowProposedApi: true,
        convertEol: true,
      });

      const fitAddon = new FitAddon();
      const searchAddon = new SearchAddon();
      term.loadAddon(fitAddon);
      term.loadAddon(searchAddon);

      if (secondaryContainer) {
        term.open(secondaryContainer);
        secondaryContainer.innerHTML = '';
        term.open(secondaryContainer);

        setTimeout(() => {
          fitAddon.fit();
        }, 100);
      }

      // 监听用户输入
      term.onData((data: string) => {
        handleTerminalInput(newSessionId, data, terminalData.connection);
      });

      // 监听终端输出
      await setupTerminalListeners(newSessionId, term);

      // 启动终端会话
      const result = await invoke('start_terminal', {
        sessionId: newSessionId,
        width: term.cols,
        height: term.rows,
      });

      if (!result) {
        console.error('Failed to start secondary terminal session');
        term.write('\r\n\x1b[31mFailed to start terminal session\x1b[0m\r\n');
        await cleanupSecondaryTerminal();
        return;
      }

      secondaryTerminal = term;
      secondaryFitAddon = fitAddon;
      secondarySearchAddon = searchAddon;
      attachTerminalKeybindings(term, 'secondary');

      // 设置 ResizeObserver
      if (secondaryContainer) {
        secondaryResizeObserver = new ResizeObserver(() => {
          if (secondaryFitAddon) {
            secondaryFitAddon.fit();
            if (secondaryTerminal) {
              sendTerminalResize(newSessionId, secondaryTerminal.cols, secondaryTerminal.rows);
            }
          }
        });
        secondaryResizeObserver.observe(secondaryContainer);
        secondaryContainer.addEventListener('paste', handleSecondaryPaste, true);
        secondaryPasteHandler = handleSecondaryPaste;
      }
    } catch (error) {
      console.error('Failed to initialize secondary terminal:', error);
      await cleanupSecondaryTerminal();
    }
  }

  async function cleanupSecondaryTerminal() {
    if (secondaryTerminal) {
      try {
        secondaryTerminal.dispose();
      } catch (e) {
        console.warn('Failed to dispose secondary terminal:', e);
      }
      secondaryTerminal = null;
    }
    secondaryFitAddon = null;
    secondarySearchAddon = null;

    if (secondaryResizeObserver) {
      secondaryResizeObserver.disconnect();
      secondaryResizeObserver = null;
    }

    if (secondaryContainer && secondaryPasteHandler) {
      secondaryContainer.removeEventListener('paste', secondaryPasteHandler, true);
      secondaryPasteHandler = null;
    }
    secondaryContainerRef = null;

    if (secondarySessionId) {
      try {
        await invoke('close_terminal', { sessionId: secondarySessionId });
      } catch (e) {
        console.warn('Failed to close secondary session:', e);
      }
      try {
        await invoke('disconnect', { sessionId: secondarySessionId });
      } catch (e) {
        console.warn('Failed to disconnect secondary session:', e);
      }
      secondarySessionId = null;
    }
  }

  // 分割条拖拽处理
  function handleSplitStart(e: MouseEvent | KeyboardEvent) {
    isResizing = true;
    document.addEventListener('mousemove', handleSplitMove);
    document.addEventListener('mouseup', handleSplitEnd);
    e.preventDefault();
  }

  function handleSplitMove(e: MouseEvent) {
    if (!isResizing || !container) return;

    const rect = (splitContainer ?? container).getBoundingClientRect();
    if (splitMode === 'horizontal') {
      // 水平分屏，上下分割
      const relativeY = e.clientY - rect.top;
      splitRatio = Math.max(0.1, Math.min(0.9, relativeY / rect.height));
    } else if (splitMode === 'vertical') {
      // 垂直分屏，左右分割
      const relativeX = e.clientX - rect.left;
      splitRatio = Math.max(0.1, Math.min(0.9, relativeX / rect.width));
    }
  }

  function handleSplitEnd() {
    isResizing = false;
    document.removeEventListener('mousemove', handleSplitMove);
    document.removeEventListener('mouseup', handleSplitEnd);
  }

  function handleSplitKeydown(e: KeyboardEvent) {
    if (splitMode === 'horizontal') {
      if (e.key === 'ArrowUp') {
        splitRatio = Math.max(0.1, splitRatio - 0.02);
        e.preventDefault();
      } else if (e.key === 'ArrowDown') {
        splitRatio = Math.min(0.9, splitRatio + 0.02);
        e.preventDefault();
      }
      return;
    }
    if (splitMode === 'vertical') {
      if (e.key === 'ArrowLeft') {
        splitRatio = Math.max(0.1, splitRatio - 0.02);
        e.preventDefault();
      } else if (e.key === 'ArrowRight') {
        splitRatio = Math.min(0.9, splitRatio + 0.02);
        e.preventDefault();
      }
    }
  }

  // Reactively update terminal options when settings change
  $: if (terminalData && terminalData.terminal) {
      terminalData.terminal.options.fontSize = $settings.terminal.fontSize;
      terminalData.terminal.options.fontFamily = $settings.terminal.fontFamily;
      terminalData.terminal.options.cursorBlink = $settings.terminal.cursorBlink;
      terminalData.terminal.options.cursorStyle = $settings.terminal.cursorStyle;
      (terminalData.terminal.options as any).cursorWidth = 1;
      terminalData.terminal.options.scrollback = $settings.terminal.scrollback;

      // Update theme
      terminalData.terminal.options.theme = getXtermTheme($settings);

      // Re-fit when font size changes
      if (isVisible && mode === 'terminal' && terminalData.fitAddon) {
        setTimeout(() => terminalData.fitAddon.fit(), 10);
      }
  }

  // Update secondary terminal options when settings change
  $: if (secondaryTerminal && splitMode !== 'none') {
      secondaryTerminal.options.fontSize = $settings.terminal.fontSize;
      secondaryTerminal.options.fontFamily = $settings.terminal.fontFamily;
      secondaryTerminal.options.cursorBlink = $settings.terminal.cursorBlink;
      secondaryTerminal.options.cursorStyle = $settings.terminal.cursorStyle;
      (secondaryTerminal.options as any).cursorWidth = 1;
      secondaryTerminal.options.scrollback = $settings.terminal.scrollback;

      // Update theme
      secondaryTerminal.options.theme = getXtermTheme($settings);

      // Re-fit when font size changes
      const fitAddon = secondaryFitAddon;
      if (fitAddon) {
        setTimeout(() => fitAddon.fit(), 10);
      }
  }

  $: if (terminalData.terminal && container && container !== primaryContainerRef) {
      attachPrimaryContainer(container);
  }

  $: if (secondaryTerminal && secondaryContainer && secondaryContainer !== secondaryContainerRef) {
      secondaryTerminal.open(secondaryContainer);
      secondaryFitAddon?.fit();
      secondaryContainerRef = secondaryContainer;
  }

  // Watch for visibility changes to resize
  $: if (isVisible && mode === 'terminal' && terminalData.fitAddon) {
      setTimeout(() => {
          terminalData.fitAddon.fit();
          terminalData.terminal.focus();
      }, 50);
  }
</script>

<div class="w-full h-full flex flex-col" style:display={isVisible ? 'flex' : 'none'}>
  <!-- Mode Switcher Toolbar -->
  <div class="flex items-center border-b border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-900 px-2 h-8 shrink-0 gap-1">
    <button 
      class="px-3 py-1 text-xs rounded-t transition-colors {mode === 'terminal' ? 'bg-white dark:bg-slate-950 text-blue-600 dark:text-blue-400 font-medium border-x border-t border-slate-200 dark:border-slate-800 -mb-[1px] relative z-10' : 'text-slate-500 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-800'}"
      on:click={() => mode = 'terminal'}
    >
      Terminal
    </button>
    <button 
      class="px-3 py-1 text-xs rounded-t transition-colors {mode === 'sftp' ? 'bg-white dark:bg-slate-950 text-blue-600 dark:text-blue-400 font-medium border-x border-t border-slate-200 dark:border-slate-800 -mb-[1px] relative z-10' : 'text-slate-500 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-800'}"
      on:click={() => mode = 'sftp'}
    >
      File Browser
    </button>
  </div>

  <div class="flex-1 relative overflow-hidden bg-white dark:bg-slate-950">
     <!-- Terminal Container - No Split -->
     {#if splitMode === 'none'}
     <div
       bind:this={container}
       class="w-full h-full overflow-hidden"
       style:display={mode === 'terminal' ? 'block' : 'none'}
       on:contextmenu|preventDefault={(e) => openContextMenu(e, 'primary')}
       on:click={() => setActivePane('primary')}
       on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && setActivePane('primary')}
       role="button"
       tabindex="0"
     ></div>
     {:else}
     <!-- Split Container -->
     <div bind:this={splitContainer} class={`w-full h-full overflow-hidden flex ${splitMode === 'horizontal' ? 'flex-col' : 'flex-row'}`} style:display={mode === 'terminal' ? 'flex' : 'none'}>
       <!-- Primary Terminal -->
       <div
         class="relative overflow-hidden bg-white dark:bg-slate-950"
         style:height={splitMode === 'horizontal' ? `${splitRatio * 100}%` : '100%'}
         style:width={splitMode === 'vertical' ? `${splitRatio * 100}%` : '100%'}
       >
         <div
           bind:this={container}
           class="w-full h-full overflow-hidden"
           on:contextmenu|preventDefault={(e) => openContextMenu(e, 'primary')}
           on:click={() => setActivePane('primary')}
            on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && setActivePane('primary')}
            role="button"
            tabindex="0"
         ></div>
         <!-- Split Indicator Label -->
         <div class="absolute top-1 left-1 text-[10px] px-1.5 py-0.5 bg-blue-500/20 text-blue-600 dark:text-blue-400 rounded border border-blue-500/30 font-medium pointer-events-none">
           主面板
         </div>
       </div>

       <!-- Splitter -->
      <button
        type="button"
        aria-label="调整分屏"
        class="bg-slate-200 dark:bg-slate-700 hover:bg-blue-500 dark:hover:bg-blue-500 transition-colors cursor-col-resize z-10 p-0 border-0"
         style:height={splitMode === 'horizontal' ? '4px' : '100%'}
         style:width={splitMode === 'vertical' ? '4px' : '100%'}
         style:cursor={splitMode === 'horizontal' ? 'row-resize' : 'col-resize'}
         on:mousedown={handleSplitStart}
        on:keydown={handleSplitKeydown}
      ></button>

       <!-- Secondary Terminal -->
       <div
         class="relative overflow-hidden bg-slate-50/50 dark:bg-slate-900/50"
         style:height={splitMode === 'horizontal' ? `${(1 - splitRatio) * 100}%` : '100%'}
         style:width={splitMode === 'vertical' ? `${(1 - splitRatio) * 100}%` : '100%'}
       >
         {#if secondaryTerminal}
         <div
           bind:this={secondaryContainer}
           class="w-full h-full overflow-hidden"
           on:contextmenu|preventDefault={(e) => openContextMenu(e, 'secondary')}
           on:click={() => setActivePane('secondary')}
           on:keydown={(e) => (e.key === 'Enter' || e.key === ' ') && setActivePane('secondary')}
           role="button"
           tabindex="0"
         ></div>
         {:else}
         <div class="w-full h-full flex items-center justify-center text-slate-400 dark:text-slate-500">
           <div class="text-center">
             <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-slate-400 mx-auto mb-2"></div>
             <p class="text-sm">正在初始化副面板...</p>
           </div>
         </div>
         {/if}
         <!-- Split Indicator Label -->
         <div class="absolute top-1 left-1 text-[10px] px-1.5 py-0.5 bg-green-500/20 text-green-600 dark:text-green-400 rounded border border-green-500/30 font-medium pointer-events-none">
           副面板
         </div>
       </div>
     </div>
     {/if}

     <!-- Search Bar -->
     {#if showSearch && mode === 'terminal'}
      <div class="absolute top-2 right-2 z-10 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 shadow-lg rounded-md p-1.5 flex items-center gap-1.5">
        <input 
          id={searchInputId}
          type="text" 
          bind:value={searchTerm} 
          on:input={handleSearch}
          on:keydown={handleKeydown}
          placeholder="Find..." 
          class="bg-slate-100 dark:bg-slate-900 border border-slate-200 dark:border-slate-700 rounded px-2 py-1 text-sm text-slate-800 dark:text-slate-200 w-48 focus:outline-none focus:ring-1 focus:ring-blue-500"
        />
        <div class="flex items-center border-l border-slate-200 dark:border-slate-700 pl-1.5 gap-1">
          <button on:click={handleSearchPrevious} class="p-1 hover:bg-slate-100 dark:hover:bg-slate-700 rounded text-slate-500 dark:text-slate-400" title="Previous (Shift+Enter)">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7"></path></svg>
          </button>
          <button on:click={handleSearchNext} class="p-1 hover:bg-slate-100 dark:hover:bg-slate-700 rounded text-slate-500 dark:text-slate-400" title="Next (Enter)">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path></svg>
          </button>
          <button on:click={closeSearch} class="p-1 hover:bg-slate-100 dark:hover:bg-slate-700 rounded text-slate-500 dark:text-slate-400 hover:text-red-500 dark:hover:text-red-400" title="Close (Esc)">
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path></svg>
          </button>
        </div>
      </div>
     {/if}

     <!-- SFTP Container -->
     {#if mode === 'sftp'}
       <div class="w-full h-full absolute inset-0 z-0">
         <DualPaneFileExplorer sessionId={terminalData.sessionId} />
       </div>
     {/if}

     <!-- 右键菜单 -->
     {#if contextMenu.show && mode === 'terminal'}
       <div
         class="fixed bg-white dark:bg-gray-800 border border-slate-200 dark:border-gray-700 rounded shadow-lg py-1 z-50 text-sm min-w-[180px]"
         style="top: {contextMenu.y}px; left: {contextMenu.x}px"
         role="menu"
         tabindex="-1"
         on:click|stopPropagation={() => {}}
         on:keydown|stopPropagation={() => {}}
       >
         <button
           class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
           on:click|stopPropagation={handleCopy}
         >
           复制
         </button>
         <button
           class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
           on:click|stopPropagation={handlePaste}
         >
           粘贴
         </button>
         <div class="border-t border-slate-200 dark:border-gray-700 my-1"></div>
         <button
           class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
           on:click|stopPropagation={handleClearScreen}
         >
           清屏
         </button>
         <button
           class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
           on:click|stopPropagation={handleSelectAll}
         >
           全选
         </button>
         <button
           class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
           on:click|stopPropagation={handleFind}
         >
           查找
         </button>
         <div class="border-t border-slate-200 dark:border-gray-700 my-1"></div>
         {#if splitMode === 'none'}
           <button
             class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 flex items-center gap-2"
             on:click|stopPropagation={handleSplitHorizontal}
           >
             <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
               <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"></path>
             </svg>
             上下分屏
           </button>
           <button
             class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 flex items-center gap-2"
             on:click|stopPropagation={handleSplitVertical}
           >
             <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
               <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 4h13M8 20h13M3 4h.01M3 20h.01"></path>
             </svg>
             左右分屏
           </button>
         {:else}
           <button
             class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
             on:click|stopPropagation={handleCancelSplit}
           >
             取消分屏
           </button>
         {/if}
         <div class="border-t border-slate-200 dark:border-gray-700 my-1"></div>
         <button
           class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
           on:click|stopPropagation={handleClearScrollback}
         >
           清除滚动缓冲区
         </button>
         <button
           class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-red-600 dark:text-red-400 hover:text-red-500 dark:hover:text-red-300"
           on:click|stopPropagation={handleReset}
         >
           重置终端
         </button>
       </div>
     {/if}
  </div>
</div>
