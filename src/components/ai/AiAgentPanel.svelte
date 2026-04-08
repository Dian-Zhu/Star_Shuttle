<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount } from 'svelte';
  import { tick } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import { marked } from 'marked';
  import {
    activeTask,
    activeTaskEvents,
    sandboxMode,
    pendingConfirm,
    taskHistory,
    startAgent,
    confirmStep,
    cancelTask,
    loadTaskHistory,
    openTask,
    cleanup,
  } from '../../lib/aiAgentService';
  import { formatAgentEventLabel, formatAgentEventSummary } from '../../lib/agentEventFormatter';
  import AgentToolCallStep from './AgentToolCallStep.svelte';
  import AiModeSwitcher from './AiModeSwitcher.svelte';
  import AiSandboxSwitcher from './AiSandboxSwitcher.svelte';
  import CommandConfirmModal from './CommandConfirmModal.svelte';

  export let sessionId: string | null = null;
  export let activeMode: 'chat' | 'agent' = 'agent';
  export let showThinking = true;

  const dispatch = createEventDispatcher<{
    changeMode: 'chat' | 'agent';
  }>();

  let instruction = '';
  let isStarting = false;
  let isCancelling = false;
  let startError = '';
  let stepsEl: HTMLDivElement;

  onMount(() => {
    void loadTaskHistory(sessionId);
  });

  onDestroy(() => {
    cleanup();
  });

  $: if (sessionId) {
    void loadTaskHistory(sessionId);
  }

  $: isRunning = ['queued', 'planning', 'executing', 'waiting_confirm', 'retrying', 'cancelling'].includes($activeTask?.status ?? '');
  $: isWaiting = $activeTask?.status === 'waiting_confirm';
  $: isDone = ['completed', 'failed', 'cancelled'].includes($activeTask?.status ?? '');
  $: canCancel = !!$activeTask && ['queued', 'planning', 'executing', 'waiting_confirm', 'retrying', 'cancelling'].includes($activeTask.status);
  $: if ($activeTask?.status !== 'cancelling') {
    isCancelling = false;
  }

  $: if ($activeTask?.steps) {
    tick().then(() => stepsEl?.lastElementChild?.scrollIntoView({ behavior: 'smooth', block: 'end' }));
  }

  async function handleStart() {
    if (!sessionId) {
      startError = '请先连接一个终端会话';
      return;
    }
    const text = instruction.trim();
    if (!text) return;

    isStarting = true;
    startError = '';

    try {
      await startAgent(sessionId, text, $sandboxMode);
      instruction = '';
    } catch (e: any) {
      startError = e?.message ?? String(e);
    } finally {
      isStarting = false;
    }
  }

  async function handleConfirm(e: CustomEvent<boolean>) {
    if (!$pendingConfirm) return;
    await confirmStep($pendingConfirm.task_id, e.detail);
  }

  async function handleCancel() {
    if (!$activeTask || isCancelling) return;

    isCancelling = true;
    startError = '';

    try {
      await cancelTask($activeTask.id);
    } catch (e: any) {
      startError = e?.message ?? String(e);
      isCancelling = false;
    }
  }

  async function handleOpenHistory(taskId: string) {
    startError = '';
    await openTask(taskId);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleStart();
    }
  }

  const STATUS_LABELS: Record<string, string> = {
    queued: '排队中',
    planning: '规划中',
    executing: '执行中',
    waiting_confirm: '等待确认',
    retrying: '重试中',
    cancelling: '停止中',
    completed: '已完成',
    failed: '失败',
    cancelled: '已取消',
  };

  const STATUS_COLORS: Record<string, string> = {
    queued: 'text-app-text-secondary',
    planning: 'text-blue-400',
    executing: 'text-blue-400',
    waiting_confirm: 'text-yellow-400',
    retrying: 'text-orange-400',
    cancelling: 'text-app-text-secondary',
    completed: 'text-green-400',
    failed: 'text-red-400',
    cancelled: 'text-app-text-secondary',
  };

  const DONE_CARD_STYLES: Record<string, string> = {
    completed: 'bg-green-500/10 border-green-500/20',
    failed: 'bg-red-500/10 border-red-500/20',
    cancelled: 'bg-app-surface border-app-border',
  };

  function renderMarkdown(content: string): string {
    try {
      return marked.parse(content, { async: false, gfm: true, breaks: true }) as string;
    } catch {
      return content;
    }
  }

  $: finalResultStep = [...($activeTask?.steps ?? [])]
    .reverse()
    .find((step) => step.kind === 'result' && (step.output?.trim() || step.title?.trim()));
  $: summaryText = $activeTask?.summary?.trim() || $activeTask?.final_result?.summary?.trim() || finalResultStep?.output?.trim() || '';
  $: summaryHtml = summaryText ? renderMarkdown(summaryText) : '';
  $: doneCardStyle = $activeTask ? DONE_CARD_STYLES[$activeTask.status] ?? 'bg-app-surface border-app-border' : 'bg-app-surface border-app-border';
  $: visibleSteps = ($activeTask?.steps ?? []).filter((step) => showThinking || step.kind !== 'planning');
  $: hasStructuredOutcome = !!summaryText || !!$activeTask?.error_message;
  $: recentEvents = [...$activeTaskEvents].slice(-6).reverse();
</script>

<style>
  :global(.agent-summary-markdown pre) {
    margin: 0.5rem 0 0;
    overflow-x: auto;
  }
</style>

{#if $pendingConfirm}
  <CommandConfirmModal confirm={$pendingConfirm} on:confirm={handleConfirm} />
{/if}

<div class="flex flex-col h-full bg-app-bg overflow-hidden">
  <div class="flex-1 overflow-y-auto py-2" bind:this={stepsEl}>
    {#if !$activeTask}
      <div class="h-full flex flex-col items-center justify-center text-center px-6 py-12 select-none">
        <div class="w-12 h-12 rounded-2xl bg-primary-600/20 flex items-center justify-center mb-4">
          <svg class="w-6 h-6 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
              d="M9 3H5a2 2 0 00-2 2v4m6-6h10a2 2 0 012 2v4M9 3v18m0 0h10a2 2 0 002-2v-4M9 21H5a2 2 0 01-2-2v-4m0 0h18" />
          </svg>
        </div>
        <h3 class="text-sm font-semibold text-app-text mb-1">Agent 自动执行</h3>
        <p class="text-xs text-app-text-secondary leading-relaxed">
          描述你的目标，Agent 将自动规划<br>并分步执行操作
        </p>
        <div class="mt-4 space-y-1.5 text-left w-full max-w-xs">
          {#each ['查看 nginx 日志中的错误', '检查磁盘使用情况并找出大文件', '查看所有运行中的 docker 容器'] as example}
            <button
              class="w-full text-left text-xs px-3 py-2 rounded-lg bg-app-surface border border-app-border text-app-text-secondary hover:text-app-text hover:border-primary-500/30 transition-colors"
              on:click={() => { instruction = example; }}
            >
              {example}
            </button>
          {/each}
        </div>

        {#if !sessionId}
          <p class="text-xs text-orange-400 mt-4">⚠ 请先连接一个终端会话</p>
        {/if}
      </div>
    {:else}
      <div class="px-3 py-2 mx-3 mb-1 bg-primary-600/10 border border-primary-500/20 rounded-lg">
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0">
            <p class="text-xs text-primary-400 font-medium">任务</p>
            <p class="text-sm text-app-text mt-0.5 truncate">{$activeTask.instruction}</p>
          </div>
          <div class="text-right">
            <p class="text-xs text-app-text-secondary">事件</p>
            <p class="text-sm text-app-text">{$activeTaskEvents.length}</p>
          </div>
        </div>
      </div>

      <div class="px-1">
        {#each visibleSteps as step (step.id)}
          <AgentToolCallStep {step} />
        {/each}

        {#if isDone}
          <div class="px-2 py-2">
            <div class="rounded-lg border p-3 {doneCardStyle}" transition:fade={{ duration: 200 }}>
              <div class="flex items-center justify-between gap-3">
                <div>
                  <p class="text-xs text-app-text-secondary">处理结果</p>
                  <p class="text-sm font-medium {STATUS_COLORS[$activeTask.status]}">
                    {STATUS_LABELS[$activeTask.status]}
                  </p>
                </div>
                {#if summaryText}
                  <span class="text-xs text-app-text-secondary">已生成最终摘要</span>
                {:else if $activeTask.status === 'failed'}
                  <span class="text-xs text-red-400">未生成最终结果</span>
                {:else if $activeTask.status === 'cancelled'}
                  <span class="text-xs text-app-text-secondary">任务已被用户终止</span>
                {/if}
              </div>

              {#if summaryText}
                <div
                  class="agent-summary-markdown mt-2 prose prose-sm dark:prose-invert max-w-none text-app-text
                         prose-code:before:content-none prose-code:after:content-none
                         prose-pre:bg-app-bg prose-pre:border prose-pre:border-app-border prose-pre:rounded-lg prose-pre:text-xs
                         prose-p:my-1 prose-ul:my-1 prose-ol:my-1 prose-li:my-0 break-words"
                >
                  {@html summaryHtml}
                </div>
              {/if}

              {#if $activeTask.error_message}
                <div class="mt-2 rounded-md border border-red-500/20 bg-red-500/10 px-2.5 py-2 text-xs text-red-400 whitespace-pre-wrap break-words">
                  {$activeTask.error_message}
                </div>
              {/if}

              {#if !hasStructuredOutcome}
                <div class="mt-2 rounded-md border border-yellow-500/20 bg-yellow-500/10 px-2.5 py-2 text-xs text-yellow-300 whitespace-pre-wrap break-words">
                  当前任务没有生成可展示的最终结果。
                </div>
              {/if}
            </div>
          </div>
        {/if}

        {#if isRunning && !isWaiting}
          <div class="flex items-center gap-2 px-3 py-2 text-xs text-blue-400">
            <div class="w-3 h-3 rounded-full border-2 border-blue-400 border-t-transparent animate-spin"></div>
            {#if $activeTask?.status === 'retrying'}
              AI 正在重试...
            {:else if $activeTask?.status === 'cancelling'}
              正在停止当前任务...
            {:else if $activeTask?.status === 'planning'}
              AI 正在规划...
            {:else}
              AI 正在执行...
            {/if}
          </div>
        {/if}
      </div>

      {#if $activeTask.error_message && !isDone}
        <div
          class="mx-3 mt-2 p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-red-400 text-xs"
          transition:fade={{ duration: 200 }}
        >
          {$activeTask.error_message}
        </div>
      {/if}

      {#if recentEvents.length}
        <div class="mx-3 mt-2 rounded-lg border border-app-border bg-app-surface/40 p-3">
          <div class="flex items-center justify-between gap-3 mb-2">
            <p class="text-xs font-medium text-app-text-secondary">最近事件</p>
            <p class="text-[11px] text-app-text-secondary">共 {$activeTaskEvents.length} 条</p>
          </div>
          <div class="space-y-1.5">
            {#each recentEvents as event (event.id)}
              <div class="rounded-md bg-app-bg border border-app-border px-2.5 py-2">
                <div class="flex items-center justify-between gap-3">
                  <p class="text-xs text-app-text">{formatAgentEventLabel(event.event_type)}</p>
                  <p class="text-[11px] text-app-text-secondary">#{event.seq}</p>
                </div>
                <p class="mt-1 text-[11px] text-app-text-secondary whitespace-pre-wrap break-all leading-relaxed">
                  {formatAgentEventSummary(event)}
                </p>
                <details class="mt-1">
                  <summary class="cursor-pointer text-[11px] text-app-text-secondary hover:text-app-text">
                    查看原始数据
                  </summary>
                  <pre class="mt-1 text-[11px] text-app-text-secondary whitespace-pre-wrap break-all leading-relaxed">{JSON.stringify(event.payload_json, null, 2)}</pre>
                </details>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    {/if}
  </div>

  {#if $taskHistory.length}
    <div class="border-t border-app-border bg-app-surface/40 px-3 py-2">
      <div class="flex items-center justify-between gap-3 mb-2">
        <p class="text-xs font-medium text-app-text-secondary">最近任务</p>
        <button
          class="text-xs text-app-text-secondary hover:text-app-text transition-colors"
          on:click={() => loadTaskHistory(sessionId)}
          type="button"
        >
          刷新
        </button>
      </div>
      <div class="space-y-1.5 max-h-32 overflow-y-auto">
        {#each $taskHistory as item (item.id)}
          <button
            class="w-full text-left rounded-lg border px-3 py-2 transition-colors
              {$activeTask?.id === item.id
                ? 'bg-primary-600/10 border-primary-500/20'
                : 'bg-app-bg border-app-border hover:border-app-border/80'}"
            on:click={() => handleOpenHistory(item.id)}
            type="button"
          >
            <div class="flex items-center justify-between gap-3">
              <p class="text-xs text-app-text truncate">{item.instruction}</p>
              <span class="text-[11px] {STATUS_COLORS[item.status]}">{STATUS_LABELS[item.status]}</span>
            </div>
            {#if item.summary || item.error_message}
              <p class="text-[11px] text-app-text-secondary mt-1 truncate">{item.summary || item.error_message}</p>
            {/if}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  {#if !isRunning}
    <div class="border-t border-app-border bg-app-bg p-3" transition:slide={{ duration: 150 }}>
      {#if startError}
        <p class="mb-2 text-xs text-red-400">{startError}</p>
      {/if}

      <div class="rounded-[18px] bg-app-surface px-3 py-2.5 shadow-[0_10px_30px_rgba(0,0,0,0.12)]">
        <textarea
          bind:value={instruction}
          on:keydown={handleKeydown}
          placeholder="描述你想让 Agent 做什么..."
          rows="2"
          disabled={isStarting || !sessionId}
          class="w-full resize-none bg-transparent px-0 py-0 text-sm text-app-text placeholder-app-text-secondary/80
                 outline-none transition-colors disabled:opacity-50 disabled:cursor-not-allowed
                 min-h-[48px] leading-relaxed"
        ></textarea>

        <div class="mt-2 flex items-center gap-2">
          <AiModeSwitcher mode={activeMode} on:change={(e) => dispatch('changeMode', e.detail)} />
          <AiSandboxSwitcher mode={$sandboxMode} on:change={(e) => sandboxMode.set(e.detail)} />
          <div class="ml-auto">
            <button
              on:click={handleStart}
              disabled={isStarting || !sessionId || !instruction.trim()}
              class="inline-flex h-8 w-8 items-center justify-center rounded-full border transition-colors
                disabled:opacity-40 disabled:cursor-not-allowed
                {isStarting
                  ? 'border-primary-500/30 bg-primary-600/12 text-primary-400'
                  : 'border-app-border bg-app-bg text-app-text-secondary hover:text-app-text hover:border-app-border/80'}"
              title="开始执行"
              type="button"
            >
              {#if isStarting}
                <svg class="h-4 w-4 animate-spin" fill="none" viewBox="0 0 24 24">
                  <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
                  <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
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
  {:else}
    <div class="border-t border-app-border bg-app-bg p-3">
      {#if startError}
        <p class="mb-2 text-xs text-red-400">{startError}</p>
      {/if}
      <div class="flex items-center justify-between gap-3 rounded-[18px] bg-app-surface px-3 py-2.5 shadow-[0_10px_30px_rgba(0,0,0,0.12)]">
        <div class="min-w-0">
          <p class="text-sm text-app-text">{STATUS_LABELS[$activeTask?.status ?? 'planning']}</p>
          <p class="text-xs text-app-text-secondary mt-0.5 truncate">
            {$activeTask?.instruction}
          </p>
        </div>
        <button
          on:click={handleCancel}
          disabled={!canCancel || isCancelling}
          class="inline-flex h-8 items-center justify-center rounded-full border px-3 text-xs transition-colors
            disabled:opacity-40 disabled:cursor-not-allowed
            border-red-500/30 bg-red-500/10 text-red-300 hover:bg-red-500/20"
          title="停止当前任务"
          type="button"
        >
          {#if isCancelling || $activeTask?.status === 'cancelling'}
            停止中
          {:else}
            停止
          {/if}
        </button>
      </div>
    </div>
  {/if}
</div>
