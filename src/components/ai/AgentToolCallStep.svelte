<script lang="ts">
  import { slide } from 'svelte/transition';
  import { renderMarkdownSafe } from '../../lib/safeMarkdown';
  import type { AgentStep } from '../../lib/aiAgentService';

  export let step: AgentStep;
  export let isLast = false;

  // 净化后再渲染：step.output 来自远程主机命令输出，属不可信内容，
  // 必须经 DOMPurify 清洗才能 {@html} 注入，防止 XSS。
  function renderMarkdown(content: string): string {
    return renderMarkdownSafe(content);
  }

  const KIND_LABELS: Record<string, string> = {
    planning: '规划',
    tool_execution: '工具执行',
    confirmation: '确认',
    result: '任务结果',
  };

  const RISK_COLORS: Record<string, string> = {
    critical: 'bg-red-500/15 text-red-400 border-red-500/30',
    high:     'bg-orange-500/15 text-orange-400 border-orange-500/30',
    medium:   'bg-yellow-500/15 text-yellow-400 border-yellow-500/30',
  };

  const RISK_LABELS: Record<string, string> = {
    critical: '高危',
    high:     '敏感',
    medium:   '注意',
  };

  $: isResultStep = step.kind === 'result';
  $: isRunning = step.status === 'running';
  $: isWaitingConfirm = step.kind === 'confirmation' && step.status === 'running';
  $: isFailed = step.status === 'failed' || step.status === 'rejected';
  $: hasOutput = !!step.output;
  $: riskColor = step.risk_level ? RISK_COLORS[step.risk_level] : '';
  $: riskLabel = step.risk_level ? RISK_LABELS[step.risk_level] : '';

  // 输出折叠：结果步骤恒展开；失败步骤默认展开（方便查看错误）；其余默认折叠。
  let userToggled: boolean | null = null;
  $: defaultExpanded = isResultStep || isFailed;
  $: isExpanded = userToggled ?? defaultExpanded;
  $: renderedOutput = isResultStep && hasOutput ? renderMarkdown(step.output ?? '') : '';

  // 节点圆点配色
  $: nodeClass = isWaitingConfirm
    ? 'border-yellow-400/40 bg-yellow-400/10'
    : isRunning
      ? 'border-blue-400/40 bg-blue-400/10'
      : step.status === 'completed'
        ? 'border-green-400/40 bg-green-400/10'
        : isFailed
          ? 'border-red-400/40 bg-red-400/10'
          : 'border-app-border bg-app-surface';
</script>

<div class="flex gap-3 px-3">
  <!-- 时间线 gutter -->
  <div class="flex-shrink-0 flex flex-col items-center">
    <div class="w-5 h-5 mt-1.5 flex items-center justify-center rounded-full border {nodeClass}">
      {#if isWaitingConfirm}
        <svg class="w-3 h-3 text-yellow-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
      {:else if isRunning}
        <div class="w-2.5 h-2.5 rounded-full border-2 border-blue-400 border-t-transparent animate-spin"></div>
      {:else if step.status === 'completed'}
        <svg class="w-3 h-3 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
        </svg>
      {:else if isFailed}
        <svg class="w-3 h-3 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M6 18L18 6M6 6l12 12" />
        </svg>
      {:else}
        <div class="w-1.5 h-1.5 rounded-full bg-app-text-secondary/40"></div>
      {/if}
    </div>
    {#if !isLast}
      <div class="w-px flex-1 bg-app-border mt-1 min-h-[12px]"></div>
    {/if}
  </div>

  <!-- 内容 -->
  <div class="flex-1 min-w-0 pb-3 {isResultStep ? '' : 'pt-0.5'}">
    <div class="flex items-center gap-2 flex-wrap">
      <span class="text-[10px] font-semibold uppercase tracking-wider {isResultStep ? 'text-primary-400' : 'text-app-text-secondary/70'}">
        {KIND_LABELS[step.kind] ?? step.kind}
      </span>

      {#if step.risk_level}
        <span class="text-[11px] px-1.5 py-0.5 rounded border {riskColor}">
          {riskLabel}
        </span>
      {/if}

      {#if step.status === 'rejected'}
        <span class="text-[11px] text-red-400">已拒绝</span>
      {:else if step.status === 'failed'}
        <span class="text-[11px] text-red-400">失败</span>
      {:else if step.status === 'skipped'}
        <span class="text-[11px] text-app-text-secondary">已跳过</span>
      {/if}
    </div>

    <!-- 标题 -->
    <p class="{isResultStep ? 'text-sm text-app-text font-semibold' : 'text-[13px] text-app-text'} mt-1 leading-snug break-words">
      {step.title}
    </p>

    <!-- 命令块 -->
    {#if step.command}
      <div class="mt-1.5 rounded-lg border border-app-border/50 bg-app-bg overflow-hidden">
        <div class="flex items-center gap-2 px-2.5 py-1.5">
          <span class="select-none text-app-text-secondary/60 font-mono text-xs">$</span>
          <pre class="flex-1 overflow-x-auto text-xs font-mono text-app-text leading-relaxed">{step.command}</pre>
          {#if hasOutput}
            <button
              class="shrink-0 text-[11px] text-app-text-secondary hover:text-app-text transition-colors"
              type="button"
              on:click={() => (userToggled = !isExpanded)}
            >
              {isExpanded ? '收起' : '展开'}
            </button>
          {/if}
        </div>
      </div>
    {/if}

    <!-- 结果步骤：markdown 输出，恒展开 -->
    {#if isResultStep && hasOutput}
      <div
        class="mt-1.5 bg-app-bg/70 border border-app-border/50 rounded-lg p-2.5 max-h-60 overflow-y-auto
               prose prose-sm dark:prose-invert max-w-none text-app-text
               prose-code:before:content-none prose-code:after:content-none
               prose-pre:bg-app-bg prose-pre:border prose-pre:border-app-border prose-pre:rounded-lg prose-pre:text-xs
               prose-p:my-1 prose-ul:my-1 prose-ol:my-1 prose-li:my-0 break-words"
      >
        {@html renderedOutput}
      </div>
    {:else if hasOutput && !step.command}
      <!-- 无命令的纯文本输出（如无法结构化的中间结果） -->
      <pre
        class="mt-1.5 bg-app-bg border border-app-border/50 rounded-lg p-2.5 max-h-60 overflow-auto
               text-xs font-mono text-app-text-secondary whitespace-pre-wrap break-all leading-relaxed"
      >{step.output}</pre>
    {:else if hasOutput && isExpanded}
      <!-- 命令输出（可折叠）：纯文本，不走 markdown 注入 -->
      <pre
        class="mt-1.5 bg-app-bg border border-app-border/50 rounded-lg p-2.5 max-h-60 overflow-auto
               text-xs font-mono {isFailed ? 'text-red-300' : 'text-app-text-secondary'}
               whitespace-pre-wrap break-all leading-relaxed"
        transition:slide={{ duration: 150 }}
      >{step.output}</pre>
    {/if}
  </div>
</div>
