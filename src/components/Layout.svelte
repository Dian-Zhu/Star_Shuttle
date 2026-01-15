<script lang="ts">
  import Sidebar from './Sidebar.svelte';
  import TerminalManager from './TerminalManager.svelte';
  import ConnectionModal from './ConnectionModal.svelte';
  import { showConnectionForm, successMessage, errorMessage } from '../lib/store';
  import { fade, fly } from 'svelte/transition';
</script>

<div class="h-screen w-screen flex bg-slate-950 text-slate-200 overflow-hidden font-sans antialiased selection:bg-blue-500/30">
  <Sidebar />
  
  <main class="flex-1 flex flex-col min-w-0 relative">
    <TerminalManager />
    
    <!-- Toast Messages -->
    <div class="absolute top-4 right-4 z-50 flex flex-col gap-2 pointer-events-none">
      {#if $successMessage}
        <div 
          transition:fly={{ y: -20, duration: 300 }}
          class="bg-green-500/10 border border-green-500/20 text-green-400 px-4 py-3 rounded-lg shadow-xl backdrop-blur-md flex items-center gap-2 pointer-events-auto min-w-[300px]"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path></svg>
          <span class="text-sm font-medium">{$successMessage}</span>
        </div>
      {/if}
      
      {#if $errorMessage}
        <div 
          transition:fly={{ y: -20, duration: 300 }}
          class="bg-red-500/10 border border-red-500/20 text-red-400 px-4 py-3 rounded-lg shadow-xl backdrop-blur-md flex items-center gap-2 pointer-events-auto min-w-[300px]"
        >
          <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
          <span class="text-sm font-medium">{$errorMessage}</span>
        </div>
      {/if}
    </div>
  </main>
</div>

{#if $showConnectionForm}
  <div transition:fade={{ duration: 200 }}>
    <ConnectionModal />
  </div>
{/if}
