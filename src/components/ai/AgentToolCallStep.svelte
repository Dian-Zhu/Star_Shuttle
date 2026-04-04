<script lang="ts">
  import { marked } from 'marked';
  import type { AgentStep } from '../../lib/aiAgentService';

  export let step: AgentStep;
  let expanded = false;

  function renderMarkdown(content: string): string {
    try {
      return marked.parse(content, { async: false, gfm: true, breaks: true }) as string;
    } catch {
      return content;
    }
  }

  $: isResultStep = step.kind === 'result';
  $: isExpanded = isResultStep && hasOutput ? true : expanded;
  $: renderedOutput = hasOutput ? renderMarkdown(step.output ?? '') : '';

  const KIND_LABELS: Record<string, string> = {
    thinking: '思考中',
    execute_command: '执行命令',
    read_file: '读取文件',
    list_directory: '列出目录',
    get_system_info: '获取系统信息',
    awaiting_confirm: '等待确认',
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

  $: isRunning = step.status === 'running';
  $: hasOutput = !!step.output;
  $: riskColor = step.risk_level ? RISK_COLORS[step.risk_level] : '';
  $: riskLabel = step.risk_level ? RISK_LABELS[step.risk_level] : '';
  $: containerClass = isResultStep
    ? 'bg-primary-600/10 border border-primary-500/20 rounded-lg'
    : 'hover:bg-app-surface/50 rounded-md';
</script>

<div class="flex gap-2.5 py-1.5 px-3 group transition-colors {containerClass}">
  <!-- Status indicator -->
  <div class="flex-shrink-0 flex flex-col items-center pt-0.5">
    <div class="w-4 h-4 flex items-center justify-center">
      {#if isRunning}
        <div class="w-3 h-3 rounded-full border-2 border-blue-400 border-t-transparent animate-spin"></div>
      {:else if step.status === 'completed' || step.status === 'confirmed'}
        <svg class="w-4 h-4 text-green-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" />
        </svg>
      {:else if step.status === 'failed' || step.status === 'rejected'}
        <svg class="w-4 h-4 text-red-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M6 18L18 6M6 6l12 12" />
        </svg>
      {:else if step.status === 'waiting_confirm'}
        <svg class="w-4 h-4 text-yellow-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
      {:else}
        <div class="w-2 h-2 rounded-full bg-app-border"></div>
      {/if}
    </div>
    <!-- Connector line (not for last item) -->
    <div class="w-px flex-1 bg-app-border mt-1 min-h-[8px]"></div>
  </div>

  <!-- Content -->
  <div class="flex-1 min-w-0 pb-1.5">
    <div class="flex items-center gap-2 flex-wrap">
      <!-- Kind label -->
      <span class="text-xs font-medium {isResultStep ? 'text-primary-400' : 'text-app-text-secondary'}">
        {KIND_LABELS[step.kind] ?? step.kind}
      </span>

      <!-- Risk badge -->
      {#if step.risk_level}
        <span class="text-xs px-1.5 py-0.5 rounded border {riskColor}">
          {riskLabel}
        </span>
      {/if}

      <!-- Status text (for non-default states) -->
      {#if step.status === 'rejected'}
        <span class="text-xs text-red-400">已拒绝</span>
      {:else if step.status === 'failed'}
        <span class="text-xs text-red-400">失败</span>
      {/if}
    </div>

    <!-- Description -->
    <p class="text-sm {isResultStep ? 'text-app-text font-medium' : 'text-app-text'} mt-0.5 leading-snug">{step.description}</p>

    {#if isResultStep && hasOutput}
      <div
        class="mt-1.5 bg-app-bg/70 border border-app-border rounded-lg p-2.5 max-h-48 overflow-y-auto
               prose prose-sm dark:prose-invert max-w-none text-app-text
               prose-code:before:content-none prose-code:after:content-none
               prose-pre:bg-app-bg prose-pre:border prose-pre:border-app-border prose-pre:rounded-lg prose-pre:text-xs
               prose-p:my-1 prose-ul:my-1 prose-ol:my-1 prose-li:my-0 break-words"
      >
        {@html renderedOutput}
      </div>
    {:else}
      {#if hasOutput && !step.command}
        <div
          class="mt-1.5 bg-app-bg border border-app-border rounded-lg p-2 max-h-48 overflow-y-auto
                 prose prose-sm dark:prose-invert max-w-none text-app-text-secondary
                 prose-code:before:content-none prose-code:after:content-none
                 prose-pre:bg-app-bg prose-pre:border prose-pre:border-app-border prose-pre:rounded-lg prose-pre:text-xs
                 prose-p:my-1 prose-ul:my-1 prose-ol:my-1 prose-li:my-0 break-words"
        >
          {@html renderedOutput}
        </div>
      {/if}

      <!-- Command display -->
      {#if step.command}
        <div class="mt-1 flex items-center gap-1.5">
          <code class="text-xs font-mono bg-app-bg border border-app-border rounded px-2 py-0.5 text-app-text-secondary flex-1 truncate">
            {step.command}
          </code>
          {#if hasOutput}
            <button
              class="text-xs text-app-text-secondary hover:text-app-text transition-colors"
              on:click={() => (expanded = !expanded)}
            >
              {isExpanded ? '收起' : '展开'}
            </button>
          {/if}
        </div>
      {/if}

      <!-- Output (collapsible) -->
      {#if hasOutput && isExpanded}
        <div class="mt-1.5 bg-app-bg border border-app-border rounded-lg p-2 max-h-48 overflow-y-auto">
          <pre class="text-xs font-mono text-app-text-secondary whitespace-pre-wrap break-all leading-relaxed">{step.output}</pre>
        </div>
      {/if}
    {/if}
  </div>
</div>
