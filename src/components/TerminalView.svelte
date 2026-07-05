<script lang="ts">
import { onDestroy } from 'svelte';
import { get } from 'svelte/store';
import { activeTerminals, broadcastSessionIds, selectedTerminalIndex, type ActiveTerminal, terminalSessionMap, closeSplitRequest } from '../lib/store';
import { createTerminalSession, closeSplitSession, disconnectTerminal } from '../lib/terminalService';
import { computeSelectedIndexAfterBatchRemoval } from '../lib/terminalStateUtils';
import type { LayoutNode, TerminalPaneNode, SplitNode } from '../lib/layout';
import { generateId, findNode, replaceNode, removeNode, findNodeBySessionId } from '../lib/layout';
import type { TerminalProxy } from '../lib/terminalProxy';
import SplitPane from './terminal/SplitPane.svelte';

  export let terminalData: ActiveTerminal;
  export let isVisible: boolean = false;

  let layoutRoot: LayoutNode | null = null;
  let initializedSessionId: string | null = null;
  let activePaneId: string | null = null;
  let destroyed = false;

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
      createdAt: Date.now(),
      onInit: (proxy: TerminalProxy) => {
        const instance = proxy.getInstance();
        const { terminal, fitAddon, searchAddon } = instance;
        // 更新 activeTerminals store
        activeTerminals.update(terminals => {
          return terminals.map(t => {
            if (t.sessionId === terminalData.sessionId) {
              return { 
                ...t, 
                terminal, 
                fitAddon, 
                searchAddon 
              };
            }
            return t;
          });
        });
      }
    };
    
    // Update session map
  updateSessionMap();
  }

  // Monitor close split request
  $: if ($closeSplitRequest && layoutRoot) {
    const targetSessionId = $closeSplitRequest;
    // Check if this TerminalView manages this session
    const node = findNodeBySessionId(layoutRoot, targetSessionId);
    if (node && node.type === 'pane') {
      if (node.isRoot) {
        // If it's the root session, close the whole terminal session
        void disconnectTerminal(targetSessionId);
        closeSplitRequest.set(null);
      } else {
        handleClosePane({ detail: { targetId: node.id } } as CustomEvent);
        closeSplitRequest.set(null);
      }
    }
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
      const next = new Map(map);
      next.set(terminalData.sessionId, sessionIds);
      return next;
    });
  }

  onDestroy(() => {
    destroyed = true;
    const rootSessionId = terminalData?.sessionId;
    if (!rootSessionId) return;
    terminalSessionMap.update((map) => {
      if (!map.has(rootSessionId)) return map;
      const next = new Map(map);
      next.delete(rootSessionId);
      return next;
    });
  });

  async function handleSplit(e: CustomEvent) {
    const { direction, targetId } = e.detail;
    if (!layoutRoot) return;

    const targetNode = findNode(layoutRoot, targetId);
    if (!targetNode || targetNode.type !== 'pane') return;
    const splitRootSessionId = terminalData.sessionId;
    const targetSessionId = targetNode.sessionId;

    try {
      // Create new session
      const newSessionId = await createTerminalSession(targetNode.connection);

      const latestRoot = layoutRoot;
      const stillActiveRoot = get(activeTerminals).some(t => t.sessionId === splitRootSessionId);
      const latestTarget = latestRoot ? findNode(latestRoot, targetId) : null;
      const targetStillValid =
        latestTarget &&
        latestTarget.type === 'pane' &&
        latestTarget.sessionId === targetSessionId;

      if (destroyed || !latestRoot || !stillActiveRoot || !targetStillValid) {
        await closeSplitSession(newSessionId);
        activeTerminals.update(terms => terms.filter(t => t.sessionId !== newSessionId));
        return;
      }
      
      // Add to activeTerminals as a child of the current root session
      activeTerminals.update(terms => [
        ...terms,
        {
          sessionId: newSessionId,
          connection: latestTarget.connection,
          terminal: null as any,
          fitAddon: null as any,
          searchAddon: null as any,
          parentId: terminalData.sessionId // Link to root session
        }
      ]);

      const newPaneId = generateId();

      // New pane for the new session
      const newPane: TerminalPaneNode = {
        type: 'pane',
        id: newPaneId,
        sessionId: newSessionId,
        connection: latestTarget.connection,
        isRoot: false,
        createdAt: Date.now(),
        onInit: (proxy: TerminalProxy) => {
          const instance = proxy.getInstance();
          const { terminal, fitAddon, searchAddon } = instance;
          // Update activeTerminals with initialized instance
          activeTerminals.update(terminals => {
            return terminals.map(t => {
              if (t.sessionId === newSessionId) {
                return { 
                  ...t, 
                  terminal, 
                  fitAddon, 
                  searchAddon 
                };
              }
              return t;
            });
          });
          
          console.log(`[TerminalView] Split pane initialized: ${newSessionId}`);
        }
      };

      // Original pane - keep the same ID so {#key} preserves the component instance
      // This ensures the original terminal is not recreated
      const splitNode: SplitNode = {
        type: 'split',
        id: generateId(),
        direction,
        splitRatio: 0.5,
        children: [latestTarget, newPane]
      };

      layoutRoot = replaceNode(layoutRoot, targetId, splitNode);
      updateSessionMap();
      
    } catch (error) {
      console.error('Failed to create split session:', error);
    }
  }

  async function handleClosePane(e: CustomEvent) {
    const { targetId } = e.detail;
    if (!layoutRoot) return;
    
    const targetNode = findNode(layoutRoot, targetId);
    
    // If it's the root pane, close the entire session
    if (targetNode && targetNode.type === 'pane' && targetNode.isRoot) {
      void disconnectTerminal(targetNode.sessionId);
      return;
    }

    if (targetNode && targetNode.type === 'pane' && !targetNode.isRoot) {
         // 关闭分屏会话
        try {
          await closeSplitSession(targetNode.sessionId);
        } catch (error) {
          console.warn('Failed to close split session', error);
          return;
        }
         // Remove from activeTerminals，并同步收敛选中下标，避免下标越界导致
         // selectedTerminal 派生为 null。
         const before = get(activeTerminals);
         const currentIndex = get(selectedTerminalIndex);
         const removed = new Set([targetNode.sessionId]);
         activeTerminals.update(terms => terms.filter(t => !removed.has(t.sessionId)));
         selectedTerminalIndex.set(
           computeSelectedIndexAfterBatchRemoval(before, currentIndex, removed)
         );
         broadcastSessionIds.update(ids => ids.filter(id => id !== targetNode.sessionId));
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

  function shouldRestorePaneFocus(paneId: string, isRoot: boolean): boolean {
      if (activePaneId) return activePaneId === paneId;
      return isRoot;
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
           rootNode={layoutRoot}
           isVisible={isVisible}
           shouldRestoreFocusForPane={shouldRestorePaneFocus}
           on:split={handleSplit}
           on:closePane={handleClosePane}
           on:activePane={handlePaneActive}
         />
       {/if}
     </div>
  </div>
</div>
