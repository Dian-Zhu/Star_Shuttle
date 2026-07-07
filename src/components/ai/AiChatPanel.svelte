<script lang="ts">
  import { onMount, onDestroy, afterUpdate, tick } from 'svelte';
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
    sendMessage,
    cancelMessage,
    type Conversation,
  } from '../../lib/aiChatService';
  import { filterSkillsByMode, loadSkillCatalog, skillCatalog } from '../../lib/aiSkillService';
  import { startNewTask } from '../../lib/aiAgentService';
  import ChatMessage from './ChatMessage.svelte';
  import ChatInput from './ChatInput.svelte';
  import AiAgentPanel from './AiAgentPanel.svelte';

  // Current SSH session from parent
  export let sessionId: string | null = null;

  // Chat/Agent mode tab
  export let activeTab: 'chat' | 'agent' = 'chat';
  export let showThinking = true;

  let showHistory = false;
  let messagesEndEl: HTMLDivElement;
  let inputEl: ChatInput;
  let sendError = '';
  let includeContext = false;
  let removeInsertDraftListener: (() => void) | null = null;
  let chatSkills = filterSkillsByMode([], 'chat');

  // Track last streaming message for cursor animation
  $: streamingMsgId = $isSending && $messages.length > 0
    ? $messages[$messages.length - 1].id
    : null;

  onMount(async () => {
    await loadSkillCatalog();
    await loadConversations();
    // Auto-open latest conversation if any
    if ($conversations.length > 0 && !$activeConversationId) {
      await loadMessages($conversations[0].id);
    }

    const handleInsertDraft = (event: Event) => {
      const customEvent = event as CustomEvent<string>;
      const content = customEvent.detail ?? '';
      if (!content.trim()) return;

      activeTab = 'chat';
      tick().then(() => {
        inputEl?.insertText(content, { asCodeBlock: true });
      });
    };

    window.addEventListener('ai:insert-chat-draft', handleInsertDraft as EventListener);
    removeInsertDraftListener = () => {
      window.removeEventListener('ai:insert-chat-draft', handleInsertDraft as EventListener);
    };
  });

  $: chatSkills = filterSkillsByMode($skillCatalog, 'chat');

  onDestroy(() => {
    removeInsertDraftListener?.();
    removeInsertDraftListener = null;
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

  async function handleSend(
    e: CustomEvent<{ content: string; includeContext: boolean; skillId: string | null }>,
  ) {
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
        e.detail.skillId ?? null,
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

  function handleModeChange(nextMode: 'chat' | 'agent') {
    activeTab = nextMode;
  }

  export async function startFreshChat() {
    // 按当前模式各自新建：agent 模式只重置 agent 任务，chat 模式只新建对话，
    // 两者互不影响。
    if (activeTab === 'agent') {
      startNewTask();
      return;
    }
    await handleNewChat();
  }

  export function openHistory() {
    activeTab = 'chat';
    showHistory = true;
  }
</script>

<div class="flex flex-col h-full bg-app-bg overflow-hidden">
  <!-- Agent Panel -->
  {#if activeTab === 'agent'}
    <div class="flex-1 overflow-hidden">
      <AiAgentPanel {sessionId} activeMode={activeTab} bind:showThinking on:changeMode={(e) => handleModeChange(e.detail)} />
    </div>

  <!-- Chat Panel -->
  {:else}
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
      skills={chatSkills}
      hasActiveSession={!!sessionId}
      activeMode={activeTab}
      on:send={handleSend}
      on:cancel={handleCancelSend}
      on:toggleContext={(e) => (includeContext = e.detail)}
      on:changeMode={(e) => handleModeChange(e.detail)}
    />
  {/if}
</div>
