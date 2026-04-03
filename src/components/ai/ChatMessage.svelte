<script lang="ts">
  import { marked } from 'marked';
  import { invoke } from '@tauri-apps/api/core';
  import type { StoredMessage } from '../../lib/aiChatService';

  export let message: StoredMessage;
  export let isStreaming = false;

  const isUser = message.role === 'user';

  // Render markdown for assistant messages
  function renderMarkdown(content: string): string {
    try {
      return marked.parse(content, { async: false }) as string;
    } catch {
      return content;
    }
  }

  // Copy code block content to clipboard
  function handleCopyCode(e: MouseEvent) {
    const btn = e.currentTarget as HTMLElement;
    const pre = btn.closest('.code-block-wrapper')?.querySelector('code');
    if (!pre) return;
    navigator.clipboard.writeText(pre.textContent ?? '');
    btn.textContent = '已复制';
    setTimeout(() => (btn.textContent = '复制'), 2000);
  }

  // "Run in terminal" button - dispatches an event up
  import { createEventDispatcher } from 'svelte';
  const dispatch = createEventDispatcher<{ runCommand: string }>();

  function handleRunCommand(e: MouseEvent) {
    const btn = e.currentTarget as HTMLElement;
    const pre = btn.closest('.code-block-wrapper')?.querySelector('code');
    if (pre) dispatch('runCommand', pre.textContent ?? '');
  }

  // Post-process rendered HTML to wrap code blocks with action buttons
  function processHtml(html: string): string {
    return html.replace(
      /<pre><code([^>]*)>([\s\S]*?)<\/code><\/pre>/g,
      `<div class="code-block-wrapper relative group my-2">
        <div class="absolute top-2 right-2 flex gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
          <button class="copy-code-btn text-xs px-2 py-0.5 rounded bg-app-surface-light text-app-text-secondary hover:text-app-text border border-app-border transition-colors">复制</button>
        </div>
        <pre><code$1>$2</code></pre>
      </div>`,
    );
  }

  $: renderedHtml = isUser
    ? ''
    : processHtml(renderMarkdown(message.content));

  // Bind the message container to attach click handlers dynamically
  let containerEl: HTMLDivElement;
  $: if (containerEl) {
    containerEl.querySelectorAll('.copy-code-btn').forEach(btn => {
      btn.removeEventListener('click', handleCopyCode as EventListener);
      btn.addEventListener('click', handleCopyCode as EventListener);
    });
  }
</script>

<div class="flex gap-3 px-4 py-3 {isUser ? 'justify-end' : 'justify-start'}">
  {#if !isUser}
    <!-- AI Avatar -->
    <div class="flex-shrink-0 w-7 h-7 rounded-full bg-primary-600 flex items-center justify-center text-white text-xs font-bold mt-0.5 select-none">
      AI
    </div>
  {/if}

  <div class="max-w-[85%] {isUser ? 'order-first' : ''}">
    <!-- Bubble -->
    <div
      class="rounded-xl px-3.5 py-2.5 text-sm leading-relaxed
        {isUser
          ? 'bg-primary-600 text-white rounded-tr-sm'
          : 'bg-app-surface border border-app-border text-app-text rounded-tl-sm'}"
    >
      {#if isUser}
        <!-- User: plain text, preserve newlines -->
        <p class="whitespace-pre-wrap break-words">{message.content}</p>
      {:else}
        <!-- Assistant: rendered Markdown -->
        <div
          bind:this={containerEl}
          class="prose prose-sm dark:prose-invert max-w-none
                 prose-code:before:content-none prose-code:after:content-none
                 prose-pre:bg-app-bg prose-pre:border prose-pre:border-app-border prose-pre:rounded-lg prose-pre:text-xs
                 prose-p:my-1 prose-ul:my-1 prose-ol:my-1 prose-li:my-0
                 {isStreaming ? 'after:animate-pulse after:content-[\"▋\"] after:ml-0.5 after:text-primary-400' : ''}"
        >
          {@html renderedHtml || (isStreaming ? '' : message.content)}
        </div>
      {/if}
    </div>

    <!-- Context snapshot indicator -->
    {#if message.context_snapshot && !isUser}
      <div class="mt-1 text-xs text-app-text-secondary flex items-center gap-1">
        <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 4H7a2 2 0 01-2-2V6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v12a2 2 0 01-2 2z" />
        </svg>
        附加了终端上下文
      </div>
    {/if}

    <!-- Timestamp -->
    <div class="mt-0.5 text-xs text-app-text-secondary {isUser ? 'text-right' : 'text-left'} opacity-50">
      {new Date(message.created_at).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit' })}
    </div>
  </div>

  {#if isUser}
    <!-- User Avatar -->
    <div class="flex-shrink-0 w-7 h-7 rounded-full bg-app-surface-light border border-app-border flex items-center justify-center text-app-text-secondary text-xs font-bold mt-0.5 select-none">
      你
    </div>
  {/if}
</div>

<style>
  :global(.code-block-wrapper pre) {
    margin: 0;
    overflow-x: auto;
  }
</style>
