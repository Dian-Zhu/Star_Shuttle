<script lang="ts">
  import { onMount, afterUpdate, tick } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import {
    conversations,
    activeConversationId,
    messages,
    isSending,
    sendingConversationId,
    loadConversations,
    createConversation,
    loadMessages,
    deleteConversation,
    clearMessages,
    sendMessage,
    cancelMessage,
    type Conversation,
  } from '../../lib/aiChatService';
  import ChatMessage from './ChatMessage.svelte';
  import ChatInput from './ChatInput.svelte';
  import AiAgentPanel from './AiAgentPanel.svelte';

  // Current SSH session from parent
  export let sessionId: string | null = null;

  // Chat/Agent mode tab
  let activeTab: 'chat' | 'agent' = 'chat';

  let showHistory = false;
  let messagesEndEl: HTMLDivElement;
  let inputEl: ChatInput;
  let sendError = '';
  let includeContext = false;

  // Track last streaming message for cursor animation
  $: streamingMsgId = $isSending && $messages.length > 0
    ? $messages[$messages.length - 1].id
    : null;

  onMount(async () => {
    await loadConversations();
    // Auto-open latest conversation if any
    if ($conversations.length > 0 && !$activeConversationId) {
      await loadMessages($conversations[0].id);
    }
  });

  // Auto-scroll to bottom when messages change
  afterUpdate(() => {
    scrollToBottom();
  });

  function scrollToBottom() {
    messagesEndEl?.scrollIntoView({ behavior: 'smooth', block: 'end' });
  }

  async function handleNewChat() {
    try {
      const id = await createConversation(sessionId ?? undefined);
      await loadMessages(id);
      showHistory = false;
      await tick();
      inputEl?.focus();
    } catch (e) {
      console.error('Failed to create conversation', e);
    }
  }

  async function handleSelectConversation(conv: Conversation) {
    await loadMessages(conv.id);
    showHistory = false;
    await tick();
    inputEl?.focus();
  }

  async function handleDelete(e: MouseEvent, conv: Conversation) {
    e.stopPropagation();
    if (!confirm(`删除对话 \"${conv.title}\"？`)) return;
    await deleteConversation(conv.id);
  }

  async function handleClearMessages() {
    if (!$activeConversationId) return;
    if (!confirm('清除本对话所有消息？')) return;
    await clearMessages($activeConversationId);
  }

  async function handleSend(e: CustomEvent<{ content: string; includeContext: boolean }>) {
    if (!$activeConversationId) {
      const id = await createConversation(sessionId ?? undefined);
      await loadMessages(id);
    }

    sendError = '';
    try {
      await sendMessage(
        e.detail.content,
        $activeConversationId!,
        sessionId,
        e.detail.includeContext,
      );
    } catch (err: any) {
      const message = err?.message ?? String(err);
      if (!message.includes('Request cancelled')) {
        sendError = message;
      }
    }
  }

  async function handleCancelSend() {
    if (!$sendingConversationId) return;
    sendError = '';
    try {
      await cancelMessage($sendingConversationId);
    } catch (err: any) {
      sendError = err?.message ?? String(err);
    }
  }

  function handleRunCommand(e: CustomEvent<string>) {
    // Dispatch up to parent to inject into terminal
    // The event will bubble through RightSidebar → Layout
    window.dispatchEvent(new CustomEvent('ai:run-command', { detail: e.detail }));
  }

  $: activeConv = $conversations.find(c => c.id === $activeConversationId);
</script>

<div class="flex flex-col h-full bg-app-bg overflow-hidden">

  <!-- Mode Tab Bar -->
  <div class="flex items-center gap-1 px-3 pt-2 pb-0 border-b border-app-border bg-app-surface flex-shrink-0">
    <button
      class="px-3 py-1.5 text-xs font-medium rounded-t-md border-b-2 transition-colors
        {activeTab === 'chat'
          ? 'border-primary-500 text-primary-400'
          : 'border-transparent text-app-text-secondary hover:text-app-text'}"
      on:click={() => (activeTab = 'chat')}
    >
      💬 Chat
    </button>
    <button
      class="px-3 py-1.5 text-xs font-medium rounded-t-md border-b-2 transition-colors
        {activeTab === 'agent'
          ? 'border-primary-500 text-primary-400'
          : 'border-transparent text-app-text-secondary hover:text-app-text'}"
      on:click={() => (activeTab = 'agent')}
    >
      🤖 Agent
    </button>
  </div>

  <!-- Agent Panel -->
  {#if activeTab === 'agent'}
    <div class="flex-1 overflow-hidden">
      <AiAgentPanel {sessionId} />
    </div>

  <!-- Chat Panel -->
  {:else}
    <!-- Chat Header -->
    <div class="flex items-center justify-between px-3 py-2.5 border-b border-app-border flex-shrink-0 bg-app-surface">
      <div class="flex items-center gap-2 min-w-0">
        <!-- History toggle -->
        <button
          class="p-1.5 rounded-md hover:bg-app-bg-hover text-app-text-secondary hover:text-app-text transition-colors"
          on:click={() => (showHistory = !showHistory)}
          title="对话历史"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
              d="M4 6h16M4 12h16M4 18h7" />
          </svg>
        </button>

        <span class="text-sm font-medium text-app-text truncate">
          {activeConv?.title ?? 'AI 助手'}
        </span>
      </div>

      <div class="flex items-center gap-1">
        <!-- Clear messages -->
        {#if $activeConversationId && $messages.length > 0}
          <button
            class="p-1.5 rounded-md hover:bg-app-bg-hover text-app-text-secondary hover:text-app-text transition-colors"
            on:click={handleClearMessages}
            title="清除消息"
          >
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
            </svg>
          </button>
        {/if}

        <!-- New Chat -->
        <button
          class="p-1.5 rounded-md hover:bg-app-bg-hover text-app-text-secondary hover:text-app-text transition-colors"
          on:click={handleNewChat}
          title="新建对话"
        >
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
              d="M12 4v16m8-8H4" />
          </svg>
        </button>
      </div>
    </div>

    <!-- History Drawer (slide in) -->
    {#if showHistory}
      <div
        class="absolute top-0 left-0 right-0 bottom-0 z-20 flex flex-col bg-app-bg"
        transition:slide={{ duration: 180, axis: 'x' }}
      >
        <div class="flex items-center justify-between px-3 py-2.5 border-b border-app-border bg-app-surface">
          <span class="text-sm font-medium text-app-text">对话历史</span>
          <button
            class="p-1.5 rounded-md hover:bg-app-bg-hover text-app-text-secondary hover:text-app-text transition-colors"
            on:click={() => (showHistory = false)}
            aria-label="关闭对话历史"
            title="关闭"
          >
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>

        <div class="flex-1 overflow-y-auto p-2 space-y-1">
          {#if $conversations.length === 0}
            <p class="text-sm text-app-text-secondary text-center py-8">暂无对话历史</p>
          {:else}
            {#each $conversations as conv (conv.id)}
              <div
                class="w-full text-left px-3 py-2 rounded-lg text-sm transition-colors flex items-center justify-between group cursor-pointer
                  {conv.id === $activeConversationId
                    ? 'bg-primary-600/15 text-primary-400'
                    : 'hover:bg-app-surface text-app-text-secondary hover:text-app-text'}"
                on:click={() => handleSelectConversation(conv)}
                role="button"
                tabindex="0"
                on:keydown={(e) => e.key === 'Enter' && handleSelectConversation(conv)}
              >
                <div class="min-w-0 flex-1">
                  <div class="font-medium truncate">{conv.title}</div>
                  <div class="text-xs opacity-60 mt-0.5">
                    {new Date(conv.updated_at).toLocaleDateString('zh-CN')}
                  </div>
                </div>
                <button
                  class="flex-shrink-0 p-1 rounded opacity-0 group-hover:opacity-100 hover:bg-red-500/20 hover:text-red-400 transition-all ml-2"
                  on:click={(e) => handleDelete(e, conv)}
                  title="删除"
                >
                  <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                  </svg>
                </button>
              </div>
            {/each}
          {/if}
        </div>

        <div class="p-2 border-t border-app-border">
          <button
            class="w-full py-2 rounded-lg bg-primary-600 hover:bg-primary-500 text-white text-sm font-medium transition-colors"
            on:click={handleNewChat}
          >
            + 新建对话
          </button>
        </div>
      </div>
    {/if}

    <!-- Messages area -->
    <div class="flex-1 overflow-y-auto py-2 relative" id="ai-messages-container">
      {#if !$activeConversationId || $messages.length === 0}
        <!-- Empty state -->
        <div class="h-full flex flex-col items-center justify-center text-center px-6 py-12 select-none">
          <div class="w-12 h-12 rounded-2xl bg-primary-600/20 flex items-center justify-center mb-4">
            <svg class="w-6 h-6 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                d="M9.663 17h4.673M12 3v1m6.364 1.636l-.707.707M21 12h-1M4 12H3m3.343-5.657l-.707-.707m2.828 9.9a5 5 0 117.072 0l-.548.547A3.374 3.374 0 0014 18.469V19a2 2 0 11-4 0v-.531c0-.895-.356-1.754-.988-2.386l-.548-.547z" />
            </svg>
          </div>
          <h3 class="text-sm font-semibold text-app-text mb-1">AI 助手</h3>
          <p class="text-xs text-app-text-secondary leading-relaxed">
            可以帮你分析终端输出、诊断错误、<br>解释命令或提供操作建议
          </p>
          {#if sessionId}
            <p class="text-xs text-primary-400 mt-3">
              已连接终端 · 可附加上下文发送
            </p>
          {/if}
        </div>
      {:else}
        {#each $messages as msg (msg.id)}
          <ChatMessage
            message={msg}
            isStreaming={$isSending && msg.id === streamingMsgId}
            on:runCommand={handleRunCommand}
          />
        {/each}
      {/if}

      <!-- Error toast -->
      {#if sendError}
        <div
          class="mx-3 my-1 px-3 py-2 bg-red-500/10 border border-red-500/20 text-red-400 text-xs rounded-lg"
          transition:fade={{ duration: 200 }}
        >
          {sendError}
        </div>
      {/if}

      <!-- Scroll anchor -->
      <div bind:this={messagesEndEl}></div>
    </div>

    <!-- Input -->
    <ChatInput
      bind:this={inputEl}
      disabled={false}
      isSending={$isSending}
      {includeContext}
      hasActiveSession={!!sessionId}
      on:send={handleSend}
      on:cancel={handleCancelSend}
      on:toggleContext={(e) => (includeContext = e.detail)}
    />
  {/if}
</div>
