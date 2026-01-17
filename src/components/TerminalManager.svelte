<script lang="ts">
  import { activeTerminals, selectedTerminalIndex } from '../lib/store';
  import { closeTerminal } from '../lib/terminalService';
  import TerminalView from './TerminalView.svelte';
  import XIcon from './icons/XIcon.svelte';
  import TerminalIcon from './icons/TerminalIcon.svelte';
  import { totalSpeed, formatSpeed } from '../lib/transferQueueService';

  function handleTabClick(index: number) {
    $selectedTerminalIndex = index;
  }

  function handleClose(sessionId: string, event: MouseEvent) {
    event.stopPropagation();
    closeTerminal(sessionId);
  }
</script>

<div class="flex flex-col h-full w-full bg-white dark:bg-slate-950">
  <!-- Tabs Bar -->
  {#if $activeTerminals.length > 0}
    <div class="flex bg-slate-100 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 overflow-x-auto no-scrollbar">
      {#each $activeTerminals as terminal, index}
        <button
          class="group flex items-center gap-2 px-4 py-2.5 min-w-[160px] max-w-[240px] text-sm border-r border-slate-200 dark:border-slate-800 transition-colors relative
          {index === $selectedTerminalIndex 
            ? 'bg-white dark:bg-slate-950 text-blue-600 dark:text-blue-400 font-medium' 
            : 'bg-slate-100 dark:bg-slate-900 text-slate-500 dark:text-slate-400 hover:bg-slate-50 dark:hover:bg-slate-800 hover:text-slate-700 dark:hover:text-slate-200'}"
          on:click={() => handleTabClick(index)}
        >
          {#if index === $selectedTerminalIndex}
            <div class="absolute top-0 left-0 right-0 h-0.5 bg-blue-500"></div>
          {/if}
          
          <TerminalIcon class="w-4 h-4 opacity-70" />
          <span class="truncate flex-1 text-left">{terminal.connection.name}</span>
          
          <span
            role="button"
            tabindex="0"
            class="opacity-0 group-hover:opacity-100 p-1 rounded hover:bg-slate-200 dark:hover:bg-slate-700 text-slate-400 hover:text-red-500 dark:hover:text-red-400 transition-all"
            on:click={(e) => handleClose(terminal.sessionId, e)}
            on:keydown={(e) => e.key === 'Enter' && handleClose(terminal.sessionId, e as any)}
          >
            <XIcon class="w-3.5 h-3.5" />
          </span>
        </button>
      {/each}
    </div>
  {/if}

  <!-- Terminal Content Area -->
  <div class="flex-1 relative overflow-hidden">
    {#if $activeTerminals.length === 0}
      <div class="absolute inset-0 flex flex-col items-center justify-center text-slate-400 dark:text-slate-500 bg-slate-50/50 dark:bg-slate-950/50">
        <div class="w-16 h-16 mb-4 rounded-2xl bg-white dark:bg-slate-900 flex items-center justify-center border border-slate-200 dark:border-slate-800 shadow-sm">
          <TerminalIcon class="w-8 h-8 opacity-50" />
        </div>
        <p class="text-lg font-medium text-slate-600 dark:text-slate-400">无活动会话</p>
        <p class="text-sm mt-2 opacity-60">请从左侧列表选择连接以开始</p>
      </div>
    {:else}
      {#each $activeTerminals as terminal, index (terminal.sessionId)}
        <TerminalView 
          terminalData={terminal} 
          isVisible={index === $selectedTerminalIndex} 
        />
      {/each}
    {/if}
  </div>

  <!-- Status Bar -->
  <div class="flex items-center justify-between px-4 py-1.5 bg-slate-800/80 dark:bg-slate-900/90 border-t border-slate-700/50 dark:border-slate-800/50 text-xs text-slate-300 dark:text-slate-400">
    <div class="flex items-center gap-4">
      <div class="flex items-center gap-1">
        <svg class="w-3.5 h-3.5 text-blue-400" fill="currentColor" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
          <path fill-rule="evenodd" d="M12.293 5.293a1 1 0 011.414 0l4 4a1 1 0 010 1.414l-4 4a1 1 0 01-1.414-1.414L14.586 11H3a1 1 0 110-2h11.586l-2.293-2.293a1 1 0 010-1.414z" clip-rule="evenodd"></path>
        </svg>
        <span>实时流量:</span>
        <span class="font-mono font-medium text-green-300">{formatSpeed($totalSpeed)}</span>
      </div>
      <div class="text-slate-500">|</div>
      <div class="text-slate-500">
        活动传输: <span class="text-slate-300">{$activeTerminals.length}</span>
      </div>
    </div>
    <div class="text-slate-500 text-xs">
      {new Date().toLocaleTimeString([], {hour: '2-digit', minute:'2-digit'})}
    </div>
  </div>
</div>

<style>
  .no-scrollbar::-webkit-scrollbar {
    display: none;
  }
  .no-scrollbar {
    -ms-overflow-style: none;
    scrollbar-width: none;
  }
</style>
