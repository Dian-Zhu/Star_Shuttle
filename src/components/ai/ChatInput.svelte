<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let disabled = false;
  export let includeContext = false;
  export let hasActiveSession = false;

  let value = '';
  let textareaEl: HTMLTextAreaElement;

  const dispatch = createEventDispatcher<{
    send: { content: string; includeContext: boolean };
    toggleContext: boolean;
  }>();

  function autoResize() {
    if (!textareaEl) return;
    textareaEl.style.height = 'auto';
    textareaEl.style.height = Math.min(textareaEl.scrollHeight, 200) + 'px';
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }

  function submit() {
    const content = value.trim();
    if (!content || disabled) return;
    dispatch('send', { content, includeContext });
    value = '';
    if (textareaEl) {
      textareaEl.style.height = 'auto';
    }
  }

  function toggleContext() {
    includeContext = !includeContext;
    dispatch('toggleContext', includeContext);
  }

  export function focus() {
    textareaEl?.focus();
  }
</script>

<div class="border-t border-app-border bg-app-bg p-3 flex flex-col gap-2">
  <!-- Context toggle bar -->
  {#if hasActiveSession}
    <div class="flex items-center gap-2">
      <button
        class="flex items-center gap-1.5 text-xs px-2 py-1 rounded-md transition-colors
          {includeContext
            ? 'bg-primary-600/15 text-primary-400 border border-primary-500/30'
            : 'bg-app-surface text-app-text-secondary hover:text-app-text border border-app-border'}"
        on:click={toggleContext}
        title="附加当前终端内容作为上下文"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
        </svg>
        {includeContext ? '已附加终端上下文' : '附加终端上下文'}
      </button>
    </div>
  {/if}

  <!-- Input row -->
  <div class="flex items-end gap-2">
    <textarea
      bind:this={textareaEl}
      bind:value
      on:input={autoResize}
      on:keydown={handleKeydown}
      placeholder="问 AI 任何问题... (Enter 发送，Shift+Enter 换行)"
      rows="1"
      {disabled}
      class="flex-1 resize-none bg-app-surface border border-app-border rounded-lg px-3 py-2
             text-sm text-app-text placeholder-app-text-secondary
             focus:border-primary-500 focus:ring-1 focus:ring-primary-500/30 outline-none
             transition-colors disabled:opacity-50 disabled:cursor-not-allowed
             min-h-[38px] max-h-[200px] leading-relaxed"
    ></textarea>

    <button
      on:click={submit}
      {disabled}
      class="flex-shrink-0 w-9 h-9 rounded-lg bg-primary-600 hover:bg-primary-500
             text-white flex items-center justify-center transition-colors
             disabled:opacity-40 disabled:cursor-not-allowed"
      title="发送 (Enter)"
    >
      {#if disabled}
        <!-- Spinner -->
        <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
        </svg>
      {:else}
        <!-- Send icon -->
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8" />
        </svg>
      {/if}
    </button>
  </div>

  <p class="text-xs text-app-text-secondary opacity-50 select-none">
    Enter 发送 · Shift+Enter 换行
  </p>
</div>
