<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount } from 'svelte';

  export let mode: 'chat' | 'agent' = 'chat';

  const dispatch = createEventDispatcher<{
    change: 'chat' | 'agent';
  }>();

  let open = false;

  const labels = {
    chat: 'Chat',
    agent: 'Agent',
  } as const;

  function selectMode(nextMode: 'chat' | 'agent') {
    open = false;
    if (nextMode === mode) return;
    dispatch('change', nextMode);
  }

  function handleWindowMouseDown(event: Event) {
    const target = event.target as HTMLElement | null;
    if (target?.closest('[data-ai-mode-switcher]')) return;
    open = false;
  }

  function handleWindowKeydown(event: Event) {
    const keyboardEvent = event as KeyboardEvent;
    if (keyboardEvent.key === 'Escape') {
      open = false;
    }
  }

  onMount(() => {
    window.addEventListener('mousedown', handleWindowMouseDown);
    window.addEventListener('keydown', handleWindowKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('mousedown', handleWindowMouseDown);
    window.removeEventListener('keydown', handleWindowKeydown);
  });
</script>

<div class="relative inline-flex" data-ai-mode-switcher>
  <button
    class="inline-flex items-center gap-1.5 rounded-md border border-app-border bg-app-bg px-2.5 py-1.5 text-xs font-medium text-app-text shadow-sm transition-colors hover:border-app-border/80 hover:text-app-text"
    on:click={() => (open = !open)}
    aria-haspopup="menu"
    aria-expanded={open}
    title="切换模式"
    type="button"
  >
    {#if mode === 'chat'}
      <svg class="w-3.5 h-3.5 opacity-75" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 7.5A1.5 1.5 0 015.5 6H10l1.5 2H18.5A1.5 1.5 0 0120 9.5v7A1.5 1.5 0 0118.5 18h-13A1.5 1.5 0 014 16.5v-9z" />
      </svg>
    {:else}
      <svg class="w-3.5 h-3.5 opacity-75" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 3a.75.75 0 01.75.75V5h3V3.75a.75.75 0 011.5 0V5h.25A2.75 2.75 0 0118 7.75v6.5A2.75 2.75 0 0115.25 17h-.635l1.118 2.236a.75.75 0 11-1.342.67L13.191 17h-2.382l-1.2 2.406a.75.75 0 01-1.342-.67L9.385 17H8.75A2.75 2.75 0 016 14.25v-6.5A2.75 2.75 0 018.75 5H9V3.75A.75.75 0 019.75 3zM8.75 6.5a1.25 1.25 0 00-1.25 1.25v6.5a1.25 1.25 0 001.25 1.25h6.5a1.25 1.25 0 001.25-1.25v-6.5a1.25 1.25 0 00-1.25-1.25h-6.5zM9.5 9.25A.75.75 0 0110.25 8.5h3.5a.75.75 0 010 1.5h-3.5a.75.75 0 01-.75-.75zm0 3a.75.75 0 01.75-.75h3.5a.75.75 0 010 1.5h-3.5a.75.75 0 01-.75-.75z" />
      </svg>
    {/if}
    <span>{labels[mode]}</span>
    <svg class="w-3.5 h-3.5 opacity-70 transition-transform {open ? 'rotate-180' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
    </svg>
  </button>

  {#if open}
    <div class="absolute left-0 bottom-full mb-2 min-w-[140px] rounded-xl border border-app-border bg-app-surface shadow-xl overflow-hidden z-30">
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs text-left transition-colors {mode === 'chat' ? 'bg-primary-600/15 text-primary-400' : 'text-app-text-secondary hover:bg-app-bg-hover hover:text-app-text'}"
        on:click={() => selectMode('chat')}
        type="button"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 7.5A1.5 1.5 0 015.5 6H10l1.5 2H18.5A1.5 1.5 0 0120 9.5v7A1.5 1.5 0 0118.5 18h-13A1.5 1.5 0 014 16.5v-9z" />
        </svg>
        <span class="font-medium">Chat</span>
      </button>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs text-left transition-colors {mode === 'agent' ? 'bg-primary-600/15 text-primary-400' : 'text-app-text-secondary hover:bg-app-bg-hover hover:text-app-text'}"
        on:click={() => selectMode('agent')}
        type="button"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9.75 3a.75.75 0 01.75.75V5h3V3.75a.75.75 0 011.5 0V5h.25A2.75 2.75 0 0118 7.75v6.5A2.75 2.75 0 0115.25 17h-.635l1.118 2.236a.75.75 0 11-1.342.67L13.191 17h-2.382l-1.2 2.406a.75.75 0 01-1.342-.67L9.385 17H8.75A2.75 2.75 0 016 14.25v-6.5A2.75 2.75 0 018.75 5H9V3.75A.75.75 0 019.75 3zM8.75 6.5a1.25 1.25 0 00-1.25 1.25v6.5a1.25 1.25 0 001.25 1.25h6.5a1.25 1.25 0 001.25-1.25v-6.5a1.25 1.25 0 00-1.25-1.25h-6.5zM9.5 9.25A.75.75 0 0110.25 8.5h3.5a.75.75 0 010 1.5h-3.5a.75.75 0 01-.75-.75zm0 3a.75.75 0 01.75-.75h3.5a.75.75 0 010 1.5h-3.5a.75.75 0 01-.75-.75z" />
        </svg>
        <span class="font-medium">Agent</span>
      </button>
    </div>
  {/if}
</div>
