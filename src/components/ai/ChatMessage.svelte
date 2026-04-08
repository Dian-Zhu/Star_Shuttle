<script lang="ts">
  import { tick } from 'svelte';
  import { marked } from 'marked';
  import type { StoredMessage } from '../../lib/aiChatService';

  export let message: StoredMessage;
  export let isStreaming = false;

  const isUser = message.role === 'user';

  // Render markdown for assistant messages
  function renderMarkdown(content: string): string {
    try {
      return marked.parse(content, { async: false, gfm: true, breaks: true }) as string;
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

  import { createEventDispatcher } from 'svelte';
  const dispatch = createEventDispatcher<{ runCommand: string }>();

  function handleRunCommand(e: MouseEvent) {
    const btn = e.currentTarget as HTMLElement;
    const pre = btn.closest('.code-block-wrapper')?.querySelector('code');
    if (pre) dispatch('runCommand', pre.textContent ?? '');
  }

  function isCommandContinuation(line: string): boolean {
    const trimmed = line.trim();
    if (!trimmed) return false;

    if (trimmed.startsWith('```')) return true;
    if (/[|&;$><`]/.test(trimmed)) return true;
    if (trimmed.includes('{{') || trimmed.includes('}}')) return true;
    if (/^[('"[{]/.test(trimmed) || /[)\]'}]$/.test(trimmed)) return true;
    if (/^(sudo|docker|kubectl|systemctl|journalctl|cat|grep|find|ps|top|df|du|free|uname|awk|sed|tail|head|chmod|chown|curl|wget|ssh|scp|tar|ls|cd|pwd|echo)\b/i.test(trimmed)) return true;
    if (/^[a-z0-9_.:/~-]+[\s=:].*/i.test(trimmed)) return true;
    if (/^[a-z0-9_.:/~'"-]+$/i.test(trimmed) && !/^[\u4e00-\u9fff]/.test(trimmed)) return true;

    return false;
  }

  function normalizeAssistantContent(content: string): string {
    const lines = content.split('\n');
    const output: string[] = [];

    for (let i = 0; i < lines.length; i += 1) {
      const line = lines[i];
      const match = line.match(/^\s*命令[:：]\s*(.*)$/);
      if (!match) {
        output.push(line);
        continue;
      }

      const commandLines: string[] = [];
      const firstLine = match[1]?.trim() ?? '';
      if (firstLine) {
        commandLines.push(firstLine);
      }

      while (i + 1 < lines.length) {
        const nextLine = lines[i + 1];
        const trimmedNext = nextLine.trim();
        if (!trimmedNext) break;
        if (/^\s*(说明|解释|提示|注意|输出|风险|原因|理由)[:：]/.test(trimmedNext)) break;
        if (/^\s*命令[:：]/.test(trimmedNext)) break;
        if (!isCommandContinuation(nextLine)) break;
        commandLines.push(trimmedNext);
        i += 1;
      }

      if (commandLines.length === 0) {
        output.push(line);
        continue;
      }

      output.push('命令：');
      output.push('```bash');
      output.push(commandLines.join('\n'));
      output.push('```');
    }

    return output.join('\n');
  }

  // Post-process rendered HTML to wrap code blocks with action buttons
  function processHtml(html: string): string {
    return html.replace(
      /<pre><code([^>]*)>([\s\S]*?)<\/code><\/pre>/g,
      `<div class="code-block-wrapper relative group my-2 rounded-lg border border-primary-500/25 bg-app-bg/90 overflow-hidden shadow-sm">
        <div class="flex items-center justify-between gap-2 px-3 py-2 border-b border-app-border bg-app-surface/80">
          <span class="text-[11px] font-medium tracking-wide text-primary-400 uppercase">命令</span>
          <div class="flex gap-1">
            <button type="button" class="run-command-btn text-xs px-2 py-0.5 rounded bg-app-surface-light text-app-text-secondary hover:text-app-text border border-app-border transition-colors">插入</button>
            <button type="button" class="copy-code-btn text-xs px-2 py-0.5 rounded bg-app-surface-light text-app-text-secondary hover:text-app-text border border-app-border transition-colors">复制</button>
          </div>
        </div>
        <pre><code$1>$2</code></pre>
      </div>`,
    );
  }

  $: renderedHtml = isUser
    ? ''
    : processHtml(renderMarkdown(normalizeAssistantContent(message.content)));

  // Bind the message container to attach click handlers dynamically
  let containerEl: HTMLDivElement;
  $: if (containerEl) {
    renderedHtml;
    tick().then(() => {
      if (!containerEl) return;
      containerEl.querySelectorAll('.copy-code-btn').forEach(btn => {
        btn.removeEventListener('click', handleCopyCode as EventListener);
        btn.addEventListener('click', handleCopyCode as EventListener);
      });
      containerEl.querySelectorAll('.run-command-btn').forEach(btn => {
        btn.removeEventListener('click', handleRunCommand as EventListener);
        btn.addEventListener('click', handleRunCommand as EventListener);
      });
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
    {#if message.context_snapshot}
      <div class="mt-1 text-xs text-app-text-secondary flex items-center gap-1 {isUser ? 'justify-end' : 'justify-start'}">
        <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 4H7a2 2 0 01-2-2V6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v12a2 2 0 01-2 2z" />
        </svg>
        附加了终端上下文
      </div>
    {/if}

    {#if message.context_snapshot && isUser}
      <details class="mt-1 rounded-lg border border-app-border bg-app-surface/60 px-3 py-2 text-left">
        <summary class="cursor-pointer text-xs text-app-text-secondary select-none">查看附加上下文</summary>
        <pre class="mt-2 text-xs text-app-text-secondary whitespace-pre-wrap break-all leading-relaxed max-h-40 overflow-y-auto">{message.context_snapshot}</pre>
      </details>
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
    padding: 0.875rem;
    overflow-x: auto;
    background: transparent;
    border: 0;
    border-radius: 0;
  }

  :global(.code-block-wrapper code) {
    display: block;
  }
</style>
