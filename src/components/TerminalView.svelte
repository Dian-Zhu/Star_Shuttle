<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { type ActiveTerminal, settings } from '../lib/store';
  import { createTerminalSession } from '../lib/terminalService';
  import type { LayoutNode, TerminalPaneNode, SplitNode } from '../lib/layout';
  import { generateId, findNode, replaceNode, removeNode } from '../lib/layout';
  import SplitPane from './terminal/SplitPane.svelte';
  import DualPaneFileExplorer from './file-transfer/DualPaneFileExplorer.svelte';

  export let terminalData: ActiveTerminal;
  export let isVisible: boolean = false;

  let mode: 'terminal' | 'sftp' = 'terminal';
  let layoutRoot: LayoutNode | null = null;
  let initializedSessionId: string | null = null;

  // Initialize layout when terminalData changes or on mount
  $: if (terminalData && terminalData.sessionId !== initializedSessionId) {
      initializedSessionId = terminalData.sessionId;
      layoutRoot = {
        type: 'pane',
        id: generateId(),
        sessionId: terminalData.sessionId,
        connection: terminalData.connection,
        isRoot: true,
        existingTerminal: terminalData.terminal,
        existingFitAddon: terminalData.fitAddon,
        existingSearchAddon: terminalData.searchAddon
      };
  }

  async function handleSplit(e: CustomEvent) {
    const { direction, targetId } = e.detail;
    if (!layoutRoot) return;

    const targetNode = findNode(layoutRoot, targetId);
    if (!targetNode || targetNode.type !== 'pane') return;

    try {
      // Create new session
      // Note: We use the connection from the target node, effectively cloning the session configuration
      const newSessionId = await createTerminalSession(targetNode.connection as any);
      
      const newPane: TerminalPaneNode = {
        type: 'pane',
        id: generateId(),
        sessionId: newSessionId,
        connection: targetNode.connection as any,
        isRoot: false
      };
      
      const splitNode: SplitNode = {
        type: 'split',
        id: generateId(),
        direction,
        splitRatio: 0.5,
        children: [targetNode, newPane]
      };

      layoutRoot = replaceNode(layoutRoot, targetId, splitNode);
      
    } catch (error) {
      console.error('Failed to create split session:', error);
    }
  }

  function handleClosePane(e: CustomEvent) {
    const { targetId } = e.detail;
    if (!layoutRoot) return;
    
    const newRoot = removeNode(layoutRoot, targetId);
    if (newRoot) {
      layoutRoot = newRoot;
    } else {
      // Should not happen as we prevent closing root pane in TerminalPane.svelte
      console.warn('Attempted to close the last pane');
    }
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
     <!-- Terminal Layout -->
     <div
       class="w-full h-full overflow-hidden"
       style:display={mode === 'terminal' ? 'block' : 'none'}
     >
       {#if layoutRoot}
         <SplitPane 
           node={layoutRoot} 
           isVisible={isVisible && mode === 'terminal'}
           on:split={handleSplit}
           on:closePane={handleClosePane}
         />
       {/if}
     </div>

     <!-- SFTP View -->
     {#if mode === 'sftp'}
       <DualPaneFileExplorer 
         sessionId={terminalData.sessionId}
         connection={terminalData.connection}
         isVisible={isVisible && mode === 'sftp'}
       />
     {/if}
  </div>
</div>
