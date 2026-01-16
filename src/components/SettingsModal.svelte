<script lang="ts">
  import { showSettings, settings } from '../lib/store';
  import XIcon from './icons/XIcon.svelte';
  import { slide } from 'svelte/transition';
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  let activeTab = 'terminal';

  const tabs = [
    { id: 'general', label: '通用' },
    { id: 'terminal', label: '终端' },
    { id: 'connection', label: '连接' },
    { id: 'appearance', label: '外观' },
    { id: 'security', label: '安全' }
  ];

  // Security State
  let hasLock = false;
  let oldPassword = '';
  let newPassword = '';
  let confirmPassword = '';
  let securityMessage = '';
  let securityError = '';

  onMount(() => {
    checkLockStatus();
  });

  async function checkLockStatus() {
    try {
      hasLock = await invoke('is_app_lock_enabled');
    } catch (e) {
      console.error(e);
    }
  }

  async function handleSetLock() {
    if (newPassword !== confirmPassword) {
      securityError = '两次输入的密码不一致';
      return;
    }
    if (!newPassword) {
      securityError = '密码不能为空';
      return;
    }
    
    try {
      await invoke('set_app_lock', { password: newPassword });
      hasLock = true;
      securityMessage = '应用锁已设置';
      securityError = '';
      newPassword = '';
      confirmPassword = '';
    } catch (e) {
      securityError = `设置失败: ${e}`;
    }
  }

  async function handleChangeLock() {
     if (!oldPassword) {
       securityError = '请输入当前密码';
       return;
     }
     
     // Verify old password first
     try {
       const isValid = await invoke('verify_app_lock', { password: oldPassword });
       if (!isValid) {
         securityError = '当前密码错误';
         return;
       }
       
       if (newPassword !== confirmPassword) {
         securityError = '两次输入的新密码不一致';
         return;
       }

       if (!newPassword) {
         securityError = '新密码不能为空';
         return;
       }
       
       await invoke('set_app_lock', { password: newPassword });
       securityMessage = '密码已更新';
       securityError = '';
       oldPassword = '';
       newPassword = '';
       confirmPassword = '';
     } catch (e) {
       securityError = `修改失败: ${e}`;
     }
  }

  async function handleRemoveLock() {
    if (!oldPassword) {
       securityError = '请输入当前密码以确认清除';
       return;
    }

    if (!confirm('确定要清除应用锁吗？清除后应用启动将不再需要密码。')) return;
    
    try {
      const isValid = await invoke('verify_app_lock', { password: oldPassword });
       if (!isValid) {
         securityError = '当前密码错误';
         return;
       }

      await invoke('remove_app_lock');
      hasLock = false;
      securityMessage = '应用锁已清除';
      securityError = '';
      oldPassword = '';
      newPassword = '';
      confirmPassword = '';
    } catch (e) {
      securityError = `清除失败: ${e}`;
    }
  }

  function handleClose() {
    showSettings.set(false);
  }

  // Pre-defined font families
  const fontFamilies = [
    { label: 'Monospace (Default)', value: 'Menlo, Monaco, "Courier New", monospace' },
    { label: 'Fira Code', value: '"Fira Code", monospace' },
    { label: 'JetBrains Mono', value: '"JetBrains Mono", monospace' },
    { label: 'Source Code Pro', value: '"Source Code Pro", monospace' }
  ];
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4" role="button" tabindex="0" on:click|self={handleClose} on:keydown={(e) => e.key === 'Escape' && handleClose()}>
  <div class="bg-slate-900 border border-slate-800 rounded-xl shadow-2xl w-full max-w-3xl h-[600px] flex overflow-hidden">
    
    <!-- Sidebar -->
    <div class="w-48 border-r border-slate-800 bg-slate-950/50 p-4 flex flex-col gap-1">
      <h2 class="text-lg font-semibold text-slate-100 px-3 py-2 mb-2">设置</h2>
      
      {#each tabs as tab}
        <button
          class="text-left px-3 py-2 rounded-lg text-sm font-medium transition-colors {activeTab === tab.id ? 'bg-blue-600 text-white' : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800'}"
          on:click={() => activeTab = tab.id}
        >
          {tab.label}
        </button>
      {/each}
    </div>

    <!-- Content -->
    <div class="flex-1 flex flex-col min-w-0">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-slate-900">
        <h3 class="text-base font-medium text-slate-200">
          {tabs.find(t => t.id === activeTab)?.label}
        </h3>
        <button 
          class="text-slate-400 hover:text-white transition-colors p-1 rounded-md hover:bg-slate-800"
          on:click={handleClose}
        >
          <XIcon class="w-5 h-5" />
        </button>
      </div>

      <!-- Settings Panel -->
      <div class="flex-1 overflow-y-auto p-6 custom-scrollbar">
        {#if activeTab === 'general'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            <!-- Theme -->
            <div>
              <label class="block text-sm font-medium text-slate-400 mb-2" for="theme">
                主题
              </label>
              <div class="grid grid-cols-3 gap-3">
                <button
                  class="px-4 py-2 rounded-lg border {$settings.theme === 'light' ? 'bg-blue-600 border-blue-600 text-white' : 'border-slate-700 bg-slate-900 text-slate-400 hover:bg-slate-800'}"
                  on:click={() => $settings.theme = 'light'}
                >
                  亮色
                </button>
                <button
                  class="px-4 py-2 rounded-lg border {$settings.theme === 'dark' ? 'bg-blue-600 border-blue-600 text-white' : 'border-slate-700 bg-slate-900 text-slate-400 hover:bg-slate-800'}"
                  on:click={() => $settings.theme = 'dark'}
                >
                  暗色
                </button>
                <button
                  class="px-4 py-2 rounded-lg border border-slate-700 bg-slate-900 text-slate-500 cursor-not-allowed opacity-50"
                  disabled
                  title="暂不支持"
                >
                  跟随系统
                </button>
              </div>
            </div>

            <!-- Language -->
            <div>
              <label class="block text-sm font-medium text-slate-400 mb-2" for="language">
                语言
              </label>
              <select
                id="language"
                class="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:border-blue-500 outline-none"
              >
                <option value="zh-CN">简体中文</option>
                <option value="en-US" disabled>English (Coming Soon)</option>
              </select>
            </div>

            <!-- App Info -->
            <div class="pt-6 border-t border-slate-800">
               <div class="flex justify-between items-center">
                 <div>
                   <h4 class="text-sm font-medium text-slate-200">关于 Star Shuttle</h4>
                   <p class="text-xs text-slate-500 mt-1">Version 0.1.0</p>
                 </div>
                 <button class="text-xs text-blue-500 hover:text-blue-400 transition-colors">
                   检查更新
                 </button>
               </div>
            </div>
          </div>

        {:else if activeTab === 'terminal'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            <!-- Font Size -->
            <div>
              <label class="block text-sm font-medium text-slate-400 mb-2" for="fontSize">
                字体大小 ({$settings.terminal.fontSize}px)
              </label>
              <div class="flex items-center gap-4">
                <input
                  type="range"
                  id="fontSize"
                  min="10"
                  max="24"
                  step="1"
                  bind:value={$settings.terminal.fontSize}
                  class="flex-1 h-2 bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-600"
                />
                <input 
                  type="number" 
                  bind:value={$settings.terminal.fontSize}
                  class="w-16 bg-slate-950 border border-slate-700 rounded-lg px-2 py-1 text-center text-slate-200 focus:border-blue-500 outline-none"
                />
              </div>
            </div>

            <!-- Font Family -->
            <div>
              <label class="block text-sm font-medium text-slate-400 mb-2" for="fontFamily">
                字体
              </label>
              <select
                id="fontFamily"
                bind:value={$settings.terminal.fontFamily}
                class="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:border-blue-500 outline-none"
              >
                {#each fontFamilies as font}
                  <option value={font.value}>{font.label}</option>
                {/each}
              </select>
              <p class="mt-2 text-xs text-slate-500">
                当前字体预览: <span style="font-family: {$settings.terminal.fontFamily}">The quick brown fox jumps over the lazy dog 0123456789</span>
              </p>
            </div>

            <!-- Cursor Blink -->
            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-slate-400" for="cursorBlink">
                  光标闪烁
                </label>
                <p class="text-xs text-slate-500 mt-0.5">启用后光标将闪烁</p>
              </div>
              <label class="relative inline-flex items-center cursor-pointer">
                <input type="checkbox" id="cursorBlink" bind:checked={$settings.terminal.cursorBlink} class="sr-only peer">
                <div class="w-11 h-6 bg-slate-700 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-blue-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
              </label>
            </div>
          </div>

        {:else if activeTab === 'connection'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            <!-- Auto Reconnect -->
            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-slate-400" for="autoReconnect">
                  自动重连
                </label>
                <p class="text-xs text-slate-500 mt-0.5">意外断开连接时尝试自动重新连接</p>
              </div>
              <label class="relative inline-flex items-center cursor-pointer">
                <input type="checkbox" id="autoReconnect" bind:checked={$settings.connection.autoReconnect} class="sr-only peer">
                <div class="w-11 h-6 bg-slate-700 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-blue-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
              </label>
            </div>
          </div>

        {:else if activeTab === 'appearance'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            <!-- Theme -->
            <div>
              <span class="block text-sm font-medium text-slate-400 mb-3">主题模式</span>
              <div class="grid grid-cols-2 gap-4">
                <button
                  class="relative p-4 border rounded-xl flex flex-col items-center gap-2 transition-all {$settings.theme === 'dark' ? 'border-blue-500 bg-blue-500/10' : 'border-slate-700 hover:border-slate-600'}"
                  on:click={() => $settings.theme = 'dark'}
                >
                  <div class="w-full h-20 bg-slate-900 rounded-lg border border-slate-800 shadow-sm overflow-hidden relative">
                    <div class="absolute left-0 top-0 bottom-0 w-8 bg-slate-800 border-r border-slate-700"></div>
                    <div class="absolute right-2 top-2 w-12 h-2 bg-slate-700 rounded"></div>
                  </div>
                  <span class="text-sm font-medium text-slate-200">深色模式</span>
                  {#if $settings.theme === 'dark'}
                    <div class="absolute top-2 right-2 w-2 h-2 bg-blue-500 rounded-full"></div>
                  {/if}
                </button>

                <button
                  class="relative p-4 border rounded-xl flex flex-col items-center gap-2 transition-all {$settings.theme === 'light' ? 'border-blue-500 bg-blue-500/10' : 'border-slate-700 hover:border-slate-600'}"
                  on:click={() => $settings.theme = 'light'}
                >
                  <div class="w-full h-20 bg-slate-100 rounded-lg border border-slate-200 shadow-sm overflow-hidden relative">
                    <div class="absolute left-0 top-0 bottom-0 w-8 bg-white border-r border-slate-200"></div>
                    <div class="absolute right-2 top-2 w-12 h-2 bg-slate-200 rounded"></div>
                  </div>
                  <span class="text-sm font-medium text-slate-200">浅色模式</span>
                  {#if $settings.theme === 'light'}
                    <div class="absolute top-2 right-2 w-2 h-2 bg-blue-500 rounded-full"></div>
                  {/if}
                </button>
              </div>
            </div>
          </div>
        {:else if activeTab === 'security'}
           <div class="space-y-6" in:slide={{ duration: 200 }}>
             <h3 class="text-lg font-medium text-slate-200">应用安全锁</h3>
             <p class="text-sm text-slate-400">设置启动密码以保护您的连接信息。</p>
             
             {#if securityMessage}
               <div class="p-3 bg-green-500/10 border border-green-500/20 text-green-400 rounded-lg text-sm">
                 {securityMessage}
               </div>
             {/if}
             
             {#if securityError}
               <div class="p-3 bg-red-500/10 border border-red-500/20 text-red-400 rounded-lg text-sm">
                 {securityError}
               </div>
             {/if}

             {#if !hasLock}
               <!-- Setup Lock -->
               <div class="space-y-4 border border-slate-800 rounded-lg p-4 bg-slate-950/30">
                 <h4 class="font-medium text-slate-300">设置新密码</h4>
                 <div>
                   <label class="block text-sm font-medium text-slate-400 mb-1" for="new-pwd">新密码</label>
                   <input type="password" id="new-pwd" bind:value={newPassword} class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-white focus:border-blue-500 outline-none" />
                 </div>
                 <div>
                   <label class="block text-sm font-medium text-slate-400 mb-1" for="confirm-pwd">确认密码</label>
                   <input type="password" id="confirm-pwd" bind:value={confirmPassword} class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-white focus:border-blue-500 outline-none" />
                 </div>
                 <button class="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm font-medium transition-colors" on:click={handleSetLock}>
                   启用应用锁
                 </button>
               </div>
             {:else}
               <!-- Change/Remove Lock -->
               <div class="space-y-4 border border-slate-800 rounded-lg p-4 bg-slate-950/30">
                 <h4 class="font-medium text-slate-300">管理密码</h4>
                 <div>
                   <label class="block text-sm font-medium text-slate-400 mb-1" for="curr-pwd">当前密码</label>
                   <input type="password" id="curr-pwd" bind:value={oldPassword} class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-white focus:border-blue-500 outline-none" />
                 </div>
                 
                 <div class="pt-4 border-t border-slate-800">
                    <h5 class="text-sm font-medium text-slate-400 mb-3">修改密码（可选）</h5>
                    <div class="space-y-3">
                        <div>
                        <label class="block text-sm font-medium text-slate-500 mb-1" for="new-pwd-change">新密码</label>
                        <input type="password" id="new-pwd-change" bind:value={newPassword} class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-white focus:border-blue-500 outline-none" />
                        </div>
                        <div>
                        <label class="block text-sm font-medium text-slate-500 mb-1" for="confirm-pwd-change">确认新密码</label>
                        <input type="password" id="confirm-pwd-change" bind:value={confirmPassword} class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-white focus:border-blue-500 outline-none" />
                        </div>
                        <div class="flex gap-3 pt-2">
                            <button class="px-4 py-2 bg-slate-700 hover:bg-slate-600 text-white rounded-lg text-sm font-medium transition-colors" on:click={handleChangeLock}>
                            更新密码
                            </button>
                            <button class="px-4 py-2 bg-red-900/30 hover:bg-red-900/50 text-red-400 border border-red-900/50 rounded-lg text-sm font-medium transition-colors" on:click={handleRemoveLock}>
                            清除应用锁
                            </button>
                        </div>
                    </div>
                 </div>
               </div>
             {/if}
           </div>
        {/if}
      </div>
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
