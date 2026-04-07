<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import AiModeSwitcher from './AiModeSwitcher.svelte';

  export let disabled = false;
  export let isSending = false;
  export let includeContext = false;
  export let hasActiveSession = false;
  export let activeMode: 'chat' | 'agent' = 'chat';

  let value = '';
  let textareaEl: HTMLTextAreaElement;

  const dispatch = createEventDispatcher<{
    send: { content: string; includeContext: boolean };
    cancel: void;
    toggleContext: boolean;
    changeMode: 'chat' | 'agent';
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
    if (!content || disabled || isSending) return;
    dispatch('send', { content, includeContext });
    value = '';
    if (textareaEl) {
      textareaEl.style.height = 'auto';
    }
  }

  function cancel() {
    if (!isSending) return;
    dispatch('cancel');
  }

  function toggleContext() {
    includeContext = !includeContext;
    dispatch('toggleContext', includeContext);
  }

  export function insertText(text: string, options: { asCodeBlock?: boolean } = {}) {
    const normalized = text.replace(/\r\n/g, '\n').trim();
    if (!normalized) return;

    const inserted = options.asCodeBlock
      ? `\`\`\`text\n${normalized}\n\`\`\``
      : normalized;

    value = value.trim()
      ? `${value.replace(/\s+$/, '')}\n\n${inserted}`
      : inserted;

    requestAnimationFrame(() => {
      autoResize();
      textareaEl?.focus();
      textareaEl?.setSelectionRange(value.length, value.length);
    });
  }

  export function focus() {
    textareaEl?.focus();
  }
</script>

<div class="border-t border-app-border bg-app-bg p-3">
  <div class="rounded-[18px] bg-app-surface px-3 py-2.5 shadow-[0_10px_30px_rgba(0,0,0,0.12)]">
    <textarea
      bind:this={textareaEl}
      bind:value
      on:input={autoResize}
      on:keydown={handleKeydown}
      placeholder="提问或输入“/”快捷命令"
      rows="1"
      disabled={disabled || isSending}
      class="w-full resize-none bg-transparent px-0 py-0 text-sm text-app-text placeholder-app-text-secondary/80
             outline-none transition-colors disabled:opacity-50 disabled:cursor-not-allowed
             min-h-[34px] max-h-[200px] leading-relaxed"
    ></textarea>

    <div class="mt-2 flex items-center gap-2">
      <AiModeSwitcher mode={activeMode} on:change={(e) => dispatch('changeMode', e.detail)} />

      <div class="ml-auto flex items-center gap-1.5">
        {#if hasActiveSession}
          <button
            class="inline-flex h-8 w-8 items-center justify-center rounded-full border transition-colors
              {includeContext
                ? 'border-primary-500/40 bg-primary-600/15 text-primary-400'
                : 'border-app-border bg-app-bg text-app-text-secondary hover:text-app-text hover:border-app-border/80'}"
            on:click={toggleContext}
            title={includeContext ? '已附加终端上下文' : '附加终端上下文'}
            type="button"
          >
            <svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
            </svg>
          </button>
        {/if}

        <button
          on:click={isSending ? cancel : submit}
          disabled={disabled}
          class="inline-flex h-8 w-8 items-center justify-center rounded-full border transition-colors
            disabled:opacity-40 disabled:cursor-not-allowed
            {isSending
              ? 'border-red-500/30 bg-red-500/12 text-red-400 hover:bg-red-500/20'
              : 'border-app-border bg-app-bg text-app-text-secondary hover:text-app-text hover:border-app-border/80'}"
          title={isSending ? '暂停生成' : '发送'}
          type="button"
        >
          {#if isSending}
            <svg class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
              <path d="M8 7a1 1 0 011 1v8a1 1 0 11-2 0V8a1 1 0 011-1zm8 0a1 1 0 011 1v8a1 1 0 11-2 0V8a1 1 0 011-1z" />
            </svg>
          {:else}
            <svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 12h12M13 4l8 8-8 8" />
            </svg>
          {/if}
        </button>
      </div>
    </div>
  </div>
</div>
