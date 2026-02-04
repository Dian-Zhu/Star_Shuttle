<script lang="ts">
  import { onDestroy } from 'svelte';
  import { activeTerminals, type ActiveTerminal, terminalSessionMap } from '../lib/store';
  import { createTerminalSession, closeSplitSession } from '../lib/terminalService';
  import type { LayoutNode, TerminalPaneNode, SplitNode } from '../lib/layout';
  import { generateId, findNode, replaceNode, removeNode } from '../lib/layout';
  import SplitPane from './terminal/SplitPane.svelte';

  export let terminalData: ActiveTerminal;
  export let isVisible: boolean = false;

  let layoutRoot: LayoutNode | null = null;
  let initializedSessionId: string | null = null;
  let activePaneId: string | null = null;

  // Helper to update terminal instance in the layout tree
  function updateTerminalInTree(root: LayoutNode, paneId: string, term: any, fit: any, search: any): LayoutNode {
      if (root.type === 'pane') {
          if (root.id === paneId) {
              return { ...root, existingTerminal: term, existingFitAddon: fit, existingSearchAddon: search };
          }
          return root;
      }
      if (root.type === 'split') {
          return {
              ...root,
              children: [
                  updateTerminalInTree(root.children[0], paneId, term, fit, search),
                  updateTerminalInTree(root.children[1], paneId, term, fit, search)
              ] as [LayoutNode, LayoutNode]
          };
      }
      return root;
  }

  // Initialize layout when terminalData changes or on mount
  $: if (terminalData && terminalData.sessionId !== initializedSessionId) {
      initializedSessionId = terminalData.sessionId;
      activePaneId = null; // Reset active pane
      
      const rootId = generateId();
      layoutRoot = {
        type: 'pane',
        id: rootId,
        sessionId: terminalData.sessionId,
        connection: terminalData.connection,
        isRoot: true,
        existingTerminal: terminalData.terminal,
        existingFitAddon: terminalData.fitAddon,
        existingSearchAddon: terminalData.searchAddon,
        onInit: (term, fit, search) => {
             // Update layout tree
             if (layoutRoot) {
                 layoutRoot = updateTerminalInTree(layoutRoot, rootId, term, fit, search);
             }
             
             // Update activeTerminals store
             activeTerminals.update(terminals => {
                 return terminals.map(t => {
                     if (t.sessionId === terminalData.sessionId) {
                         return { ...t, terminal: term, fitAddon: fit, searchAddon: search };
                     }
                     return t;
                 });
             });
        }
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
                handleClosePane(new CustomEvent('closePane', { detail: { targetId: activePaneId } }));
            }
          }
      }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="w-full h-full flex flex-col" style:display={isVisible ? 'flex' : 'none'}>
  <div class="flex-1 relative overflow-hidden">
     <!-- Terminal Layout -->
     <div class="w-full h-full overflow-hidden">
       {#if layoutRoot}
         <SplitPane 
           node={layoutRoot} 
           isVisible={isVisible}
           on:split={handleSplit}
           on:closePane={handleClosePane}
           on:activePane={handlePaneActive}
         />
       {/if}
     </div>
  </div>
</div>
