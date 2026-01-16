<script lang="ts">
  import { showConnectionForm } from '../lib/store';
  import { saveConnection } from '../lib/connectionService';
  import XIcon from './icons/XIcon.svelte';

  // Form state
  let formData = {
    name: '',
    host: '',
    port: 22,
    username: '',
    authMethod: 'password' as 'password' | 'privateKey' | 'agent' | 'certificate',
    password: '',
    savePassword: false,
    keyPath: '',
    passphrase: '',
    savePassphrase: false,
    agentPath: '',
    certificatePath: '',
    privateKeyPath: '',
    description: '',
    tags: '',
  };

  let isSaving = false;

  async function handleSubmit() {
    isSaving = true;
    const success = await saveConnection(formData);
    isSaving = false;
    
    if (success) {
      handleClose();
    }
  }

  function handleClose() {
    showConnectionForm.set(false);
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4">
  <div class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-2xl w-full max-w-2xl max-h-[90vh] flex flex-col overflow-hidden">
    <!-- Header -->
    <div class="flex items-center justify-between px-6 py-4 border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
      <h2 class="text-lg font-semibold text-slate-900 dark:text-slate-100">新建连接</h2>
      <button 
        class="text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white transition-colors p-1 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800"
        on:click={handleClose}
      >
        <XIcon class="w-5 h-5" />
      </button>
    </div>

    <!-- Scrollable Content -->
    <div class="flex-1 overflow-y-auto p-6 custom-scrollbar">
      <form id="connection-form" on:submit|preventDefault={handleSubmit} class="space-y-6">
        <!-- Basic Info -->
        <div class="grid grid-cols-1 md:grid-cols-2 gap-5">
          <div class="md:col-span-2">
            <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="name">连接名称</label>
            <input
              type="text"
              id="name"
              bind:value={formData.name}
              class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all"
              placeholder="例如: 生产环境服务器"
              required
            />
          </div>

          <div class="md:col-span-2 grid grid-cols-12 gap-4">
            <div class="col-span-8">
              <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="host">主机地址</label>
              <input
                type="text"
                id="host"
                bind:value={formData.host}
                class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono"
                placeholder="192.168.1.1 或 example.com"
                required
              />
            </div>
            <div class="col-span-4">
              <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="port">端口</label>
              <input
                type="number"
                id="port"
                bind:value={formData.port}
                class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono"
                min="1"
                max="65535"
                required
              />
            </div>
          </div>

          <div class="md:col-span-2">
            <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="username">用户名</label>
            <input
              type="text"
              id="username"
              bind:value={formData.username}
              class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono"
              placeholder="root"
              required
            />
          </div>
        </div>

        <!-- Authentication -->
        <div class="border-t border-slate-200 dark:border-slate-800 pt-5">
          <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">认证方式</span>
          
          <div class="flex space-x-4 mb-4">
            <label class="flex items-center cursor-pointer">
              <input type="radio" bind:group={formData.authMethod} value="password" class="w-4 h-4 text-blue-600 bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
              <span class="ml-2 text-sm text-slate-700 dark:text-slate-300">密码</span>
            </label>
            <label class="flex items-center cursor-pointer">
              <input type="radio" bind:group={formData.authMethod} value="privateKey" class="w-4 h-4 text-blue-600 bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
              <span class="ml-2 text-sm text-slate-700 dark:text-slate-300">私钥</span>
            </label>
            <label class="flex items-center cursor-pointer">
              <input type="radio" bind:group={formData.authMethod} value="agent" class="w-4 h-4 text-blue-600 bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
              <span class="ml-2 text-sm text-slate-700 dark:text-slate-300">Agent</span>
            </label>
          </div>

          <div class="bg-slate-50/50 dark:bg-slate-950/50 rounded-lg p-4 border border-slate-200/50 dark:border-slate-800/50">
            {#if formData.authMethod === 'password'}
              <div>
                <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="password">密码</label>
                <input
                  type="password"
                  id="password"
                  bind:value={formData.password}
                  class="w-full bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all"
                />
                <label class="flex items-center mt-2 cursor-pointer">
                  <input type="checkbox" bind:checked={formData.savePassword} class="rounded border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-blue-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                  <span class="ml-2 text-xs text-slate-500 dark:text-slate-400">保存密码</span>
                </label>
              </div>
            {:else if formData.authMethod === 'privateKey'}
              <div class="space-y-3">
                <div>
                  <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="keyPath">私钥路径</label>
                  <input
                    type="text"
                    id="keyPath"
                    bind:value={formData.keyPath}
                    class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono text-sm"
                    placeholder="~/.ssh/id_rsa"
                  />
                </div>
                <div>
                  <label class="block text-sm font-medium text-slate-400 mb-1.5" for="passphrase">密码短语 (可选)</label>
                  <input
                    type="password"
                    id="passphrase"
                    bind:value={formData.passphrase}
                    class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all"
                  />
                  <label class="flex items-center mt-2 cursor-pointer">
                    <input type="checkbox" bind:checked={formData.savePassphrase} class="rounded border-slate-700 bg-slate-900 text-blue-600 focus:ring-blue-600 ring-offset-slate-900">
                    <span class="ml-2 text-xs text-slate-400">保存密码短语</span>
                  </label>
                </div>
              </div>
            {:else if formData.authMethod === 'agent'}
              <div>
                <label class="block text-sm font-medium text-slate-400 mb-1.5" for="agentPath">Agent 路径 (可选)</label>
                <input
                  type="text"
                  id="agentPath"
                  bind:value={formData.agentPath}
                  class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono text-sm"
                  placeholder="默认使用系统 SSH_AUTH_SOCK"
                />
              </div>
            {/if}
          </div>
        </div>

        <!-- Optional Info -->
        <div class="border-t border-slate-800 pt-5">
          <div class="grid grid-cols-1 gap-4">
            <div>
              <label class="block text-sm font-medium text-slate-400 mb-1.5" for="tags">标签</label>
              <input
                type="text"
                id="tags"
                bind:value={formData.tags}
                class="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all"
                placeholder="生产环境, Web服务器 (用逗号分隔)"
              />
            </div>
            <div>
              <label class="block text-sm font-medium text-slate-400 mb-1.5" for="description">描述</label>
              <textarea
                id="description"
                bind:value={formData.description}
                rows="3"
                class="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all resize-none"
                placeholder="关于此服务器的备注信息..."
              ></textarea>
            </div>
          </div>
        </div>
      </form>
    </div>

    <!-- Footer -->
    <div class="px-6 py-4 border-t border-slate-800 bg-slate-900 flex justify-end gap-3">
      <button
        type="button"
        class="px-4 py-2 text-sm font-medium text-slate-300 hover:text-white bg-slate-800 hover:bg-slate-700 rounded-lg transition-colors"
        on:click={handleClose}
      >
        取消
      </button>
      <button
        type="submit"
        form="connection-form"
        disabled={isSaving}
        class="px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-500 active:bg-blue-700 rounded-lg transition-colors shadow-lg shadow-blue-900/20 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
      >
        {#if isSaving}
          <div class="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
          <span>保存中...</span>
        {:else}
          <span>保存连接</span>
        {/if}
      </button>
    </div>
  </div>
</div>

<style>
  .custom-scrollbar::-webkit-scrollbar {
    width: 6px;
  }
  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: #334155;
    border-radius: 3px;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: #475569;
  }
</style>
