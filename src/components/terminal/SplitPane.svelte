<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { LayoutNode } from '../../lib/layout';
  import { getPaneIndex, getSplitDirectionFromDrag } from '../../lib/layout';
  import TerminalPane from './TerminalPane.svelte';

  export let node: LayoutNode;
  export let isVisible: boolean = true;
  export let shouldRestoreFocusForPane: ((paneId: string, isRoot: boolean) => boolean) | undefined = undefined;

  const dispatch = createEventDispatcher();

  // Get root node for calculating pane indices
  export let rootNode: LayoutNode | null = null;

  // Resize logic
  let isResizing = false;
  let splitContainer: HTMLElement;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragInitialDirection: 'horizontal' | 'vertical' | null = null;
  let dragDirectionLocked = false;

  function handleSplitStart(e: MouseEvent) {
    if (node.type !== 'split') return;
    isResizing = true;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    dragInitialDirection = node.direction;
    dragDirectionLocked = false;
    document.addEventListener('mousemove', handleSplitMove);
    document.addEventListener('mouseup', handleSplitEnd);
    e.preventDefault();
  }

  function handleSplitMove(e: MouseEvent) {
    if (!isResizing || !splitContainer || node.type !== 'split') return;

    if (!dragDirectionLocked && dragInitialDirection) {
      const nextDirection = getSplitDirectionFromDrag(
        dragInitialDirection,
        { x: dragStartX, y: dragStartY },
        { x: e.clientX, y: e.clientY }
      );
      if (nextDirection !== node.direction) {
        node.direction = nextDirection;
        dragDirectionLocked = true;
      }
    }
    
    const rect = splitContainer.getBoundingClientRect();
    let newRatio = node.splitRatio;
    
    if (node.direction === 'horizontal') {
      const relativeY = e.clientY - rect.top;
      newRatio = Math.max(0.1, Math.min(0.9, relativeY / rect.height));
    } else {
      const relativeX = e.clientX - rect.left;
      newRatio = Math.max(0.1, Math.min(0.9, relativeX / rect.width));
    }
    
    node.splitRatio = newRatio;
  }

  function handleSplitEnd() {
    isResizing = false;
    dragInitialDirection = null;
    dragDirectionLocked = false;
    document.removeEventListener('mousemove', handleSplitMove);
    document.removeEventListener('mouseup', handleSplitEnd);
  }
  
  function handlePaneSplit(e: CustomEvent, paneId: string) {
      dispatch('split', { ...e.detail, targetId: paneId });
  }

  function handlePaneClose(_e: CustomEvent, paneId: string) {
      dispatch('closePane', { targetId: paneId });
  }

  function handlePaneActive(_e: CustomEvent, paneId: string) {
      dispatch('activePane', { targetId: paneId });
  }
</script>

{#if node.type === 'pane'}
  {@const paneIndex = rootNode ? getPaneIndex(rootNode, node.id) : 1}
  {#key node.id}
    <TerminalPane
      sessionId={node.sessionId}
      connection={node.connection}
      isRoot={node.isRoot}
      paneIndex={paneIndex}
      onInit={node.onInit}
      isVisible={isVisible}
      shouldRestoreFocus={shouldRestoreFocusForPane ? shouldRestoreFocusForPane(node.id, node.isRoot ?? false) : (node.isRoot ?? false)}
      on:split={(e) => handlePaneSplit(e, node.id)}
      on:close={(e) => handlePaneClose(e, node.id)}
      on:active={(e) => handlePaneActive(e, node.id)}
    />
  {/key}
{:else}
  <div 
    bind:this={splitContainer}
    class={`w-full h-full flex overflow-hidden ${node.direction === 'vertical' ? 'flex-row' : 'flex-col'}`}
  >
    <div
      class="relative overflow-hidden"
      style:flex-basis={`${node.splitRatio * 100}%`}
    >
      <svelte:self
        node={node.children[0]}
        isVisible={isVisible}
        rootNode={rootNode}
        shouldRestoreFocusForPane={shouldRestoreFocusForPane}
        on:split
        on:closePane
        on:activePane
      />
    </div>

    <!-- Splitter Handle -->
    <button
      type="button"
      aria-label="调整分割"
      title="拖拽调整分割；沿另一方向大幅拖动可切换上下或左右排版"
      class="bg-app-border hover:bg-primary-500 transition-colors z-10 p-0 border-0 shrink-0"
      style:width={node.direction === 'vertical' ? '4px' : '100%'}
      style:height={node.direction === 'horizontal' ? '4px' : '100%'}
      style:cursor={node.direction === 'horizontal' ? 'row-resize' : 'col-resize'}
      on:mousedown={handleSplitStart}
    ></button>

    <div
      class="relative overflow-hidden flex-1"
    >
      <svelte:self
        node={node.children[1]}
        isVisible={isVisible}
        rootNode={rootNode}
        shouldRestoreFocusForPane={shouldRestoreFocusForPane}
        on:split
        on:closePane
        on:activePane
      />
    </div>
  </div>
{/if}
