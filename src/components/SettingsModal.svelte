<script lang="ts">
  // Force rebuild.
  import { showSettings, settings, type AppSettings } from '../lib/store';
  import XIcon from './icons/XIcon.svelte';
  import { slide } from 'svelte/transition';
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  let activeTab = 'terminal';

  const tabs = [
    { id: 'general', label: '通用' },
    { id: 'shortcuts', label: '快捷键' },
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

  type ShortcutKey = keyof AppSettings['shortcuts'];

  let shortcutDrafts: AppSettings['shortcuts'] = { ...$settings.shortcuts };
  let shortcutErrors: Partial<Record<ShortcutKey, string>> = {};

  const shortcutLabels: Record<ShortcutKey, string> = {
    commandPalette: '命令面板',
    toggleSidebar: '切换侧边栏',
    newConnection: '新建连接',
    settings: '设置',
    closeTerminal: '关闭终端',
    prevTab: '上一个标签页',
    nextTab: '下一个标签页',
    copy: '复制',
    paste: '粘贴'
  };

  onMount(() => {
    checkLockStatus();
  });

  function updateTheme(theme: 'dark' | 'light') {
    settings.update(s => ({ ...s, theme }));
  }

  function updateTerminalTheme(theme: 'auto' | 'dracula' | 'nord' | 'solarized-dark' | 'solarized-light' | 'monokai' | 'one-dark' | 'github-dark' | 'tokyo-night' | 'catppuccin' | 'custom') {
    settings.update(s => ({
      ...s,
      appearance: {
        ...s.appearance,
        terminalTheme: theme,
        customTheme: theme === 'custom' ? s.appearance.customTheme || defaultCustomTheme : undefined
      }
    }));
  }

  function updateUiSetting<K extends keyof (typeof $settings)['ui']>(key: K, value: (typeof $settings)['ui'][K]) {
    settings.update(s => ({
      ...s,
      ui: {
        ...s.ui,
        [key]: value
      }
    }));
  }

  function updateTerminalSetting<K extends keyof (typeof $settings)['terminal']>(
    key: K,
    value: (typeof $settings)['terminal'][K]
  ) {
    settings.update(s => ({
      ...s,
      terminal: {
        ...s.terminal,
        [key]: value
      }
    }));
  }

  function updateTerminalScrollback(raw: string) {
    const v = Number(raw);
    if (!Number.isFinite(v)) return;
    const clamped = Math.max(1000, Math.min(50000, Math.trunc(v)));
    updateTerminalSetting('scrollback', clamped);
  }

  function updateConnectionSetting<K extends keyof (typeof $settings)['connection']>(
    key: K,
    value: (typeof $settings)['connection'][K]
  ) {
    settings.update(s => ({
      ...s,
      connection: {
        ...s.connection,
        [key]: value
      }
    }));
  }

  function updateShortcutSetting<K extends keyof (typeof $settings)['shortcuts']>(
    key: K,
    value: (typeof $settings)['shortcuts'][K]
  ) {
    settings.update(s => ({
      ...s,
      shortcuts: {
        ...s.shortcuts,
        [key]: value
      }
    }));
  }

  function updateSecuritySetting<K extends keyof (typeof $settings)['security']>(
    key: K,
    value: (typeof $settings)['security'][K]
  ) {
    settings.update(s => ({
      ...s,
      security: {
        ...s.security,
        [key]: value
      }
    }));
  }

  function normalizeShortcut(raw: string): { value: string } | { error: string } {
    const trimmed = raw.trim();
    if (!trimmed) return { value: '' };
    const parts = trimmed.split('+').map(p => p.trim()).filter(Boolean);
    if (parts.length === 0) return { value: '' };
    if (parts.length === 1) return { value: formatShortcut([], parts[0]) };

    const key = parts[parts.length - 1];
    const modifierParts = parts.slice(0, -1);

    const modifiers = new Set<'Ctrl' | 'Shift' | 'Alt' | 'Meta'>();
    for (const m of modifierParts) {
      const lower = m.toLowerCase();
      if (lower === 'ctrl' || lower === 'control') modifiers.add('Ctrl');
      else if (lower === 'shift') modifiers.add('Shift');
      else if (lower === 'alt' || lower === 'option') modifiers.add('Alt');
      else if (lower === 'meta' || lower === 'cmd' || lower === 'command') modifiers.add('Meta');
      else return { error: '格式错误：只允许修饰键 + 按键' };
    }

    return { value: formatShortcut(Array.from(modifiers), key) };
  }

  function formatShortcut(modifiers: Array<'Ctrl' | 'Shift' | 'Alt' | 'Meta'>, keyRaw: string): string {
    const order: Array<'Ctrl' | 'Shift' | 'Alt' | 'Meta'> = ['Ctrl', 'Shift', 'Alt', 'Meta'];
    const unique = Array.from(new Set(modifiers));
    const ordered = order.filter(m => unique.includes(m));

    const key = normalizeKeyToken(keyRaw);
    if (!ordered.length) return key;
    return `${ordered.join('+')}+${key}`;
  }

  function normalizeKeyToken(keyRaw: string): string {
    const t = keyRaw.trim();
    if (!t) return '';
    if (t.length === 1) return t.toUpperCase();
    const lower = t.toLowerCase();
    if (lower === 'esc') return 'Escape';
    if (lower === 'return') return 'Enter';
    if (lower === 'del') return 'Delete';
    return t;
  }

  function computeShortcutConflict(
    key: ShortcutKey,
    normalizedValue: string
  ): { conflictWith: ShortcutKey } | null {
    if (!normalizedValue) return null;
    const target = normalizedValue.toLowerCase();
    const entries = Object.entries($settings.shortcuts) as Array<[ShortcutKey, string]>;
    for (const [k, v] of entries) {
      if (k === key) continue;
      const normalized = normalizeShortcut(v);
      const other = 'value' in normalized ? normalized.value.toLowerCase() : v.trim().toLowerCase();
      if (other && other === target) return { conflictWith: k };
    }
    return null;
  }

  function handleShortcutInput(key: ShortcutKey, raw: string) {
    shortcutDrafts = { ...shortcutDrafts, [key]: raw };
    const normalized = normalizeShortcut(raw);
    if ('error' in normalized) {
      shortcutErrors = { ...shortcutErrors, [key]: normalized.error };
      return;
    }
    const conflict = computeShortcutConflict(key, normalized.value);
    if (conflict) {
      shortcutErrors = { ...shortcutErrors, [key]: `与「${shortcutLabels[conflict.conflictWith]}」冲突` };
      return;
    }
    const nextErrors = { ...shortcutErrors };
    delete nextErrors[key];
    shortcutErrors = nextErrors;

    shortcutDrafts = { ...shortcutDrafts, [key]: normalized.value };
    updateShortcutSetting(key, normalized.value);
  }

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

  // Terminal theme presets
  type TerminalThemePreset = AppSettings['appearance']['terminalTheme'];

  const terminalThemes: Array<{ id: TerminalThemePreset; name: string; preview: { background: string; foreground: string } }> = [
    {
      id: 'auto',
      name: '自动',
      preview: { background: '#0f172a', foreground: '#e2e8f0' }
    },
    {
      id: 'dracula',
      name: 'Dracula',
      preview: { background: '#282a36', foreground: '#f8f8f2' }
    },
    {
      id: 'nord',
      name: 'Nord',
      preview: { background: '#2e3440', foreground: '#d8dee9' }
    },
    {
      id: 'solarized-dark',
      name: 'Solarized Dark',
      preview: { background: '#002b36', foreground: '#93a1a1' }
    },
    {
      id: 'solarized-light',
      name: 'Solarized Light',
      preview: { background: '#fdf6e3', foreground: '#657b83' }
    },
    {
      id: 'monokai',
      name: 'Monokai',
      preview: { background: '#272822', foreground: '#f8f8f2' }
    },
    {
      id: 'one-dark',
      name: 'One Dark',
      preview: { background: '#282c34', foreground: '#abb2bf' }
    },
    {
      id: 'github-dark',
      name: 'GitHub Dark',
      preview: { background: '#0d1117', foreground: '#c9d1d9' }
    },
    {
      id: 'tokyo-night',
      name: 'Tokyo Night',
      preview: { background: '#1a1b26', foreground: '#a9b1d6' }
    },
    {
      id: 'catppuccin',
      name: 'Catppuccin',
      preview: { background: '#1e1e2e', foreground: '#cdd6f4' }
    },
    {
      id: 'custom',
      name: '自定义',
      preview: { background: '#1e1e1e', foreground: '#ffffff' }
    }
  ];

  // Default custom theme colors
  const defaultCustomTheme = {
    background: '#1e1e1e',
    foreground: '#d4d4d4',
    cursor: '#ffffff',
    selectionBackground: '#264f78',
    black: '#000000',
    red: '#cd3131',
    green: '#0dbc79',
    yellow: '#e5e510',
    blue: '#2472c8',
    magenta: '#bc3fbc',
    cyan: '#11a8cd',
    white: '#e5e5e5',
    brightBlack: '#666666',
    brightRed: '#f14c4c',
    brightGreen: '#23d18b',
    brightYellow: '#f5f543',
    brightBlue: '#3b8eea',
    brightMagenta: '#d670d6',
    brightCyan: '#29b8db',
    brightWhite: '#ffffff'
  };

  // Color key labels
  type CustomThemeKey = keyof NonNullable<AppSettings['appearance']['customTheme']>;

  const colorLabels: Record<CustomThemeKey, string> = {
    background: '背景',
    foreground: '前景',
    cursor: '光标',
    selectionBackground: '选中背景',
    black: '黑色',
    red: '红色',
    green: '绿色',
    yellow: '黄色',
    blue: '蓝色',
    magenta: '品红',
    cyan: '青色',
    white: '白色',
    brightBlack: '亮黑',
    brightRed: '亮红',
    brightGreen: '亮绿',
    brightYellow: '亮黄',
    brightBlue: '亮蓝',
    brightMagenta: '亮品红',
    brightCyan: '亮青色',
    brightWhite: '亮白'
  };

  const basicColorKeys: CustomThemeKey[] = ['background', 'foreground', 'cursor', 'selectionBackground'];
  const standardColorKeys: CustomThemeKey[] = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white'];
  const brightColorKeys: CustomThemeKey[] = ['brightBlack', 'brightRed', 'brightGreen', 'brightYellow', 'brightBlue', 'brightMagenta', 'brightCyan', 'brightWhite'];

  function updateCustomColor(key: CustomThemeKey, value: string) {
    if (!$settings.appearance.customTheme) {
      settings.update(s => ({
        ...s,
        appearance: {
          ...s.appearance,
          customTheme: { ...defaultCustomTheme }
        }
      }));
      return;
    }
    settings.update(s => ({
      ...s,
      appearance: {
        ...s.appearance,
        customTheme: {
          ...s.appearance.customTheme!,
          [key]: value
        }
      }
    }));
  }

  function resetCustomTheme() {
    settings.update(s => ({
      ...s,
      appearance: {
        ...s.appearance,
        customTheme: { ...defaultCustomTheme }
      }
    }));
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4" role="button" tabindex="0" on:click|self={handleClose} on:keydown={(e) => e.key === 'Escape' && handleClose()}>
  <div class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-2xl w-full max-w-3xl h-[600px] flex overflow-hidden text-slate-900 dark:text-slate-200">
    
    <!-- Sidebar -->
    <div class="w-48 border-r border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950/50 p-4 flex flex-col gap-1">
      <h2 class="text-lg font-semibold text-slate-800 dark:text-slate-100 px-3 py-2 mb-2">设置</h2>
      
      {#each tabs as tab}
        <button
          class="text-left px-3 py-2 rounded-lg text-sm font-medium transition-colors {activeTab === tab.id ? 'bg-blue-600 text-white' : 'text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-slate-200 hover:bg-slate-200 dark:hover:bg-slate-800'}"
          on:click={() => activeTab = tab.id}
        >
          {tab.label}
        </button>
      {/each}
    </div>

    <!-- Content -->
    <div class="flex-1 flex flex-col min-w-0">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
        <h3 class="text-base font-medium text-slate-800 dark:text-slate-200">
          {tabs.find(t => t.id === activeTab)?.label}
        </h3>
        <button 
          class="text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white transition-colors p-1 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800"
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
              <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-2" for="theme">
                主题
              </label>
              <select
                id="theme"
                value={$settings.theme}
                on:change={(e) => updateTheme(e.currentTarget.value as any)}
                class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
              >
                <option value="system">跟随系统</option>
                <option value="dark">深色</option>
                <option value="light">浅色</option>
              </select>
            </div>

            <!-- Language -->
            <div>
              <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-2" for="language">
                语言
              </label>
              <select
                id="language"
                class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                disabled
              >
                <option value="zh-CN">简体中文</option>
                <option value="en-US" disabled>English (Coming Soon)</option>
              </select>
              <p class="mt-2 text-xs text-slate-500">语言设置暂未开放</p>
            </div>

            <!-- UI -->
            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-slate-600 dark:text-slate-400" for="sidebarCollapsed">
                  折叠侧边栏
                </label>
                <p class="text-xs text-slate-500 mt-0.5">在窗口左侧显示紧凑模式</p>
              </div>
              <label class="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  id="sidebarCollapsed"
                  checked={$settings.ui.sidebarCollapsed}
                  on:change={(e) => updateUiSetting('sidebarCollapsed', (e.target as HTMLInputElement).checked)}
                  class="sr-only peer"
                >
                <div class="w-11 h-6 bg-slate-200 dark:bg-slate-700 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-blue-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
              </label>
            </div>

            <!-- App Info -->
            <div class="pt-6 border-t border-slate-200 dark:border-slate-800">
               <div class="flex justify-between items-center">
                 <div>
                   <h4 class="text-sm font-medium text-slate-900 dark:text-slate-200">关于 Star Shuttle</h4>
                   <p class="text-xs text-slate-500 mt-1">Version 0.1.0</p>
                 </div>
                 <button class="text-xs text-blue-500 hover:text-blue-400 transition-colors">
                   检查更新
                 </button>
               </div>
            </div>
          </div>

        {:else if activeTab === 'shortcuts'}
           <div class="space-y-6" in:slide={{ duration: 200 }}>
             <h3 class="text-lg font-medium text-slate-800 dark:text-slate-200">快捷键设置</h3>
             
             <div class="space-y-4">
                <div class="grid grid-cols-2 gap-4 items-center border-b border-slate-200 dark:border-slate-800 pb-4">
                  <div>
                    <span class="block text-sm font-medium text-slate-700 dark:text-slate-300">命令面板</span>
                    <span class="text-xs text-slate-500">快速访问所有命令</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      value={shortcutDrafts.commandPalette}
                      on:input={(e) => handleShortcutInput('commandPalette', (e.target as HTMLInputElement).value)}
                      class="bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 text-sm font-mono focus:border-blue-500 outline-none w-full"
                    />
                    {#if shortcutErrors.commandPalette}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.commandPalette}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-slate-200 dark:border-slate-800 pb-4">
                  <div>
                    <span class="block text-sm font-medium text-slate-700 dark:text-slate-300">新建连接</span>
                    <span class="text-xs text-slate-500">打开新建连接窗口</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      value={shortcutDrafts.newConnection}
                      on:input={(e) => handleShortcutInput('newConnection', (e.target as HTMLInputElement).value)}
                      class="bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 text-sm font-mono focus:border-blue-500 outline-none w-full"
                    />
                    {#if shortcutErrors.newConnection}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.newConnection}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-slate-200 dark:border-slate-800 pb-4">
                  <div>
                    <span class="block text-sm font-medium text-slate-700 dark:text-slate-300">设置</span>
                    <span class="text-xs text-slate-500">打开设置窗口</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      value={shortcutDrafts.settings}
                      on:input={(e) => handleShortcutInput('settings', (e.target as HTMLInputElement).value)}
                      class="bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 text-sm font-mono focus:border-blue-500 outline-none w-full"
                    />
                    {#if shortcutErrors.settings}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.settings}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-slate-200 dark:border-slate-800 pb-4">
                  <div>
                    <span class="block text-sm font-medium text-slate-700 dark:text-slate-300">关闭终端</span>
                    <span class="text-xs text-slate-500">关闭当前活动的终端会话</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      value={shortcutDrafts.closeTerminal}
                      on:input={(e) => handleShortcutInput('closeTerminal', (e.target as HTMLInputElement).value)}
                      class="bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 text-sm font-mono focus:border-blue-500 outline-none w-full"
                    />
                    {#if shortcutErrors.closeTerminal}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.closeTerminal}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-slate-200 dark:border-slate-800 pb-4">
                  <div>
                    <span class="block text-sm font-medium text-slate-700 dark:text-slate-300">上一个标签页</span>
                    <span class="text-xs text-slate-500">切换到左侧终端标签</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      value={shortcutDrafts.prevTab}
                      on:input={(e) => handleShortcutInput('prevTab', (e.target as HTMLInputElement).value)}
                      class="bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 text-sm font-mono focus:border-blue-500 outline-none w-full"
                    />
                    {#if shortcutErrors.prevTab}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.prevTab}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-slate-200 dark:border-slate-800 pb-4">
                  <div>
                    <span class="block text-sm font-medium text-slate-700 dark:text-slate-300">下一个标签页</span>
                    <span class="text-xs text-slate-500">切换到右侧终端标签</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      value={shortcutDrafts.nextTab}
                      on:input={(e) => handleShortcutInput('nextTab', (e.target as HTMLInputElement).value)}
                      class="bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 text-sm font-mono focus:border-blue-500 outline-none w-full"
                    />
                    {#if shortcutErrors.nextTab}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.nextTab}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-slate-200 dark:border-slate-800 pb-4">
                  <div>
                    <span class="block text-sm font-medium text-slate-700 dark:text-slate-300">复制</span>
                    <span class="text-xs text-slate-500">复制选中文件到剪贴板</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      value={shortcutDrafts.copy}
                      on:input={(e) => handleShortcutInput('copy', (e.target as HTMLInputElement).value)}
                      class="bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 text-sm font-mono focus:border-blue-500 outline-none w-full"
                    />
                    {#if shortcutErrors.copy}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.copy}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-slate-200 dark:border-slate-800 pb-4">
                  <div>
                    <span class="block text-sm font-medium text-slate-700 dark:text-slate-300">粘贴</span>
                    <span class="text-xs text-slate-500">从剪贴板粘贴文件</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      value={shortcutDrafts.paste}
                      on:input={(e) => handleShortcutInput('paste', (e.target as HTMLInputElement).value)}
                      class="bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 text-sm font-mono focus:border-blue-500 outline-none w-full"
                    />
                    {#if shortcutErrors.paste}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.paste}</div>
                    {/if}
                  </div>
                </div>
             </div>
             
             <div class="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-900/30 rounded-lg p-3 text-xs text-blue-800 dark:text-blue-200">
               <p>提示：快捷键格式为 "修饰键+按键"，例如 "Ctrl+Shift+P"。支持 Ctrl, Shift, Alt, Meta (Cmd)。</p>
             </div>
           </div>

        {:else if activeTab === 'terminal'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            <!-- Font Size -->
            <div>
              <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-2" for="fontSize">
                字体大小 ({$settings.terminal.fontSize}px)
              </label>
              <div class="flex items-center gap-4">
                <input
                  type="range"
                  id="fontSize"
                  min="10"
                  max="24"
                  step="1"
                  value={$settings.terminal.fontSize}
                  on:input={(e) => updateTerminalSetting('fontSize', Number((e.target as HTMLInputElement).value))}
                  class="flex-1 h-2 bg-slate-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer accent-blue-600"
                />
                <input 
                  type="number" 
                  value={$settings.terminal.fontSize}
                  min="10"
                  max="24"
                  step="1"
                  on:input={(e) => updateTerminalSetting('fontSize', Number((e.target as HTMLInputElement).value))}
                  class="w-16 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-2 py-1 text-center text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                />
              </div>
            </div>

            <!-- Font Family -->
            <div>
              <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-2" for="fontFamily">
                字体
              </label>
              <select
                id="fontFamily"
                value={$settings.terminal.fontFamily}
                on:change={(e) => updateTerminalSetting('fontFamily', (e.target as HTMLSelectElement).value)}
                class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
              >
                {#each fontFamilies as font}
                  <option value={font.value}>{font.label}</option>
                {/each}
              </select>
              <p class="mt-2 text-xs text-slate-500">
                当前字体预览: <span style="font-family: {$settings.terminal.fontFamily}">The quick brown fox jumps over the lazy dog 0123456789</span>
              </p>
            </div>

            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-slate-600 dark:text-slate-400" for="scrollback">
                  滚动行数
                </label>
                <p class="text-xs text-slate-500 mt-0.5">保留的历史输出行数</p>
              </div>
              <input
                type="number"
                id="scrollback"
                min="1000"
                max="50000"
                step="500"
                value={$settings.terminal.scrollback}
                on:input={(e) => updateTerminalScrollback((e.target as HTMLInputElement).value)}
                class="w-24 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-2 py-1 text-center text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
              />
            </div>

            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-slate-600 dark:text-slate-400" for="cursorStyle">
                  光标样式
                </label>
                <p class="text-xs text-slate-500 mt-0.5">设置光标的形状</p>
              </div>
              <select
                id="cursorStyle"
                value={$settings.terminal.cursorStyle}
                on:change={(e) => updateTerminalSetting('cursorStyle', (e.target as HTMLSelectElement).value as 'block' | 'underline' | 'bar')}
                class="bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
              >
                <option value="block">方块</option>
                <option value="underline">下划线</option>
                <option value="bar">竖线</option>
              </select>
            </div>

            <!-- Cursor Blink -->
            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-slate-600 dark:text-slate-400" for="cursorBlink">
                  光标闪烁
                </label>
                <p class="text-xs text-slate-500 mt-0.5">启用后光标将闪烁</p>
              </div>
              <label class="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  id="cursorBlink"
                  checked={$settings.terminal.cursorBlink}
                  on:change={(e) => updateTerminalSetting('cursorBlink', (e.target as HTMLInputElement).checked)}
                  class="sr-only peer"
                >
                <div class="w-11 h-6 bg-slate-200 dark:bg-slate-700 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-blue-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
              </label>
            </div>
          </div>

        {:else if activeTab === 'connection'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            <!-- Auto Reconnect -->
            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-slate-600 dark:text-slate-400" for="autoReconnect">
                  自动重连
                </label>
                <p class="text-xs text-slate-500 mt-0.5">意外断开连接时尝试自动重新连接</p>
              </div>
              <label class="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  id="autoReconnect"
                  checked={$settings.connection.autoReconnect}
                  on:change={(e) => updateConnectionSetting('autoReconnect', (e.target as HTMLInputElement).checked)}
                  class="sr-only peer"
                >
                <div class="w-11 h-6 bg-slate-200 dark:bg-slate-700 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-blue-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
              </label>
            </div>
          </div>

        {:else if activeTab === 'appearance'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            <!-- Theme Mode -->
            <div>
              <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">主题模式</span>
              <div class="grid grid-cols-2 gap-4">
                <button
                  class="relative p-4 border rounded-xl flex flex-col items-center gap-2 transition-all {$settings.theme === 'dark' ? 'border-blue-500 bg-blue-50 dark:bg-blue-500/10' : 'border-slate-200 dark:border-slate-700 hover:border-slate-300 dark:hover:border-slate-600'}"
                  on:click={() => updateTheme('dark')}
                >
                  <div class="w-full h-20 bg-slate-900 rounded-lg border border-slate-800 shadow-sm overflow-hidden relative">
                    <div class="absolute left-0 top-0 bottom-0 w-8 bg-slate-800 border-r border-slate-700"></div>
                    <div class="absolute right-2 top-2 w-12 h-2 bg-slate-700 rounded"></div>
                  </div>
                  <span class="text-sm font-medium text-slate-900 dark:text-slate-200">深色模式</span>
                  {#if $settings.theme === 'dark'}
                    <div class="absolute top-2 right-2 w-2 h-2 bg-blue-500 rounded-full"></div>
                  {/if}
                </button>

                <button
                  class="relative p-4 border rounded-xl flex flex-col items-center gap-2 transition-all {$settings.theme === 'light' ? 'border-blue-500 bg-blue-50 dark:bg-blue-500/10' : 'border-slate-200 dark:border-slate-700 hover:border-slate-300 dark:hover:border-slate-600'}"
                  on:click={() => updateTheme('light')}
                >
                  <div class="w-full h-20 bg-slate-100 rounded-lg border border-slate-200 shadow-sm overflow-hidden relative">
                    <div class="absolute left-0 top-0 bottom-0 w-8 bg-white border-r border-slate-200"></div>
                    <div class="absolute right-2 top-2 w-12 h-2 bg-slate-200 rounded"></div>
                  </div>
                  <span class="text-sm font-medium text-slate-900 dark:text-slate-200">浅色模式</span>
                  {#if $settings.theme === 'light'}
                    <div class="absolute top-2 right-2 w-2 h-2 bg-blue-500 rounded-full"></div>
                  {/if}
                </button>
              </div>
            </div>

            <!-- Terminal Theme -->
            <div>
              <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">终端主题</span>
              <div class="grid grid-cols-5 gap-2">
                {#each terminalThemes as theme}
                  <button
                    class="relative p-2 border rounded-lg flex flex-col items-center gap-2 transition-all {$settings.appearance.terminalTheme === theme.id ? 'border-blue-500 bg-blue-50 dark:bg-blue-500/10' : 'border-slate-200 dark:border-slate-700 hover:border-slate-300 dark:hover:border-slate-600'}"
                    on:click={() => updateTerminalTheme(theme.id)}
                    title={theme.name}
                  >
                    <div
                      class="w-full h-12 rounded shadow-sm"
                      style="background-color: {theme.preview.background}"
                    >
                      <div class="p-1 text-xs font-mono" style="color: {theme.preview.foreground}">
                        Aa
                      </div>
                    </div>
                    <span class="text-xs text-slate-700 dark:text-slate-300 truncate w-full text-center">{theme.name}</span>
                    {#if $settings.appearance.terminalTheme === theme.id}
                      <div class="absolute top-1 right-1 w-2 h-2 bg-blue-500 rounded-full"></div>
                    {/if}
                  </button>
                {/each}
              </div>
            </div>

            <!-- Custom Theme Editor -->
            {#if $settings.appearance.terminalTheme === 'custom'}
              <div class="border border-slate-200 dark:border-slate-700 rounded-lg p-4 bg-slate-50 dark:bg-slate-900/50">
                <div class="flex items-center justify-between mb-4">
                  <span class="text-sm font-medium text-slate-700 dark:text-slate-300">自定义主题</span>
                  <button
                    class="text-xs px-3 py-1 bg-blue-600 hover:bg-blue-500 text-white rounded transition-colors"
                    on:click={resetCustomTheme}
                  >
                    重置
                  </button>
                </div>

                <div class="grid grid-cols-2 gap-4">
                  <!-- Basic Colors -->
                  <div class="space-y-2">
                    <h4 class="text-xs font-medium text-slate-600 dark:text-slate-400 uppercase">基础颜色</h4>
                    {#each basicColorKeys as key}
                      {@const label = colorLabels[key]}
                      {@const value = $settings.appearance.customTheme?.[key] || '#000000'}
                      {@const inputId = `custom-color-${key}`}
                      <div class="flex items-center gap-2">
                        <label for={inputId} class="text-xs text-slate-600 dark:text-slate-400 w-20 shrink-0">{label}</label>
                        <input
                          id={inputId}
                          type="color"
                          value={value}
                          on:input={(e) => updateCustomColor(key, (e.target as HTMLInputElement).value)}
                          class="w-8 h-8 rounded cursor-pointer border border-slate-300 dark:border-slate-600"
                        />
                        <input
                          type="text"
                          value={value}
                          on:input={(e) => updateCustomColor(key, (e.target as HTMLInputElement).value)}
                          class="flex-1 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 py-1 text-xs font-mono text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                        />
                      </div>
                    {/each}
                  </div>

                  <!-- Standard Colors -->
                  <div class="space-y-2">
                    <h4 class="text-xs font-medium text-slate-600 dark:text-slate-400 uppercase">标准颜色</h4>
                    {#each standardColorKeys as key}
                      {@const label = colorLabels[key]}
                      {@const value = $settings.appearance.customTheme?.[key] || '#000000'}
                      {@const inputId = `custom-color-${key}`}
                      <div class="flex items-center gap-2">
                        <label for={inputId} class="text-xs text-slate-600 dark:text-slate-400 w-20 shrink-0">{label}</label>
                        <input
                          id={inputId}
                          type="color"
                          value={value}
                          on:input={(e) => updateCustomColor(key, (e.target as HTMLInputElement).value)}
                          class="w-8 h-8 rounded cursor-pointer border border-slate-300 dark:border-slate-600"
                        />
                        <input
                          type="text"
                          value={value}
                          on:input={(e) => updateCustomColor(key, (e.target as HTMLInputElement).value)}
                          class="flex-1 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 py-1 text-xs font-mono text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                        />
                      </div>
                    {/each}
                  </div>

                  <!-- Bright Colors -->
                  <div class="col-span-2 space-y-2">
                    <h4 class="text-xs font-medium text-slate-600 dark:text-slate-400 uppercase">亮色变体</h4>
                    <div class="grid grid-cols-4 gap-2">
                      {#each brightColorKeys as key}
                        {@const label = colorLabels[key]}
                        {@const value = $settings.appearance.customTheme?.[key] || '#000000'}
                        {@const inputId = `custom-color-${key}`}
                        <div class="flex items-center gap-1">
                          <input
                            id={inputId}
                            type="color"
                            value={value}
                            on:input={(e) => updateCustomColor(key, (e.target as HTMLInputElement).value)}
                            class="w-6 h-6 rounded cursor-pointer border border-slate-300 dark:border-slate-600"
                          />
                          <label for={inputId} class="text-xs text-slate-600 dark:text-slate-400 truncate">{label}</label>
                        </div>
                      {/each}
                    </div>
                  </div>
                </div>
              </div>
            {/if}
          </div>
        {:else if activeTab === 'security'}
           <div class="space-y-6" in:slide={{ duration: 200 }}>
             <h3 class="text-lg font-medium text-slate-800 dark:text-slate-200">应用安全锁</h3>
             <p class="text-sm text-slate-600 dark:text-slate-400">设置启动密码以保护您的连接信息。</p>
             
             {#if securityMessage}
               <div class="p-3 bg-green-50 dark:bg-green-500/10 border border-green-200 dark:border-green-500/20 text-green-600 dark:text-green-400 rounded-lg text-sm">
                 {securityMessage}
               </div>
             {/if}
             
             {#if securityError}
               <div class="p-3 bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/20 text-red-600 dark:text-red-400 rounded-lg text-sm">
                 {securityError}
               </div>
             {/if}

             {#if !hasLock}
               <!-- Setup Lock -->
               <div class="space-y-4 border border-slate-200 dark:border-slate-800 rounded-lg p-4 bg-slate-50 dark:bg-slate-950/30">
                 <h4 class="font-medium text-slate-700 dark:text-slate-300">设置新密码</h4>
                 <div>
                   <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1" for="new-pwd">新密码</label>
                   <input type="password" id="new-pwd" bind:value={newPassword} class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-white focus:border-blue-500 outline-none" />
                 </div>
                 <div>
                   <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1" for="confirm-pwd">确认密码</label>
                   <input type="password" id="confirm-pwd" bind:value={confirmPassword} class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-white focus:border-blue-500 outline-none" />
                 </div>
                 <button class="px-4 py-2 bg-blue-600 hover:bg-blue-500 text-white rounded-lg text-sm font-medium transition-colors" on:click={handleSetLock}>
                   启用应用锁
                 </button>
               </div>
             {:else}
               <!-- Auto Lock Settings -->
               <div class="space-y-4 border border-slate-200 dark:border-slate-800 rounded-lg p-4 bg-slate-50 dark:bg-slate-950/30">
                 <h4 class="font-medium text-slate-700 dark:text-slate-300">自动锁定</h4>
                 
                 <div class="flex items-center justify-between">
                    <div>
                      <label class="block text-sm font-medium text-slate-600 dark:text-slate-400" for="autoLockTime">
                        自动锁定时间
                      </label>
                      <p class="text-xs text-slate-500 mt-0.5">无操作指定时间后自动锁定 (0 为禁用)</p>
                    </div>
                    <div class="flex items-center gap-2">
                      <input 
                        type="number" 
                        id="autoLockTime"
                        min="0"
                        max="120"
                        value={$settings.security.autoLockMinutes}
                        on:input={(e) => updateSecuritySetting('autoLockMinutes', Number((e.target as HTMLInputElement).value))}
                        class="w-16 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-2 py-1 text-center text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                      />
                      <span class="text-sm text-slate-500">分钟</span>
                   </div>
                </div>

                <div class="flex items-center justify-between border-t border-slate-200 dark:border-slate-800 pt-4">
                   <div>
                     <label class="block text-sm font-medium text-slate-600 dark:text-slate-400" for="lockOnBlur">
                       失去焦点时锁定
                     </label>
                     <p class="text-xs text-slate-500 mt-0.5">当切换到其他应用窗口时自动锁定</p>
                   </div>
                   <label class="relative inline-flex items-center cursor-pointer">
                     <input
                       type="checkbox"
                       id="lockOnBlur"
                       checked={$settings.security.lockOnBlur}
                       on:change={(e) => updateSecuritySetting('lockOnBlur', (e.target as HTMLInputElement).checked)}
                       class="sr-only peer"
                     >
                     <div class="w-11 h-6 bg-slate-200 dark:bg-slate-700 peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-blue-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-blue-600"></div>
                   </label>
                </div>
              </div>

              <!-- Change/Remove Lock -->
              <div class="space-y-4 border border-slate-200 dark:border-slate-800 rounded-lg p-4 bg-slate-50 dark:bg-slate-950/30">
                <h4 class="font-medium text-slate-700 dark:text-slate-300">管理密码</h4>
                <div>
                  <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1" for="curr-pwd">当前密码</label>
                  <input type="password" id="curr-pwd" bind:value={oldPassword} class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-white focus:border-blue-500 outline-none" />
                </div>
                
                <div class="pt-4 border-t border-slate-200 dark:border-slate-800">
                   <h5 class="text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">修改密码（可选）</h5>
                   <div class="space-y-3">
                       <div>
                       <label class="block text-sm font-medium text-slate-500 mb-1" for="new-pwd-change">新密码</label>
                       <input type="password" id="new-pwd-change" bind:value={newPassword} class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-white focus:border-blue-500 outline-none" />
                       </div>
                       <div>
                       <label class="block text-sm font-medium text-slate-500 mb-1" for="confirm-pwd-change">确认新密码</label>
                       <input type="password" id="confirm-pwd-change" bind:value={confirmPassword} class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-white focus:border-blue-500 outline-none" />
                       </div>
                       <div class="flex gap-3 pt-2">
                           <button class="px-4 py-2 bg-slate-200 dark:bg-slate-700 hover:bg-slate-300 dark:hover:bg-slate-600 text-slate-900 dark:text-white rounded-lg text-sm font-medium transition-colors" on:click={handleChangeLock}>
                           更新密码
                           </button>
                           <button class="px-4 py-2 bg-red-50 dark:bg-red-900/30 hover:bg-red-100 dark:hover:bg-red-900/50 text-red-600 dark:text-red-400 border border-red-200 dark:border-red-900/50 rounded-lg text-sm font-medium transition-colors" on:click={handleRemoveLock}>
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
