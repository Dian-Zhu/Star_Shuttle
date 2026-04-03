<script lang="ts">
  import { passwordPromptRequest } from '../lib/store';
  import { onMount, tick } from 'svelte';

  let password = '';
  let inputEl: HTMLInputElement | undefined;

  function handleSubmit() {
    const trimmed = password.trim();
    if (!trimmed) return;
    $passwordPromptRequest?.resolve(trimmed);
    passwordPromptRequest.set(null);
    password = '';
  }

  function handleCancel() {
    $passwordPromptRequest?.resolve(null);
    passwordPromptRequest.set(null);
    password = '';
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      handleCancel();
    }
  }

  onMount(async () => {
    await tick();
    inputEl?.focus();
  });
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="fixed inset-0 z-[1001] flex items-center justify-center bg-black/50" role="dialog" aria-modal="true">
  <div class="bg-app-surface border border-app-border p-[30px] rounded-xl w-[90%] max-w-[400px] shadow-2xl">
    <div class="text-base font-semibold text-app-text mb-[15px]">
      {$passwordPromptRequest?.title ?? ''}
    </div>

    <form on:submit|preventDefault={handleSubmit}>
      <input
        bind:this={inputEl}
        bind:value={password}
        type="password"
        autocomplete="off"
        placeholder="请输入密码"
        class="w-full px-3 py-2 border border-app-border bg-app-bg rounded-lg text-sm text-app-text placeholder-app-text-secondary focus:outline-none focus:border-primary-500 mb-[20px]"
      />

      <div class="flex justify-end gap-[10px]">
        <button
          type="button"
          class="px-4 py-2 border border-app-border bg-app-bg rounded-lg text-sm text-app-text cursor-pointer hover:bg-app-bg-hover transition-colors"
          on:click={handleCancel}
        >
          取消
        </button>
        <button
          type="submit"
          class="px-4 py-2 border border-transparent bg-primary-600 rounded-lg text-sm text-white cursor-pointer hover:bg-primary-500 transition-colors shadow-lg shadow-primary-900/20"
          disabled={!password.trim()}
        >
          确认
        </button>
      </div>
    </form>
  </div>
</div>
