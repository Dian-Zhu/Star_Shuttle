<script lang="ts">
  import { createEventDispatcher, onDestroy } from 'svelte';
  import {
    getSkillById,
    matchSkills,
    type AiSkillSummary,
    type AiSkillMatchResult,
  } from '../../lib/aiSkillService';
  import AiSkillSlashMenu from './AiSkillSlashMenu.svelte';
  import AiModeSwitcher from './AiModeSwitcher.svelte';
  import {
    buildSkillOptions,
    findSkillCommand,
    removeSkillCommand,
    type SkillCommandMatch,
    type SkillOption,
  } from '../../lib/aiSkillInput';

  export let disabled = false;
  export let isSending = false;
  export let includeContext = false;
  export let hasActiveSession = false;
  export let activeMode: 'chat' | 'agent' = 'chat';
  export let skills: AiSkillSummary[] = [];

  let value = '';
  let selectedSkillId = '';
  let textareaEl: HTMLTextAreaElement;
  let selectedSkill: AiSkillSummary | null = null;
  let activeSkillCommand: SkillCommandMatch | null = null;
  let skillOptions: SkillOption[] = [];
  let highlightedSkillIndex = 0;
  let dismissedCommandKey = '';
  let autoMatchedSkillId = '';
  let autoMatchedReason = '';
  let autoMatchedSkill: AiSkillSummary | null = null;
  let matchTimer: ReturnType<typeof setTimeout> | null = null;
  let lastMatchedInput = '';

  const dispatch = createEventDispatcher<{
    send: { content: string; includeContext: boolean; skillId: string | null };
    cancel: void;
    toggleContext: boolean;
    changeMode: 'chat' | 'agent';
  }>();

  function autoResize() {
    if (!textareaEl) return;
    textareaEl.style.height = 'auto';
    textareaEl.style.height = Math.min(textareaEl.scrollHeight, 200) + 'px';
  }

  function commandKey(match: SkillCommandMatch | null): string {
    return match ? `${match.start}:${match.end}:${match.query}` : '';
  }

  function syncSkillCommand(resetHighlight = false) {
    const nextCommand = findSkillCommand(value, textareaEl?.selectionStart ?? value.length);
    const previousQuery = activeSkillCommand?.query ?? '';
    const nextKey = commandKey(nextCommand);

    activeSkillCommand = nextCommand;
    skillOptions = nextCommand ? buildSkillOptions(skills, nextCommand.query) : [];

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

  function applySkillSelection(skillId: string | null) {
    selectedSkillId = skillId ?? '';
    autoMatchedSkillId = '';
    autoMatchedReason = '';

    if (activeSkillCommand) {
      const nextValue = removeSkillCommand(value, activeSkillCommand);
      value = nextValue.text;
      dismissedCommandKey = '';

      requestAnimationFrame(() => {
        autoResize();
        textareaEl?.focus();
        textareaEl?.setSelectionRange(nextValue.cursor, nextValue.cursor);
        syncSkillCommand();
      });
      return;
    }

    textareaEl?.focus();
  }

  function clearSkillSelection() {
    selectedSkillId = '';
    queueSkillMatch();
    textareaEl?.focus();
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

    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }

  function handleInput() {
    autoResize();
    syncSkillCommand(true);
    queueSkillMatch();
  }

  function handleCaretChange() {
    syncSkillCommand();
  }

  function submit() {
    const content = value.trim();
    if (!content || disabled || isSending) return;
    dispatch('send', {
      content,
      includeContext,
      skillId: selectedSkillId || autoMatchedSkillId || null,
    });
    value = '';
    selectedSkillId = '';
    autoMatchedSkillId = '';
    autoMatchedReason = '';
    lastMatchedInput = '';
    activeSkillCommand = null;
    skillOptions = [];
    dismissedCommandKey = '';
    if (textareaEl) {
      textareaEl.style.height = 'auto';
    }
  }

  function cancel() {
    if (!isSending) return;
    dispatch('cancel');
  }

  function toggleContext() {
    includeContext = !includeContext;
    dispatch('toggleContext', includeContext);
  }

  export function insertText(text: string, options: { asCodeBlock?: boolean } = {}) {
    const normalized = text.replace(/\r\n/g, '\n').trim();
    if (!normalized) return;

    const inserted = options.asCodeBlock
      ? `\`\`\`text\n${normalized}\n\`\`\``
      : normalized;

    value = value.trim()
      ? `${value.replace(/\s+$/, '')}\n\n${inserted}`
      : inserted;

    requestAnimationFrame(() => {
      autoResize();
      textareaEl?.focus();
      textareaEl?.setSelectionRange(value.length, value.length);
    });
  }

  export function focus() {
    textareaEl?.focus();
  }

  function dismissAutoMatch() {
    autoMatchedSkillId = '';
    autoMatchedReason = '';
    lastMatchedInput = value.trim();
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
    const input = value.trim();
    if (!input || selectedSkillId || disabled || isSending || activeSkillCommand) {
      autoMatchedSkillId = '';
      autoMatchedReason = '';
      return;
    }
    if (input === lastMatchedInput) {
      return;
    }

    lastMatchedInput = input;
    const result: AiSkillMatchResult = await matchSkills(input, 'chat');
    if (value.trim() !== input || selectedSkillId) {
      return;
    }
    autoMatchedSkillId = result.auto_applied ? result.matched_skill_id ?? '' : '';
    autoMatchedReason = result.reason ?? '';
  }

  onDestroy(() => {
    if (matchTimer) {
      clearTimeout(matchTimer);
    }
  });

  $: selectedSkill = skills.find((skill) => skill.id === selectedSkillId) ?? null;
  $: autoMatchedSkill = !selectedSkill ? getSkillById(autoMatchedSkillId || null) : null;
  $: activeCommandKey = commandKey(activeSkillCommand);
  $: showSkillMenu =
    !!activeSkillCommand &&
    activeCommandKey !== dismissedCommandKey &&
    !disabled &&
    !isSending;
</script>

<div class="border-t border-app-border bg-app-bg p-3">
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
      bind:this={textareaEl}
      bind:value
      on:input={handleInput}
      on:keydown={handleKeydown}
      on:click={handleCaretChange}
      on:keyup={handleCaretChange}
      on:focus={handleCaretChange}
      placeholder="提问，输入“/”选择 Skill"
      rows="1"
      disabled={disabled || isSending}
      class="w-full resize-none bg-transparent px-0 py-0 text-sm text-app-text placeholder-app-text-secondary/80
             outline-none transition-colors disabled:opacity-50 disabled:cursor-not-allowed
             min-h-[34px] max-h-[200px] leading-relaxed"
    ></textarea>

    <div class="mt-2 flex items-center gap-2">
      <AiModeSwitcher mode={activeMode} on:change={(e) => dispatch('changeMode', e.detail)} />

      <div class="ml-auto flex items-center gap-1.5">
        {#if hasActiveSession}
          <button
            class="inline-flex h-8 w-8 items-center justify-center rounded-full border transition-colors
              {includeContext
                ? 'border-primary-500/40 bg-primary-600/15 text-primary-400'
                : 'border-app-border bg-app-bg text-app-text-secondary hover:text-app-text hover:border-app-border/80'}"
            on:click={toggleContext}
            title={includeContext ? '已附加终端上下文' : '附加终端上下文'}
            type="button"
          >
            <svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 9l3 3-3 3m5 0h3M5 20h14a2 2 0 002-2V6a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z" />
            </svg>
          </button>
        {/if}

        <button
          on:click={isSending ? cancel : submit}
          disabled={disabled}
          class="inline-flex h-8 w-8 items-center justify-center rounded-full border transition-colors
            disabled:opacity-40 disabled:cursor-not-allowed
            {isSending
              ? 'border-red-500/30 bg-red-500/12 text-red-400 hover:bg-red-500/20'
              : 'border-app-border bg-app-bg text-app-text-secondary hover:text-app-text hover:border-app-border/80'}"
          title={isSending ? '暂停生成' : '发送'}
          type="button"
        >
          {#if isSending}
            <svg class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
              <path d="M8 7a1 1 0 011 1v8a1 1 0 11-2 0V8a1 1 0 011-1zm8 0a1 1 0 011 1v8a1 1 0 11-2 0V8a1 1 0 011-1z" />
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
