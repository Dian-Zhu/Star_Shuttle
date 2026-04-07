<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount } from 'svelte';

  export let mode: 'standard' | 'full' = 'standard';

  const dispatch = createEventDispatcher<{
    change: 'standard' | 'full';
  }>();

  let open = false;

  const labels = {
    standard: '标准',
    full: 'Full',
  } as const;

  function selectMode(nextMode: 'standard' | 'full') {
    open = false;
    if (nextMode === mode) return;
    dispatch('change', nextMode);
  }

  function handleWindowMouseDown(event: Event) {
    const target = event.target as HTMLElement | null;
    if (target?.closest('[data-ai-sandbox-switcher]')) return;
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

<div class="relative inline-flex" data-ai-sandbox-switcher>
  <button
    class="inline-flex items-center gap-1.5 rounded-md border border-app-border bg-app-bg px-2.5 py-1.5 text-xs font-medium text-app-text shadow-sm transition-colors hover:border-app-border/80 hover:text-app-text"
    on:click={() => (open = !open)}
    aria-haspopup="menu"
    aria-expanded={open}
    title={mode === 'standard' ? '标准模式：启用沙箱' : 'Full 模式：关闭沙箱'}
    type="button"
  >
    <svg class="w-3.5 h-3.5 opacity-75" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
    </svg>
    <span>{labels[mode]}</span>
    <svg class="w-3.5 h-3.5 opacity-70 transition-transform {open ? 'rotate-180' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
    </svg>
  </button>

  {#if open}
    <div class="absolute left-0 bottom-full mb-2 min-w-[150px] rounded-xl border border-app-border bg-app-surface shadow-xl overflow-hidden z-30">
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs text-left transition-colors {mode === 'standard' ? 'bg-primary-600/15 text-primary-400' : 'text-app-text-secondary hover:bg-app-bg-hover hover:text-app-text'}"
        on:click={() => selectMode('standard')}
        type="button"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
        </svg>
        <div>
          <div class="font-medium">标准</div>
          <div class="mt-0.5 text-[11px] opacity-70">启用沙箱</div>
        </div>
      </button>
      <button
        class="w-full flex items-center gap-2 px-3 py-2 text-xs text-left transition-colors {mode === 'full' ? 'bg-primary-600/15 text-primary-400' : 'text-app-text-secondary hover:bg-app-bg-hover hover:text-app-text'}"
        on:click={() => selectMode('full')}
        type="button"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
        <div>
          <div class="font-medium">Full</div>
          <div class="mt-0.5 text-[11px] opacity-70">关闭沙箱</div>
        </div>
      </button>
    </div>
  {/if}
</div>
