<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  
  export let label: string = '';
  export let disabled: boolean = false;
  export let danger: boolean = false;
  export let iconComponent: any = null; // Optional icon component
  
  const dispatch = createEventDispatcher();
  
  function handleClick(e: MouseEvent) {
    if (disabled) return;
    dispatch('click', e);
  }
</script>

<button 
  class="w-full text-left px-3 py-1.5 flex items-center gap-2.5 transition-colors group
    {disabled ? 'opacity-50 cursor-not-allowed' : 'hover:bg-app-text/5 active:bg-app-text/10'}
    {danger ? 'text-red-500 hover:text-red-600' : 'text-app-text'}
  "
  on:click|stopPropagation={handleClick}
  disabled={disabled}
  type="button"
>
  <!-- Icon Slot or Prop -->
  <div class="w-4 h-4 flex items-center justify-center opacity-70 group-hover:opacity-100 transition-opacity">
    {#if $$slots.icon}
      <slot name="icon" />
    {:else if iconComponent}
      <svelte:component this={iconComponent} class="w-3.5 h-3.5" />
    {/if}
  </div>
  
  <span class="flex-1 truncate font-medium">
    {#if label}{label}{:else}<slot />{/if}
  </span>
  
  <!-- Right slot for shortcuts or arrows -->
  {#if $$slots.right}
    <div class="ml-2 text-xs opacity-50 font-mono">
      <slot name="right" />
    </div>
  {/if}
</button>
