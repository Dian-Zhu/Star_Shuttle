<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { LayoutNode } from '../../lib/layout';
  import TerminalPane from './TerminalPane.svelte';

  export let node: LayoutNode;
  export let isVisible: boolean = true;

  const dispatch = createEventDispatcher();

  // Resize logic
  let isResizing = false;
  let splitContainer: HTMLElement;

  function handleSplitStart(e: MouseEvent) {
    if (node.type !== 'split') return;
    isResizing = true;
    document.addEventListener('mousemove', handleSplitMove);
    document.addEventListener('mouseup', handleSplitEnd);
    e.preventDefault();
  }

  function handleSplitMove(e: MouseEvent) {
    if (!isResizing || !splitContainer || node.type !== 'split') return;
    
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
  <TerminalPane 
    sessionId={node.sessionId}
    connection={node.connection}
    isRoot={node.isRoot}
    existingTerminal={node.existingTerminal}
    existingFitAddon={node.existingFitAddon}
    existingSearchAddon={node.existingSearchAddon}
    onInit={node.onInit}
    isVisible={isVisible}
    on:split={(e) => handlePaneSplit(e, node.id)}
    on:close={(e) => handlePaneClose(e, node.id)}
    on:active={(e) => handlePaneActive(e, node.id)}
  />
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
        on:split
        on:closePane
        on:activePane
      />
    </div>

    <!-- Splitter Handle -->
    <button
      type="button"
      aria-label="调整分割"
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
        on:split
        on:closePane
        on:activePane
      />
    </div>
  </div>
{/if}
