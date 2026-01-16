<script lang="ts">
  import { onMount } from 'svelte';
  import { initTerminal } from '../lib/terminalService';
  import { settings, type ActiveTerminal } from '../lib/store';
  import FileExplorer from './file-transfer/FileExplorer.svelte';
  
  // Props using Svelte 4 syntax for compatibility
  export let terminalData: ActiveTerminal;
  export let isVisible: boolean = false;
  
  let container: HTMLElement;
  let mode: 'terminal' | 'sftp' = 'terminal';
  let showSearch = false;
  let searchTerm = '';
  
  onMount(async () => {
      // If terminal instance doesn't exist, create it
      if (!terminalData.terminal) {
          const result = await initTerminal(container, terminalData.sessionId, terminalData.connection);
          if (result) {
              // Update the reference in the object
              // Note: This mutation won't trigger store updates automatically, which is fine here
              terminalData.terminal = result.terminal;
              terminalData.fitAddon = result.fitAddon;
              terminalData.searchAddon = result.searchAddon;
              
              // Add key binding for search (Ctrl+F)
              terminalData.terminal.attachCustomKeyEventHandler((e) => {
                  if (e.ctrlKey && e.key === 'f' && e.type === 'keydown') {
                      showSearch = !showSearch;
                      if (showSearch) {
                          // Need to wait for DOM update
                          setTimeout(() => document.getElementById(`search-input-${terminalData.sessionId}`)?.focus(), 100);
                      } else {
                          terminalData.terminal.focus();
                      }
                      return false; // Prevent default Ctrl+F
                  }
                  return true;
              });
          }
      } else {
          // If terminal already exists, open it in this container
          terminalData.terminal.open(container);
          terminalData.fitAddon.fit();
      }
  });

  function handleSearch() {
      if (terminalData.searchAddon) {
          terminalData.searchAddon.findNext(searchTerm, {
            regex: false,
            wholeWord: false,
            caseSensitive: false,
            incremental: true
          });
      }
  }

  function handleSearchPrevious() {
      if (terminalData.searchAddon) {
          terminalData.searchAddon.findPrevious(searchTerm, {
            regex: false,
            wholeWord: false,
            caseSensitive: false,
          });
      }
  }

  function handleSearchNext() {
      if (terminalData.searchAddon) {
          terminalData.searchAddon.findNext(searchTerm, {
            regex: false,
            wholeWord: false,
            caseSensitive: false,
          });
      }
  }

  function closeSearch() {
      showSearch = false;
      terminalData.terminal.focus();
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

  // Reactively update terminal options when settings change
  $: if (terminalData && terminalData.terminal) {
      terminalData.terminal.options.fontSize = $settings.terminal.fontSize;
      terminalData.terminal.options.fontFamily = $settings.terminal.fontFamily;
      terminalData.terminal.options.cursorBlink = $settings.terminal.cursorBlink;
      
      // Update theme
      terminalData.terminal.options.theme = $settings.theme === 'light' ? {
        background: '#ffffff', // white
        foreground: '#0f172a', // slate-950
        cursor: '#2563eb',     // blue-600
        selectionBackground: '#e2e8f0', // slate-200
        black: '#000000',
        red: '#ef4444',
        green: '#22c55e',
        yellow: '#eab308',
        blue: '#3b82f6',
        magenta: '#d946ef',
        cyan: '#06b6d4',
        white: '#64748b',
        brightBlack: '#94a3b8',
        brightRed: '#f87171',
        brightGreen: '#4ade80',
        brightYellow: '#facc15',
        brightBlue: '#60a5fa',
        brightMagenta: '#e879f9',
        brightCyan: '#22d3ee',
        brightWhite: '#f1f5f9',
      } : {
        background: '#0f172a', // slate-950
        foreground: '#e2e8f0', // slate-200
        cursor: '#3b82f6',     // blue-500
        selectionBackground: '#334155', // slate-700
      };

      // Re-fit when font size changes
      if (isVisible && mode === 'terminal' && terminalData.fitAddon) {
        setTimeout(() => terminalData.fitAddon.fit(), 10);
      }
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
     <!-- Terminal Container -->
     <div 
       bind:this={container} 
       class="w-full h-full overflow-hidden"
       style:display={mode === 'terminal' ? 'block' : 'none'}
     ></div>

     <!-- Search Bar -->
     {#if showSearch && mode === 'terminal'}
      <div class="absolute top-2 right-2 z-10 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 shadow-lg rounded-md p-1.5 flex items-center gap-1.5">
        <input 
          id="search-input-{terminalData.sessionId}"
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
         <FileExplorer sessionId={terminalData.sessionId} />
       </div>
     {/if}
  </div>
</div>
