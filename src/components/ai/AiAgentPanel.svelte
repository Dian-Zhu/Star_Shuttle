<script lang="ts">
  import { createEventDispatcher } from 'svelte';
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
  {/if}
</div>
