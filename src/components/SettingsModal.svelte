<script lang="ts">
  import { showSettings, settings } from '../lib/store';
  import XIcon from './icons/XIcon.svelte';
  import { slide } from 'svelte/transition';

  let activeTab = 'terminal';

  const tabs = [
    { id: 'general', label: '通用' },
    { id: 'terminal', label: '终端' },
    { id: 'connection', label: '连接' },
    { id: 'appearance', label: '外观' }
  ];

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
            <div class="text-center text-slate-500 py-10">
              <p>更多通用设置正在开发中...</p>
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
