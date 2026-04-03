<script lang="ts">
  import type { AgentStep } from '../../lib/aiAgentService';

  export let step: AgentStep;
  let expanded = false;

  const KIND_LABELS: Record<string, string> = {
    thinking: '思考中',
    execute_command: '执行命令',
    read_file: '读取文件',
    list_directory: '列出目录',
    get_system_info: '获取系统信息',
    awaiting_confirm: '等待确认',
    result: '任务结果',
  };

  const STATUS_COLORS: Record<string, string> = {
    pending:          'text-app-text-secondary',
    running:          'text-blue-400',
    waiting_confirm:  'text-yellow-400',
    confirmed:        'text-green-400',
    rejected:         'text-red-400',
    completed:        'text-green-400',
    failed:           'text-red-400',
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

  $: statusColor = STATUS_COLORS[step.status] ?? 'text-app-text-secondary';
  $: isRunning = step.status === 'running';
  $: hasOutput = !!step.output;
  $: riskColor = step.risk_level ? RISK_COLORS[step.risk_level] : '';
  $: riskLabel = step.risk_level ? RISK_LABELS[step.risk_level] : '';
</script>

<div class="flex gap-2.5 py-1.5 px-3 group hover:bg-app-surface/50 transition-colors rounded-md">
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
      <span class="text-xs font-medium text-app-text-secondary">
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
    <p class="text-sm text-app-text mt-0.5 leading-snug">{step.description}</p>

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
            {expanded ? '收起' : '展开'}
          </button>
        {/if}
      </div>
    {/if}

    <!-- Output (collapsible) -->
    {#if hasOutput && expanded}
      <div class="mt-1.5 bg-app-bg border border-app-border rounded-lg p-2 max-h-48 overflow-y-auto">
        <pre class="text-xs font-mono text-app-text-secondary whitespace-pre-wrap break-all leading-relaxed">{step.output}</pre>
      </div>
    {/if}
  </div>
</div>
