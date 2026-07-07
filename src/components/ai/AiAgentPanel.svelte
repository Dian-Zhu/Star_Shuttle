<script lang="ts">
  import { createEventDispatcher, onDestroy, onMount } from 'svelte';
  import { tick } from 'svelte';
  import { slide, fade } from 'svelte/transition';
  import { renderMarkdownSafe } from '../../lib/safeMarkdown';
  import {
    activeTask,
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
  import {
    filterSkillsByMode,
    getSkillById,
    getSkillLabel,
    loadSkillCatalog,
    matchSkills,
    skillCatalog,
    type AiSkillSummary,
  } from '../../lib/aiSkillService';
  import AiSkillSlashMenu from './AiSkillSlashMenu.svelte';
  import AgentToolCallStep from './AgentToolCallStep.svelte';
  import AiModeSwitcher from './AiModeSwitcher.svelte';
  import AiSandboxSwitcher from './AiSandboxSwitcher.svelte';
  import CommandConfirmModal from './CommandConfirmModal.svelte';
  import {
    buildSkillOptions,
    findSkillCommand,
    removeSkillCommand,
    type SkillCommandMatch,
    type SkillOption,
  } from '../../lib/aiSkillInput';

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
  let instructionEl: HTMLTextAreaElement;
  let selectedSkillId = '';
  let agentSkills = filterSkillsByMode([], 'agent');
  let activeSkillCommand: SkillCommandMatch | null = null;
  let skillOptions: SkillOption[] = [];
  let highlightedSkillIndex = 0;
  let dismissedCommandKey = '';
  let autoMatchedSkillId = '';
  let autoMatchedReason = '';
  let autoMatchedSkill: AiSkillSummary | null = null;
  let autoMatchDismissedInput = '';
  let matchTimer: ReturnType<typeof setTimeout> | null = null;

  // 已提交指令的历史（最新在末尾），用于方向键上/下翻阅，方便二次输入。
  let inputHistory: string[] = [];
  // 当前翻阅位置：null 表示未处于翻阅态；否则为 inputHistory 的索引。
  let historyIndex: number | null = null;
  // 进入翻阅前的草稿，翻到最新之后回到它。
  let historyDraft = '';

  onMount(() => {
    void loadSkillCatalog();
    void loadTaskHistory(sessionId);
  });

  onDestroy(() => {
    if (matchTimer) {
      clearTimeout(matchTimer);
    }
    cleanup();
  });

  $: if (sessionId) {
    void loadTaskHistory(sessionId);
  }
  $: agentSkills = filterSkillsByMode($skillCatalog, 'agent');

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
      await startAgent(sessionId, text, $sandboxMode, selectedSkillId || autoMatchedSkillId || null);
      // 记录到输入历史（去掉与上一条重复的连续项），供方向键翻阅。
      if (inputHistory[inputHistory.length - 1] !== text) {
        inputHistory = [...inputHistory, text];
      }
      historyIndex = null;
      historyDraft = '';
      instruction = '';
      selectedSkillId = '';
      autoMatchedSkillId = '';
      autoMatchedReason = '';
      autoMatchDismissedInput = '';
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
    if (showSkillMenu) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        if (skillOptions.length > 0) {
          highlightedSkillIndex = (highlightedSkillIndex + 1) % skillOptions.length;
        }
        return;
      }

      if (e.key === 'ArrowUp') {
        e.preventDefault();
        if (skillOptions.length > 0) {
          highlightedSkillIndex =
            (highlightedSkillIndex - 1 + skillOptions.length) % skillOptions.length;
        }
        return;
      }

      if ((e.key === 'Enter' || e.key === 'Tab') && skillOptions.length > 0) {
        e.preventDefault();
        applySkillSelection(skillOptions[highlightedSkillIndex]?.id ?? null);
        return;
      }

      if (e.key === 'Escape') {
        e.preventDefault();
        dismissedCommandKey = activeCommandKey;
        return;
      }
    }

    // 方向键翻阅历史指令：仅在光标位于文本起点/终点时触发，避免打断多行编辑。
    if (e.key === 'ArrowUp' && !e.shiftKey && caretAtStart()) {
      if (inputHistory.length > 0) {
        e.preventDefault();
        recallHistory(-1);
        return;
      }
    }

    if (e.key === 'ArrowDown' && !e.shiftKey && historyIndex !== null && caretAtEnd()) {
      e.preventDefault();
      recallHistory(1);
      return;
    }

    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleStart();
    }
  }

  function caretAtStart(): boolean {
    if (!instructionEl) return true;
    return instructionEl.selectionStart === 0 && instructionEl.selectionEnd === 0;
  }

  function caretAtEnd(): boolean {
    if (!instructionEl) return true;
    return (
      instructionEl.selectionStart === instruction.length &&
      instructionEl.selectionEnd === instruction.length
    );
  }

  // step=-1 往更早翻，step=+1 往更新翻；翻过最新一条后回到进入翻阅前的草稿。
  function recallHistory(step: number) {
    if (inputHistory.length === 0) return;

    let nextIndex: number;
    if (historyIndex === null) {
      if (step > 0) return;
      historyDraft = instruction;
      nextIndex = inputHistory.length - 1;
    } else {
      nextIndex = historyIndex + step;
    }

    if (nextIndex >= inputHistory.length) {
      // 翻过最新一条：退出翻阅态，恢复草稿。
      historyIndex = null;
      instruction = historyDraft;
    } else if (nextIndex < 0) {
      // 已在最早一条，保持不动。
      return;
    } else {
      historyIndex = nextIndex;
      instruction = inputHistory[nextIndex];
    }

    requestAnimationFrame(() => {
      instructionEl?.focus();
      const pos = instruction.length;
      instructionEl?.setSelectionRange(pos, pos);
      syncSkillCommand();
    });
  }

  function commandKey(match: SkillCommandMatch | null): string {
    return match ? `${match.start}:${match.end}:${match.query}` : '';
  }

  function syncSkillCommand(resetHighlight = false) {
    const nextCommand = findSkillCommand(
      instruction,
      instructionEl?.selectionStart ?? instruction.length,
    );
    const previousQuery = activeSkillCommand?.query ?? '';
    const nextKey = commandKey(nextCommand);

    activeSkillCommand = nextCommand;
    skillOptions = nextCommand ? buildSkillOptions(agentSkills, nextCommand.query) : [];

    if (!nextCommand) {
      dismissedCommandKey = '';
      highlightedSkillIndex = 0;
      return;
    }

    if (dismissedCommandKey && dismissedCommandKey !== nextKey) {
      dismissedCommandKey = '';
    }

    if (resetHighlight || previousQuery !== nextCommand.query) {
      highlightedSkillIndex = 0;
      return;
    }

    highlightedSkillIndex = Math.min(
      highlightedSkillIndex,
      Math.max(skillOptions.length - 1, 0),
    );
  }

  function handleInstructionInput() {
    // 一旦用户手动编辑，脱离历史翻阅态。
    historyIndex = null;
    syncSkillCommand(true);
    queueSkillMatch();
  }

  function handleInstructionCaretChange() {
    syncSkillCommand();
  }

  function applySkillSelection(skillId: string | null) {
    selectedSkillId = skillId ?? '';
    autoMatchedSkillId = '';
    autoMatchedReason = '';
    autoMatchDismissedInput = '';

    if (activeSkillCommand) {
      const nextInstruction = removeSkillCommand(instruction, activeSkillCommand);
      instruction = nextInstruction.text;
      dismissedCommandKey = '';

      requestAnimationFrame(() => {
        instructionEl?.focus();
        instructionEl?.setSelectionRange(nextInstruction.cursor, nextInstruction.cursor);
        syncSkillCommand();
      });
      return;
    }

    instructionEl?.focus();
  }

  function clearSkillSelection() {
    selectedSkillId = '';
    queueSkillMatch();
    instructionEl?.focus();
  }

  function dismissAutoMatch() {
    autoMatchedSkillId = '';
    autoMatchedReason = '';
    autoMatchDismissedInput = instruction.trim();
  }

  function queueSkillMatch() {
    if (matchTimer) {
      clearTimeout(matchTimer);
    }
    matchTimer = setTimeout(() => {
      void refreshAutoMatch();
    }, 180);
  }

  async function refreshAutoMatch() {
    const input = instruction.trim();
    if (
      !input ||
      !sessionId ||
      selectedSkillId ||
      isStarting ||
      activeSkillCommand ||
      autoMatchDismissedInput === input
    ) {
      if (!input) {
        autoMatchedSkillId = '';
        autoMatchedReason = '';
      }
      return;
    }

    const result = await matchSkills(input, 'agent');
    if (instruction.trim() !== input || selectedSkillId) {
      return;
    }
    autoMatchedSkillId = result.auto_applied ? result.matched_skill_id ?? '' : '';
    autoMatchedReason = result.reason ?? '';
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
    cancelled: 'bg-app-surface border-app-border/60',
  };

  const HISTORY_CAUSE_STYLES: Record<string, string> = {
    rejected: 'bg-red-500/10 text-red-400 border-red-500/20',
    timed_out: 'bg-yellow-500/10 text-yellow-300 border-yellow-500/20',
  };

  function renderMarkdown(content: string): string {
    return renderMarkdownSafe(content);
  }

  $: finalResultStep = [...($activeTask?.steps ?? [])]
    .reverse()
    .find((step) => step.kind === 'result' && (step.output?.trim() || step.title?.trim()));
  $: selectedSkill = getSkillById(selectedSkillId || null);
  $: autoMatchedSkill = !selectedSkill ? getSkillById(autoMatchedSkillId || null) : null;
  $: activeCommandKey = commandKey(activeSkillCommand);
  $: activeTaskSkillLabel = getSkillLabel($activeTask?.skill_id);
  $: summaryText = $activeTask?.summary?.trim() || $activeTask?.final_result?.summary?.trim() || finalResultStep?.output?.trim() || '';
  $: summaryHtml = summaryText ? renderMarkdown(summaryText) : '';
  $: doneCardStyle = $activeTask ? DONE_CARD_STYLES[$activeTask.status] ?? 'bg-app-surface border-app-border/50' : 'bg-app-surface border-app-border/50';
  $: visibleSteps = ($activeTask?.steps ?? []).filter((step) => showThinking || step.kind !== 'planning');
  $: hasStructuredOutcome = !!summaryText || !!$activeTask?.error_message;
  // 若已有 result step 渲染了摘要，结果卡不再重复展示同一段内容
  $: summaryShownInSteps = !!finalResultStep && (finalResultStep.output?.trim() ?? '') === summaryText;
  $: showSkillMenu =
    !!activeSkillCommand &&
    activeCommandKey !== dismissedCommandKey &&
    !isStarting &&
    !!sessionId;
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
              class="w-full text-left text-xs px-3 py-2 rounded-lg bg-app-surface border border-app-border/50 text-app-text-secondary hover:text-app-text hover:border-primary-500/30 transition-colors"
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
      <div class="px-3 py-3 mx-3 mb-2.5 bg-primary-600/[0.07] border border-primary-500/10 rounded-xl">
        <div class="flex items-start justify-between gap-3">
          <div class="min-w-0">
            <div class="flex items-center gap-2">
              <p class="text-[10px] font-semibold uppercase tracking-wider text-primary-400/70">任务</p>
              {#if activeTaskSkillLabel}
                <span class="shrink-0 rounded-full border border-primary-500/15 bg-primary-600/10 px-2 py-0.5 text-[11px] text-primary-400">
                  {activeTaskSkillLabel}
                </span>
              {/if}
            </div>
            <p class="mt-1.5 text-sm font-medium text-app-text leading-relaxed break-words">{$activeTask.instruction}</p>
          </div>
          <span
            class="shrink-0 inline-flex items-center gap-1.5 rounded-full border border-app-border/50 bg-app-bg px-2.5 py-1 text-[11px] font-medium {STATUS_COLORS[$activeTask.status]}"
          >
            {#if isRunning}
              <span class="w-1.5 h-1.5 rounded-full bg-current animate-pulse"></span>
            {/if}
            {STATUS_LABELS[$activeTask.status]}
          </span>
        </div>
      </div>

      <div class="px-1">
        {#each visibleSteps as step, i (step.id)}
          <AgentToolCallStep {step} isLast={i === visibleSteps.length - 1 && !isRunning && !isDone} />
        {/each}

        {#if isRunning && !isWaiting}
          <!-- 运行中的活动节点，与步骤时间线对齐 -->
          <div class="flex gap-3 px-3">
            <div class="flex-shrink-0 flex flex-col items-center">
              <div class="w-5 h-5 mt-1.5 flex items-center justify-center rounded-full border border-blue-400/40 bg-blue-400/10">
                <div class="w-2.5 h-2.5 rounded-full border-2 border-blue-400 border-t-transparent animate-spin"></div>
              </div>
            </div>
            <div class="flex-1 min-w-0 pt-1.5 pb-3">
              <p class="text-xs text-blue-400">
                {#if $activeTask?.status === 'retrying'}
                  AI 正在重试...
                {:else if $activeTask?.status === 'cancelling'}
                  正在停止当前任务...
                {:else if $activeTask?.status === 'planning'}
                  AI 正在规划...
                {:else}
                  AI 正在执行...
                {/if}
              </p>
            </div>
          </div>
        {/if}

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
                {#if summaryShownInSteps}
                  <span class="text-xs text-app-text-secondary">见上方任务结果</span>
                {:else if $activeTask.status === 'failed'}
                  <span class="text-xs text-red-400">未生成最终结果</span>
                {:else if $activeTask.status === 'cancelled'}
                  <span class="text-xs text-app-text-secondary">任务已被用户终止</span>
                {/if}
              </div>

              {#if summaryText && !summaryShownInSteps}
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
      </div>

      {#if $activeTask.error_message && !isDone}
        <div
          class="mx-3 mt-2 p-3 bg-red-500/10 border border-red-500/20 rounded-lg text-red-400 text-xs"
          transition:fade={{ duration: 200 }}
        >
          {$activeTask.error_message}
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
                : 'bg-app-bg border-app-border/50 hover:border-app-border/80'}"
            on:click={() => handleOpenHistory(item.id)}
            type="button"
          >
            <div class="flex items-center justify-between gap-3">
              <p class="text-xs text-app-text truncate">{item.instruction}</p>
              <div class="flex items-center gap-1.5 shrink-0">
                {#if item.skill_id}
                  <span class="text-[11px] px-1.5 py-0.5 rounded border border-primary-500/20 bg-primary-600/10 text-primary-400">
                    {getSkillLabel(item.skill_id)}
                  </span>
                {/if}
                {#if item.history_cause && item.history_cause_label}
                  <span class="text-[11px] px-1.5 py-0.5 rounded border {HISTORY_CAUSE_STYLES[item.history_cause]}">
                    {item.history_cause_label}
                  </span>
                {/if}
                <span class="text-[11px] {STATUS_COLORS[item.status]}">{STATUS_LABELS[item.status]}</span>
              </div>
            </div>
            {#if item.history_preview || item.summary || item.error_message}
              <p class="text-[11px] text-app-text-secondary mt-1 truncate">
                {item.history_preview || item.summary || item.error_message}
              </p>
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
        {#if !selectedSkill && autoMatchedSkill}
          <div class="mb-2 rounded-2xl border border-blue-500/20 bg-blue-500/10 px-3 py-2">
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <div class="flex items-center gap-2">
                  <span class="rounded-full border border-blue-500/25 bg-blue-500/10 px-2 py-0.5 text-[10px] text-blue-400">
                    自动匹配
                  </span>
                  <span class="text-xs font-medium text-app-text">{autoMatchedSkill.name}</span>
                </div>
                <p class="mt-1 text-[11px] leading-relaxed text-app-text-secondary">
                  {autoMatchedReason || autoMatchedSkill.description}
                </p>
              </div>
              <button
                class="shrink-0 rounded-full border border-app-border bg-app-bg px-2 py-1 text-[11px] text-app-text-secondary transition-colors hover:border-app-border/80 hover:text-app-text"
                type="button"
                on:click={dismissAutoMatch}
              >
                忽略
              </button>
            </div>
          </div>
        {/if}

        <AiSkillSlashMenu
          {selectedSkill}
          visible={showSkillMenu}
          query={activeSkillCommand?.query ?? ''}
          options={skillOptions}
          highlightedIndex={highlightedSkillIndex}
          on:select={(e) => applySkillSelection(e.detail)}
          on:clear={clearSkillSelection}
          on:highlight={(e) => (highlightedSkillIndex = e.detail)}
        />

        <textarea
          bind:this={instructionEl}
          bind:value={instruction}
          on:input={handleInstructionInput}
          on:keydown={handleKeydown}
          on:click={handleInstructionCaretChange}
          on:keyup={handleInstructionCaretChange}
          on:focus={handleInstructionCaretChange}
          placeholder="描述你想让 Agent 做什么，输入“/”选择 Skill..."
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
          <div class="mt-0.5 flex items-center gap-2 min-w-0">
            <p class="text-xs text-app-text-secondary truncate">
              {$activeTask?.instruction}
            </p>
            {#if activeTaskSkillLabel}
              <span class="shrink-0 rounded-full border border-primary-500/20 bg-primary-600/10 px-2 py-0.5 text-[11px] text-primary-400">
                {activeTaskSkillLabel}
              </span>
            {/if}
          </div>
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
