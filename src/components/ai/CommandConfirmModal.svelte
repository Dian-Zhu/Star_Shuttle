<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { fade, scale } from 'svelte/transition';
  import type { PendingConfirm } from '../../lib/aiAgentService';

  export let confirm: PendingConfirm;

  const dispatch = createEventDispatcher<{
    confirm: boolean;
  }>();

  const RISK_CONFIG: Record<string, { label: string; color: string; bg: string; icon: string }> = {
    critical: {
      label: '高危操作',
      color: 'text-red-400',
      bg: 'bg-red-500/10 border-red-500/30',
      icon: '🔴',
    },
    high: {
      label: '敏感操作',
      color: 'text-orange-400',
      bg: 'bg-orange-500/10 border-orange-500/30',
      icon: '🟠',
    },
    medium: {
      label: '需要确认',
      color: 'text-yellow-400',
      bg: 'bg-yellow-500/10 border-yellow-500/30',
      icon: '🟡',
    },
  };

  $: riskCfg = RISK_CONFIG[confirm.risk_level] ?? RISK_CONFIG['medium'];

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') dispatch('confirm', false);
    if (e.key === 'Enter' && e.ctrlKey) dispatch('confirm', true);
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- Backdrop -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/60 backdrop-blur-sm"
  transition:fade={{ duration: 150 }}
  on:click|self={() => dispatch('confirm', false)}
  on:keydown={(e) => e.key === 'Escape' && dispatch('confirm', false)}
  role="dialog"
  aria-modal="true"
  aria-labelledby="confirm-title"
  tabindex="-1"
>
  <!-- Modal -->
  <div
    class="w-full max-w-md bg-app-surface border border-app-border rounded-xl shadow-2xl overflow-hidden"
    transition:scale={{ duration: 150, start: 0.95 }}
  >
    <!-- Header -->
    <div class="px-5 py-4 border-b border-app-border flex items-center gap-3">
      <div class="text-2xl" role="img" aria-label={riskCfg.label}>{riskCfg.icon}</div>
      <div>
        <h2 id="confirm-title" class="font-semibold text-app-text">AI Agent 命令确认</h2>
        <p class="{riskCfg.color} text-xs font-medium">{riskCfg.label}</p>
      </div>
    </div>

    <!-- Body -->
    <div class="px-5 py-4 space-y-4">
      <!-- Risk banner -->
      <div class="p-3 rounded-lg border {riskCfg.bg}">
        <p class="{riskCfg.color} text-sm font-medium">{confirm.reason}</p>
      </div>

      <!-- Command preview -->
      <div>
        <p class="text-xs text-app-text-secondary mb-1.5 font-medium">待执行命令：</p>
        <div class="bg-app-bg border border-app-border rounded-lg p-3">
          <code class="text-sm font-mono text-app-text break-all">{confirm.command}</code>
        </div>
      </div>

      <!-- Warning text -->
      <p class="text-xs text-app-text-secondary leading-relaxed">
        {#if confirm.risk_level === 'critical'}
          ⚠️ 此命令可能造成<strong class="text-red-400">不可逆的损坏</strong>，请仔细确认后再决定。
        {:else if confirm.risk_level === 'high'}
          此命令会修改系统状态，请确认这是您期望的操作。
        {:else}
          AI Agent 请求执行此命令，请确认是否允许。
        {/if}
      </p>

      <p class="text-xs text-app-text-secondary opacity-60">
        快捷键：Ctrl+Enter 确认 · Esc 拒绝
      </p>
    </div>

    <!-- Footer -->
    <div class="px-5 py-3 bg-app-bg border-t border-app-border flex gap-3 justify-end">
      <button
        class="px-4 py-2 rounded-lg bg-app-surface hover:bg-app-border text-app-text-secondary hover:text-app-text text-sm font-medium transition-colors"
        on:click={() => dispatch('confirm', false)}
      >
        拒绝
      </button>
      <button
        class="px-4 py-2 rounded-lg text-white text-sm font-medium transition-colors
          {confirm.risk_level === 'critical'
            ? 'bg-red-600 hover:bg-red-500'
            : confirm.risk_level === 'high'
              ? 'bg-orange-600 hover:bg-orange-500'
              : 'bg-primary-600 hover:bg-primary-500'}"
        on:click={() => dispatch('confirm', true)}
      >
        {confirm.risk_level === 'critical' ? '确认执行（高危）' : '确认执行'}
      </button>
    </div>
  </div>
</div>
