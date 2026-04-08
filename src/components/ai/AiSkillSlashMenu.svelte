<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { AiSkillSummary } from '../../lib/aiSkillService';
  import type { SkillOption } from '../../lib/aiSkillInput';

  export let selectedSkill: AiSkillSummary | null = null;
  export let visible = false;
  export let query = '';
  export let options: SkillOption[] = [];
  export let highlightedIndex = 0;

  const dispatch = createEventDispatcher<{
    select: string | null;
    clear: void;
    highlight: number;
  }>();

  function handleSelect(skillId: string | null) {
    dispatch('select', skillId);
  }

  function clearSelection() {
    dispatch('clear');
  }

  function setHighlight(index: number) {
    dispatch('highlight', index);
  }
</script>

{#if selectedSkill}
  <div class="mb-2 rounded-2xl border border-primary-500/20 bg-primary-600/10 px-3 py-2">
    <div class="flex items-start justify-between gap-3">
      <div class="min-w-0">
        <div class="inline-flex items-center rounded-full border border-primary-500/25 bg-primary-600/15 px-2.5 py-1 text-[11px] font-medium text-primary-400">
          {selectedSkill.name}
        </div>
        <p class="mt-1 text-[11px] leading-relaxed text-app-text-secondary">
          {selectedSkill.description}
          {#if selectedSkill.recommended_sandbox}
            · 推荐沙箱：{selectedSkill.recommended_sandbox === 'full' ? '完整' : '标准'}
          {/if}
        </p>
      </div>
      <button
        class="shrink-0 rounded-full border border-app-border bg-app-bg px-2 py-1 text-[11px] text-app-text-secondary transition-colors hover:border-app-border/80 hover:text-app-text"
        type="button"
        on:click={clearSelection}
      >
        移除
      </button>
    </div>
  </div>
{/if}

{#if visible}
  <div class="mb-2 overflow-hidden rounded-2xl border border-app-border bg-app-bg shadow-[0_14px_40px_rgba(0,0,0,0.18)]">
    <div class="flex items-center justify-between gap-3 border-b border-app-border px-3 py-2">
      <p class="text-[11px] font-medium text-app-text-secondary">Skill</p>
      <p class="text-[11px] text-app-text-secondary/80">/{query || '...'}</p>
    </div>

    {#if options.length}
      <div class="max-h-60 overflow-y-auto p-1">
        {#each options as option, index (`${option.id ?? 'none'}-${index}`)}
          <button
            class="flex w-full items-start justify-between gap-3 rounded-xl px-3 py-2 text-left transition-colors
              {index === highlightedIndex
                ? 'bg-primary-600/15 text-app-text'
                : 'text-app-text-secondary hover:bg-app-surface hover:text-app-text'}"
            type="button"
            on:mousedown|preventDefault={() => handleSelect(option.id)}
            on:mouseenter={() => setHighlight(index)}
          >
            <div class="min-w-0">
              <p class="text-xs font-medium">{option.name}</p>
              <p class="mt-1 text-[11px] leading-relaxed text-app-text-secondary">{option.description}</p>
            </div>
            {#if option.recommendedSandbox}
              <span class="shrink-0 rounded-full border border-app-border bg-app-surface px-2 py-0.5 text-[10px] text-app-text-secondary">
                {option.recommendedSandbox === 'full' ? '完整沙箱' : '标准沙箱'}
              </span>
            {/if}
          </button>
        {/each}
      </div>
    {:else}
      <div class="px-3 py-3 text-[11px] text-app-text-secondary">
        没有匹配的 Skill，继续输入可缩小范围，或按 `Esc` 关闭。
      </div>
    {/if}
  </div>
{/if}
