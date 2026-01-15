<script lang="ts">
  import { onMount } from 'svelte';
  import { initTerminal } from '../lib/terminalService';
  import type { ActiveTerminal } from '../lib/store';
  
  // Props using Svelte 4 syntax for compatibility
  export let terminalData: ActiveTerminal;
  export let isVisible: boolean = false;
  
  let container: HTMLElement;
  
  onMount(async () => {
      // If terminal instance doesn't exist, create it
      if (!terminalData.terminal) {
          const result = await initTerminal(container, terminalData.sessionId, terminalData.connection);
          if (result) {
              // Update the reference in the object
              // Note: This mutation won't trigger store updates automatically, which is fine here
              terminalData.terminal = result.terminal;
              terminalData.fitAddon = result.fitAddon;
          }
      } else {
          // If terminal already exists, open it in this container
          terminalData.terminal.open(container);
          terminalData.fitAddon.fit();
      }
  });

  // Watch for visibility changes to resize
  $: if (isVisible && terminalData.fitAddon) {
      setTimeout(() => {
          terminalData.fitAddon.fit();
          terminalData.terminal.focus();
      }, 50);
  }
</script>

<div 
  bind:this={container} 
  class="w-full h-full overflow-hidden bg-slate-950"
  style:display={isVisible ? 'block' : 'none'}
></div>
