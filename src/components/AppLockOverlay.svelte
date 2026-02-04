<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { fade } from 'svelte/transition';
  import { isLocked } from '../lib/store';

  let password = '';
  let isLoading = false;
  let error = '';

  async function handleUnlock() {
    if (!password) return;
    
    isLoading = true;
    error = '';
    
    try {
      const isValid = await invoke('verify_app_lock', { password });
      if (isValid) {
        isLocked.set(false);
      } else {
        error = '密码错误';
        password = '';
      }
    } catch (e) {
      error = `验证失败: ${e}`;
    } finally {
      isLoading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      handleUnlock();
    }
  }
</script>

<div class="fixed inset-0 z-[100] bg-app-bg flex items-center justify-center" transition:fade>
  <div class="w-full max-w-md p-8 bg-app-surface rounded-xl shadow-2xl border border-app-border">
    <div class="text-center mb-8">
      <div class="w-16 h-16 bg-primary-600 rounded-2xl mx-auto flex items-center justify-center mb-4 shadow-lg shadow-primary-900/30">
        <svg class="w-8 h-8 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z"></path>
        </svg>
      </div>
      <h2 class="text-2xl font-bold text-app-text mb-2">应用已锁定</h2>
      <p class="text-app-text-secondary">请输入密码解锁 Star Shuttle</p>
    </div>

    <div class="space-y-4">
      <div>
        <input
          type="password"
          bind:value={password}
          on:keydown={handleKeydown}
          placeholder="输入密码"
          class="w-full bg-app-bg border border-app-border rounded-lg px-4 py-3 text-app-text placeholder-app-text-secondary/50 focus:outline-none focus:border-primary-500 focus:ring-1 focus:ring-primary-500 transition-all"
        />
        {#if error}
          <p class="mt-2 text-sm text-red-500 dark:text-red-400">{error}</p>
        {/if}
      </div>

      <button
        class="w-full bg-primary-600 hover:bg-primary-500 text-white font-medium py-3 rounded-lg transition-all shadow-lg shadow-primary-900/20 disabled:opacity-50 disabled:cursor-not-allowed"
        on:click={handleUnlock}
        disabled={isLoading || !password}
      >
        {#if isLoading}
          解锁中...
        {:else}
          解锁
        {/if}
      </button>
    </div>
  </div>
</div>
