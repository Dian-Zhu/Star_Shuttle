<script lang="ts">
  import { onMount, onDestroy, tick } from 'svelte';
  import { type ActiveTerminal, settings, terminalSessionMap, activePaneId as activePaneIdStore } from '../lib/store';
  import { createTerminalSession, closeSplitSession } from '../lib/terminalService';
  import type { LayoutNode, TerminalPaneNode, SplitNode } from '../lib/layout';
  import { generateId, findNode, replaceNode, removeNode } from '../lib/layout';
  import SplitPane from './terminal/SplitPane.svelte';
  import DualPaneFileExplorer from './file-transfer/DualPaneFileExplorer.svelte';

  export let terminalData: ActiveTerminal;
  export let isVisible: boolean = false;

  let mode: 'terminal' | 'sftp' = 'terminal';
  let layoutRoot: LayoutNode | null = null;
  let initializedSessionId: string | null = null;
  let activePaneId: string | null = null;

  // Sync activePaneId with store
  $: activePaneIdStore.set(activePaneId);
  $: if (layoutRoot && !activePaneId) {
      // Initialize activePaneId with root pane if not set
      if (layoutRoot.type === 'pane') {
          activePaneId = layoutRoot.id;
      }
  }

  // Initialize layout when terminalData changes or on mount
  $: if (terminalData && terminalData.sessionId !== initializedSessionId) {
      initializedSessionId = terminalData.sessionId;
      activePaneId = null; // Reset active pane
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
      
      // Update session map
      updateSessionMap();
  }

  function getAllSessionIds(node: LayoutNode): string[] {
    if (node.type === 'pane') {
      return [node.sessionId];
    }
    return [...getAllSessionIds(node.children[0]), ...getAllSessionIds(node.children[1])];
  }

  function updateSessionMap() {
    if (!layoutRoot || !terminalData) return;
    const sessionIds = new Set(getAllSessionIds(layoutRoot));
    terminalSessionMap.update(map => {
      map.set(terminalData.sessionId, sessionIds);
      return map;
    });
  }

  onDestroy(() => {
    if (terminalData && layoutRoot) {
      // Clean up all non-root sessions
      const cleanupNode = (node: LayoutNode) => {
        if (node.type === 'pane' && !node.isRoot) {
           closeSplitSession(node.sessionId, node.existingTerminal);
        } else if (node.type === 'split') {
           cleanupNode(node.children[0]);
           cleanupNode(node.children[1]);
        }
      };
      cleanupNode(layoutRoot);
      
      // Remove from session map
      terminalSessionMap.update(map => {
        map.delete(terminalData.sessionId);
        return map;
      });
    }
  });

  async function handleSplit(e: CustomEvent) {
    const { direction, targetId } = e.detail;
    if (!layoutRoot) return;

    const targetNode = findNode(layoutRoot, targetId);
    if (!targetNode || targetNode.type !== 'pane') return;

    try {
      // Create new session
      // Note: We use the connection from the target node, effectively cloning the session configuration
      const newSessionId = await createTerminalSession(targetNode.connection as any);
      
      const newPaneId = generateId();
      const newPane: TerminalPaneNode = {
        type: 'pane',
        id: newPaneId,
        sessionId: newSessionId,
        connection: targetNode.connection as any,
        isRoot: false,
        // Capture the terminal instance when it is initialized
        onInit: (term, fit, search) => {
             if (!layoutRoot) return;
             // We need to find the node again because layoutRoot might have changed?
             // Actually we can just update the layout tree immutably to include these instances
             // However, finding the node by ID is safer.
             
             // Helper to update node in tree
             const updateNode = (root: LayoutNode): LayoutNode => {
                 if (root.type === 'pane' && root.id === newPaneId) {
                     return { ...root, existingTerminal: term, existingFitAddon: fit, existingSearchAddon: search };
                 }
                 if (root.type === 'split') {
                     return {
                         ...root,
                         children: [updateNode(root.children[0]), updateNode(root.children[1])]
                     };
                 }
                 return root;
             };
             
             layoutRoot = updateNode(layoutRoot);
        }
      };
      
      const splitNode: SplitNode = {
        type: 'split',
        id: generateId(),
        direction,
        splitRatio: 0.5,
        children: [targetNode, newPane]
      };

      layoutRoot = replaceNode(layoutRoot, targetId, splitNode);
      updateSessionMap();
      
    } catch (error) {
      console.error('Failed to create split session:', error);
    }
  }

  function handleClosePane(e: CustomEvent) {
    const { targetId } = e.detail;
    if (!layoutRoot) return;
    
    const targetNode = findNode(layoutRoot, targetId);
    if (targetNode && targetNode.type === 'pane' && !targetNode.isRoot) {
         // Explicitly close the session
         closeSplitSession(targetNode.sessionId, targetNode.existingTerminal);
    }
    
    const newRoot = removeNode(layoutRoot, targetId);
    if (newRoot) {
      layoutRoot = newRoot;
      updateSessionMap();
    } else {
      // Should not happen as we prevent closing root pane in TerminalPane.svelte
      console.warn('Attempted to close the last pane');
    }
  }

  function handlePaneActive(e: CustomEvent) {
      activePaneId = e.detail.targetId;
  }
  
  function handleRearrange(e: CustomEvent) {
    const { sourceId, targetId, direction } = e.detail;
    console.log('handleRearrange called', { sourceId, targetId, direction });
    
    if (!layoutRoot) return;
    if (sourceId === targetId) return;

    // 1. Find source node
    const sourceNode = findNode(layoutRoot, sourceId);
    console.log('Source node found:', sourceNode);
    
    if (!sourceNode || sourceNode.type !== 'pane') {
        console.warn('Source node invalid or not found');
        return;
    }

    // 2. Remove source node
    const newRootAfterRemove = removeNode(layoutRoot, sourceId);
    console.log('Root after remove:', newRootAfterRemove);
    
    if (!newRootAfterRemove) {
        console.warn('Root became null after remove (should not happen if multiple panes exist)');
        return;
    }

    // 3. Find target node in the new tree
    const targetNode = findNode(newRootAfterRemove, targetId);
    console.log('Target node found in new tree:', targetNode);
    
    if (!targetNode) {
        console.warn('Target node not found after removal during rearrange');
        return;
    }

    // 4. Create new Split
    const children: [LayoutNode, LayoutNode] = (direction === 'top' || direction === 'left') 
        ? [sourceNode, targetNode] 
        : [targetNode, sourceNode];

    const newSplit: SplitNode = {
        type: 'split',
        id: generateId(),
        direction: (direction === 'top' || direction === 'bottom') ? 'vertical' : 'horizontal',
        splitRatio: 0.5,
        children
    };
    
    console.log('New split node created:', newSplit);

    // 5. Replace target with new Split
    const finalRoot = replaceNode(newRootAfterRemove, targetId, newSplit);
    console.log('Final root layout:', finalRoot);
    
    layoutRoot = finalRoot;
    updateSessionMap();
  }

  function handleKeydown(e: KeyboardEvent) {
      if (!isVisible) return;
      
      // Ctrl+Shift+W to close active pane
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'w' || e.key === 'W')) {
          if (activePaneId && layoutRoot) {
              e.preventDefault();
              e.stopPropagation();
              
              // Don't close if it's the root pane
              const node = findNode(layoutRoot, activePaneId);
              if (node && node.type === 'pane' && !node.isRoot) {
                  handleClosePane({ detail: { targetId: activePaneId } });
              }
          }
      }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

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
           on:activePane={handlePaneActive}
           on:rearrange={handleRearrange}
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
