<script lang="ts">
  import { tick } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import { marked } from 'marked';
  import {
    currentTask,
    sandboxMode,
    pendingConfirm,
    startAgent,
    confirmStep,
    cancelTask,
    cleanup,
  } from '../../lib/aiAgentService';
  import AgentToolCallStep from './AgentToolCallStep.svelte';
  import CommandConfirmModal from './CommandConfirmModal.svelte';

  export let sessionId: string | null = null;

  // Show AI thinking steps toggle
  let showThinking = true;

  let instruction = '';
  let isStarting = false;
  let startError = '';
  let stepsEl: HTMLDivElement;

  $: isRunning = $currentTask?.status === 'running' || $currentTask?.status === 'waiting_confirm';
  $: isWaiting = $currentTask?.status === 'waiting_confirm';
  $: isDone = $currentTask?.status === 'completed' || $currentTask?.status === 'failed' || $currentTask?.status === 'cancelled';

  // Auto-scroll steps
  $: if ($currentTask?.steps) {
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
    if (!$currentTask) return;
    await cancelTask($currentTask.id);
  }

  function handleNewTask() {
    cleanup();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleStart();
    }
  }

  const STATUS_LABELS: Record<string, string> = {
    running: '运行中',
    waiting_confirm: '等待确认',
    completed: '已完成',
    failed: '失败',
    cancelled: '已取消',
  };

  const STATUS_COLORS: Record<string, string> = {
    running: 'text-blue-400',
    waiting_confirm: 'text-yellow-400',
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

  $: finalResultStep = [...($currentTask?.steps ?? [])]
    .reverse()
    .find((step) => step.kind === 'result');
  $: fallbackOutputStep = [...($currentTask?.steps ?? [])]
    .reverse()
    .find((step) => step.output && step.status !== 'running' && step.kind !== 'thinking');
  $: summaryText = finalResultStep?.output || finalResultStep?.description || fallbackOutputStep?.output || fallbackOutputStep?.description || '';
  $: summaryHtml = summaryText ? renderMarkdown(summaryText) : '';
  $: doneCardStyle = $currentTask ? DONE_CARD_STYLES[$currentTask.status] ?? 'bg-app-surface border-app-border' : 'bg-app-surface border-app-border';
  $: visibleSteps = ($currentTask?.steps ?? []).filter((step) => showThinking || (step.kind !== 'thinking' && step.kind !== 'execute_command' && step.kind !== 'result'));
</script>

<style>
  :global(.agent-summary-markdown pre) {
    margin: 0.5rem 0 0;
    overflow-x: auto;
  }
</style>

<!-- Confirm Modal -->

{#if $pendingConfirm}
  <CommandConfirmModal confirm={$pendingConfirm} on:confirm={handleConfirm} />
{/if}

<div class="flex flex-col h-full bg-app-bg overflow-hidden">

  <!-- Header -->
  <div class="flex items-center justify-between px-3 py-2.5 border-b border-app-border flex-shrink-0 bg-app-surface">
    <div class="flex items-center gap-2">
      <svg class="w-4 h-4 text-primary-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
          d="M9 3H5a2 2 0 00-2 2v4m6-6h10a2 2 0 012 2v4M9 3v18m0 0h10a2 2 0 002-2v-4M9 21H5a2 2 0 01-2-2v-4m0 0h18" />
      </svg>
      <span class="text-sm font-medium text-app-text">Agent 模式</span>

      {#if $currentTask}
        <span class="text-xs {STATUS_COLORS[$currentTask.status]}">
          · {STATUS_LABELS[$currentTask.status]}
        </span>
      {/if}
    </div>

    <div class="flex items-center gap-2">
      <!-- Thinking toggle -->
      <button
        class="text-xs px-2 py-1 rounded-md border transition-colors
          {showThinking
            ? 'bg-primary-600/10 border-primary-500/30 text-primary-400'
            : 'bg-app-bg border-app-border text-app-text-secondary hover:text-app-text'}"
        on:click={() => (showThinking = !showThinking)}
        title={showThinking ? '隐藏思考过程' : '显示思考过程'}
      >
        {showThinking ? '🧠 思考' : '🧠'}
      </button>

      <!-- Sandbox mode toggle -->
      <div class="flex items-center gap-1 bg-app-bg border border-app-border rounded-lg p-0.5">
        <button
          class="text-xs px-2 py-1 rounded-md transition-colors
            {$sandboxMode === 'standard'
              ? 'bg-app-surface text-app-text shadow-sm'
              : 'text-app-text-secondary hover:text-app-text'}"
          on:click={() => sandboxMode.set('standard')}
          title="标准沙箱：白名单放行"
        >
          标准
        </button>
        <button
          class="text-xs px-2 py-1 rounded-md transition-colors
            {$sandboxMode === 'strict'
              ? 'bg-app-surface text-app-text shadow-sm'
              : 'text-app-text-secondary hover:text-app-text'}"
          on:click={() => sandboxMode.set('strict')}
          title="严格沙箱：黑名单拦截"
        >
          严格
        </button>
      </div>

      <!-- Cancel/New task button -->
      {#if isRunning}
        <button
          class="text-xs px-2 py-1 rounded-md bg-red-500/10 text-red-400 hover:bg-red-500/20 transition-colors border border-red-500/20"
          on:click={handleCancel}
        >
          停止
        </button>
      {:else if isDone}
        <button
          class="text-xs px-2 py-1 rounded-md bg-app-surface text-app-text-secondary hover:text-app-text transition-colors border border-app-border"
          on:click={handleNewTask}
        >
          新任务
        </button>
      {/if}
    </div>
  </div>

  <!-- Steps List -->
  <div class="flex-1 overflow-y-auto py-2" bind:this={stepsEl}>
    {#if !$currentTask}
      <!-- Empty state -->
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
      <!-- Instruction reminder -->
      <div class="px-3 py-2 mx-3 mb-1 bg-primary-600/10 border border-primary-500/20 rounded-lg">
        <p class="text-xs text-primary-400 font-medium">任务</p>
        <p class="text-sm text-app-text mt-0.5">{$currentTask.instruction}</p>
      </div>

      <!-- Steps timeline -->
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
                  <p class="text-sm font-medium {STATUS_COLORS[$currentTask.status]}">
                    {STATUS_LABELS[$currentTask.status]}
                  </p>
                </div>
                {#if finalResultStep || fallbackOutputStep}
                  <span class="text-xs text-app-text-secondary">已生成最终摘要</span>
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

              {#if $currentTask.error}
                <div class="mt-2 rounded-md border border-red-500/20 bg-red-500/10 px-2.5 py-2 text-xs text-red-400 whitespace-pre-wrap break-words">
                  {$currentTask.error}
                </div>
              {/if}
            </div>
          </div>
        {/if}

        <!-- Spinning indicator when running -->
        {#if isRunning && !isWaiting}
          <div class="flex items-center gap-2 px-3 py-2 text-xs text-blue-400">
            <div class="w-3 h-3 rounded-full border-2 border-blue-400 border-t-transparent animate-spin"></div>
            AI 正在思考...
          </div>
        {/if}
      </div>

      <!-- Error display -->
      {#if $currentTask.error && !isDone}
        <div
          class="mx-3 mt-2 p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-red-400 text-xs"
          transition:fade={{ duration: 200 }}
        >
          {$currentTask.error}
        </div>
      {/if}
    {/if}
  </div>

  <!-- Input Area -->
  {#if !isRunning}
    <div class="border-t border-app-border bg-app-bg p-3 flex flex-col gap-2" transition:slide={{ duration: 150 }}>
      {#if startError}
        <p class="text-xs text-red-400">{startError}</p>
      {/if}

      <!-- Sandbox mode indicator -->
      <div class="flex items-center gap-1.5 text-xs text-app-text-secondary">
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
            d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
        </svg>
        <span>
          {$sandboxMode === 'standard' ? '标准沙箱（白名单）' : '严格沙箱（黑名单）'}
          · 高危命令需确认
        </span>
      </div>

      <div class="flex gap-2">
        <textarea
          bind:value={instruction}
          on:keydown={handleKeydown}
          placeholder="描述你想让 Agent 做什么... (Enter 发送)"
          rows="2"
          disabled={isStarting || !sessionId}
          class="flex-1 resize-none bg-app-surface border border-app-border rounded-lg px-3 py-2
                 text-sm text-app-text placeholder-app-text-secondary
                 focus:border-primary-500 outline-none transition-colors
                 disabled:opacity-50 disabled:cursor-not-allowed"
        ></textarea>
        <button
          on:click={handleStart}
          disabled={isStarting || !sessionId || !instruction.trim()}
          class="flex-shrink-0 w-9 h-auto rounded-lg bg-primary-600 hover:bg-primary-500
                 text-white flex items-center justify-center transition-colors
                 disabled:opacity-40 disabled:cursor-not-allowed self-end mb-0"
          style="height: 38px;"
        >
          {#if isStarting}
            <svg class="w-4 h-4 animate-spin" fill="none" viewBox="0 0 24 24">
              <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
              <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
            </svg>
          {:else}
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z" />
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          {/if}
        </button>
      </div>
    </div>
  {/if}
</div>
