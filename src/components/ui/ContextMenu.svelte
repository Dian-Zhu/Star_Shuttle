<script lang="ts">
  import { createEventDispatcher, onMount, tick } from 'svelte';
  import { scale } from 'svelte/transition';

  export let x: number;
  export let y: number;
  
  let menuEl: HTMLDivElement;
  let posX = x;
  let posY = y;
  
  // Update position when x/y changes, but clamp to viewport
  $: if (x !== undefined && y !== undefined) {
      updatePosition();
  }

  async function updatePosition() {
      // Wait for render to get dimensions
      await tick();
      if (!menuEl) return;
      
      const rect = menuEl.getBoundingClientRect();
      const winWidth = window.innerWidth;
      const winHeight = window.innerHeight;
      
      let newX = x;
      let newY = y;
      
      // Horizontal clamping
      if (x + rect.width > winWidth) {
          newX = x - rect.width; // Flip to left if it overflows right
          if (newX < 0) newX = 10; // If still overflows, stick to left
      }
      
      // Vertical clamping
      if (y + rect.height > winHeight) {
          newY = y - rect.height; // Flip up
          if (newY < 0) newY = 10; // If still overflows, stick to top
      }
      
      posX = newX;
      posY = newY;
  }

  const dispatch = createEventDispatcher();

  function close() {
    dispatch('close');
  }

  function handleOutsideClick(e: MouseEvent) {
    if (menuEl && !menuEl.contains(e.target as Node)) {
        close();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
      if (e.key === 'Escape') close();
  }

  onMount(() => {
      // Use mousedown with capture to ensure we detect clicks even if other components stop propagation
      window.addEventListener('mousedown', handleOutsideClick, true); 
      window.addEventListener('keydown', handleKeydown);
      
      updatePosition();

      return () => {
          window.removeEventListener('mousedown', handleOutsideClick, true);
          window.removeEventListener('keydown', handleKeydown);
      };
  });
</script>

<div 
  bind:this={menuEl}
  role="menu"
  tabindex="-1"
  class="fixed z-50 min-w-[180px] rounded-lg shadow-xl border border-app-border/50 py-1.5 text-sm overflow-hidden text-app-text select-none context-menu-glass"
  style="left: {posX}px; top: {posY}px;"
  transition:scale={{duration: 100, start: 0.95}}
  on:contextmenu|preventDefault
>
  <slot />
</div>

<style>
  /* Glass effect using color-mix for theme compatibility with transparency */
  .context-menu-glass {
    background-color: color-mix(in srgb, var(--color-surface) 90%, transparent);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
  }
</style>
