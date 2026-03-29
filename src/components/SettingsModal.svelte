<script lang="ts">
  // Force rebuild.
  import { showSettings, settings, isLocked, type AppSettings } from '../lib/store';
  import { themeColors } from '../lib/themeColors';
  import XIcon from './icons/XIcon.svelte';
  import EyeDropperIcon from './icons/EyeDropperIcon.svelte';
  import UploadIcon from './icons/UploadIcon.svelte';
  import TrashIcon from './icons/TrashIcon.svelte';
  import { slide } from 'svelte/transition';
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { applyScrollbarColor } from '../lib/terminalService';
  import { getShortcutFromKeyboardEvent, normalizeShortcut } from '../lib/shortcuts';

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
  let ansiPresetDropdownOpen = false;
  let ansiPresetDropdownEl: HTMLDivElement | null = null;
  let terminalThemeDropdownOpen = false;
  let terminalThemeDropdownEl: HTMLDivElement | null = null;

  type ShortcutKey = keyof AppSettings['shortcuts'];

  let shortcutDrafts: AppSettings['shortcuts'] = { ...$settings.shortcuts };
  let shortcutErrors: Partial<Record<ShortcutKey, string>> = {};
  let shortcutSnapshot = '';

  $: {
    const nextSnapshot = JSON.stringify($settings.shortcuts);
    if (nextSnapshot !== shortcutSnapshot) {
      shortcutSnapshot = nextSnapshot;
      shortcutDrafts = { ...$settings.shortcuts };
    }
  }

  const shortcutLabels: Record<ShortcutKey, string> = {
    commandPalette: '命令面板',
    toggleSidebar: '切换侧边栏',
    toggleFileBrowser: '切换文件浏览器面板',
    newConnection: '新建连接',
    settings: '设置',
    closeTerminal: '关闭终端',
    prevTab: '上一个标签页',
    nextTab: '下一个标签页',
    copy: '复制',
    paste: '粘贴',
    fileBrowserRefresh: '文件浏览器：刷新',
    fileBrowserNewFolder: '文件浏览器：新建文件夹',
    fileBrowserNewFile: '文件浏览器：新建文件',
    fileBrowserRename: '文件浏览器：重命名',
    fileBrowserDelete: '文件浏览器：删除',
    fileBrowserSelectAll: '文件浏览器：全选',
    fileBrowserOpen: '文件浏览器：打开/进入',
    fileBrowserBack: '文件浏览器：返回上一级'
  };

  onMount(() => {
    checkLockStatus();

    const handlePointerDown = (event: MouseEvent) => {
      if (ansiPresetDropdownOpen && ansiPresetDropdownEl && !ansiPresetDropdownEl.contains(event.target as Node)) {
        ansiPresetDropdownOpen = false;
      }
      if (terminalThemeDropdownOpen && terminalThemeDropdownEl && !terminalThemeDropdownEl.contains(event.target as Node)) {
        terminalThemeDropdownOpen = false;
      }
    };

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        ansiPresetDropdownOpen = false;
        terminalThemeDropdownOpen = false;
      }
    };

    window.addEventListener('mousedown', handlePointerDown, true);
    window.addEventListener('keydown', handleEscape);

    return () => {
      window.removeEventListener('mousedown', handlePointerDown, true);
      window.removeEventListener('keydown', handleEscape);
    };
  });

  function getAutoLinkedAnsiPreset(
    terminalTheme: AppSettings['appearance']['terminalTheme'],
    appTheme: AppSettings['theme']
  ): AppSettings['appearance']['ansiColorPreset'] {
    if (terminalTheme === 'custom') {
      return 'custom';
    }

    if (terminalTheme === 'auto') {
      return appTheme === 'light' ? 'standard-light' : 'classic';
    }

    return terminalTheme === 'solarized-light' ? 'standard-light' : 'classic';
  }

  function updateTheme(theme: AppSettings['theme']) {
    settings.update(s => {
      let newSettings = { ...s, theme };
      // Initialize customUITheme if selecting custom for the first time
      if (theme === 'custom' && !s.appearance.customUITheme) {
        newSettings.appearance = {
          ...s.appearance,
          customUITheme: { ...defaultCustomUITheme }
        };
      }

      if (s.appearance.terminalTheme === 'auto' && s.appearance.ansiColorPreset !== 'custom') {
        newSettings.appearance = {
          ...newSettings.appearance,
          ansiColorPreset: getAutoLinkedAnsiPreset('auto', theme)
        };
      }

      return newSettings;
    });
  }

  function updateTerminalTheme(theme: 'auto' | 'dracula' | 'nord' | 'solarized-dark' | 'solarized-light' | 'monokai' | 'one-dark' | 'github-dark' | 'tokyo-night' | 'catppuccin' | 'custom') {
    let nextSettings: AppSettings | null = null;

    settings.update(s => {
      const nextAnsiPreset = getAutoLinkedAnsiPreset(theme, s.theme);
      nextSettings = {
        ...s,
        appearance: {
          ...s.appearance,
          terminalTheme: theme,
          ansiColorPreset: nextAnsiPreset,
          customTheme: theme === 'custom' ? s.appearance.customTheme || defaultCustomTheme : undefined,
          customAnsiColors: nextAnsiPreset === 'custom'
            ? s.appearance.customAnsiColors || { ...defaultCustomAnsiColors }
            : undefined
        }
      };

      return nextSettings;
    });

    // Update scrollbar colors immediately after theme change
    if (nextSettings) {
      applyScrollbarColor(nextSettings);
    }
  }

  function updateAccentColorSetting(color: string) {
    settings.update(s => ({
      ...s,
      appearance: {
        ...s.appearance,
        accentColor: color
      }
    }));
  }

  function updateAppearanceSetting<K extends keyof AppSettings['appearance']>(
    key: K,
    value: AppSettings['appearance'][K]
  ) {
    settings.update(s => ({
      ...s,
      appearance: {
        ...s.appearance,
        [key]: value
      }
    }));
  }

  function handleBackgroundImageUpload(e: Event) {
    const input = e.target as HTMLInputElement;
    if (input.files && input.files[0]) {
      const file = input.files[0];
      const reader = new FileReader();
      reader.onload = (e) => {
        const result = e.target?.result;
        if (typeof result === 'string') {
          updateAppearanceSetting('backgroundImage', result);
        }
      };
      reader.readAsDataURL(file);
    }
    // Clear input so same file can be selected again
    input.value = '';
  }

  function clearBackgroundImage() {
    updateAppearanceSetting('backgroundImage', null);
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

  function updateConnectionSetting<K extends keyof AppSettings['connection']>(
    key: K,
    value: AppSettings['connection'][K]
  ) {
    settings.update(s => ({
      ...s,
      connection: {
        ...s.connection,
        [key]: value
      }
    }));
  }

  function updateShortcutSetting<K extends keyof AppSettings['shortcuts']>(
    key: K,
    value: AppSettings['shortcuts'][K]
  ) {
    settings.update(s => ({
      ...s,
      shortcuts: {
        ...s.shortcuts,
        [key]: value
      }
    }));
  }

  function updateSecuritySetting<K extends keyof AppSettings['security']>(
    key: K,
    value: AppSettings['security'][K]
  ) {
    settings.update(s => ({
      ...s,
      security: {
        ...s.security,
        [key]: value
      }
    }));
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

  function handleShortcutKeydown(key: ShortcutKey, event: KeyboardEvent) {
    if (event.key === 'Tab') return;

    event.preventDefault();
    event.stopPropagation();

    if (event.key === 'Escape' && !event.ctrlKey && !event.metaKey && !event.altKey && !event.shiftKey) {
      const nextErrors = { ...shortcutErrors };
      delete nextErrors[key];
      shortcutErrors = nextErrors;
      shortcutDrafts = { ...shortcutDrafts, [key]: '' };
      updateShortcutSetting(key, '');
      return;
    }

    const captured = getShortcutFromKeyboardEvent(event);
    if (!captured) return;

    handleShortcutInput(key, captured);
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
      isLocked.set(true);
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
     
     try {
       if (newPassword !== confirmPassword) {
         securityError = '两次输入的新密码不一致';
         return;
       }

       if (!newPassword) {
         securityError = '新密码不能为空';
         return;
       }
       
       await invoke('change_app_lock', { currentPassword: oldPassword, newPassword });
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
      await invoke('remove_app_lock', { currentPassword: oldPassword });
      hasLock = false;
      isLocked.set(false);
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
    { label: 'Cascadia Code', value: '"Cascadia Code", "Cascadia Mono", monospace' },
    { label: 'Consolas', value: 'Consolas, "Courier New", monospace' },
    { label: 'Fira Code', value: '"Fira Code", monospace' },
    { label: 'IBM Plex Mono', value: '"IBM Plex Mono", monospace' },
    { label: 'JetBrains Mono', value: '"JetBrains Mono", monospace' },
    { label: '宋体', value: 'SimSun, "Songti SC", serif' },
    { label: '仿宋', value: 'FangSong, STFangsong, serif' },
    { label: '楷体', value: 'KaiTi, STKaiti, serif' },
    { label: '微软雅黑', value: '"Microsoft YaHei", "PingFang SC", sans-serif' },
    { label: 'SF Mono', value: '"SF Mono", Menlo, Monaco, monospace' },
    { label: 'Source Code Pro', value: '"Source Code Pro", monospace' },
    { label: 'Ubuntu Mono', value: '"Ubuntu Mono", monospace' },
    { label: 'DejaVu Sans Mono', value: '"DejaVu Sans Mono", monospace' }
  ];

  function isPresetFontFamily(value: string): boolean {
    return fontFamilies.some((font) => font.value === value);
  }

  function getSelectedFontFamilyOption(value: string): string {
    return isPresetFontFamily(value) ? value : '__custom__';
  }

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
    white: '#f8fafc',
    brightBlack: '#666666',
    brightRed: '#f14c4c',
    brightGreen: '#23d18b',
    brightYellow: '#f5f543',
    brightBlue: '#3b8eea',
    brightMagenta: '#d670d6',
    brightCyan: '#29b8db',
    brightWhite: '#ffffff',
    cursorAccent: '#000000',
    selectionForeground: '#ffffff',
    selectionInactiveBackground: '#3a3d41',
    scrollbarSliderBackground: '#79797966',
    scrollbarSliderHoverBackground: '#646464bb',
    scrollbarSliderActiveBackground: '#bfbfbf66',
    overviewRulerBorder: '#7f7f7f',
    extendedAnsi: []
  };

  // Color key labels
  type CustomThemeKey = Exclude<keyof NonNullable<AppSettings['appearance']['customTheme']>, 'extendedAnsi'>;

  const colorLabels: Record<CustomThemeKey, string> = {
    background: '背景',
    foreground: '前景',
    cursor: '光标',
    cursorAccent: '光标文字',
    selectionBackground: '选中背景',
    selectionForeground: '选中文字',
    selectionInactiveBackground: '非激活选中',
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
  type EyeDropperResult = { sRGBHex: string };
  type EyeDropperCtor = new () => { open: () => Promise<EyeDropperResult> };

  async function handleEyeDropper(updateFn: (color: string) => void) {
     if (!('EyeDropper' in window)) return;
     try {
       const eyeDropper = new (window as Window & { EyeDropper?: EyeDropperCtor }).EyeDropper!();
       const result = await eyeDropper.open();
       updateFn(result.sRGBHex);
     } catch (e) {
       // ignore
     }
   }

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
  // Custom UI Theme logic
  const defaultCustomUITheme = {
    backgroundColor: '#0f172a',
    surfaceColor: '#1e293b',
    statusBarColor: '#1e293b',
    surfaceLightColor: '#334155',
    textColor: '#f8fafc',
    secondaryTextColor: '#94a3b8',
    borderColor: '#475569',
    borderLightColor: '#64748b'
  };

  type CustomUIThemeKey = keyof typeof defaultCustomUITheme;

  const customUIColorLabels: Record<CustomUIThemeKey, string> = {
    backgroundColor: '背景颜色',
    surfaceColor: '表面颜色',
    statusBarColor: '下边栏颜色',
    surfaceLightColor: '亮表面色',
    textColor: '主要文字',
    secondaryTextColor: '次要文字',
    borderColor: '边框颜色',
    borderLightColor: '亮边框色'
  };

  function updateCustomUITheme(key: CustomUIThemeKey, value: string) {
    if (!$settings.appearance.customUITheme) {
      settings.update(s => ({
        ...s,
        appearance: {
          ...s.appearance,
          customUITheme: { ...defaultCustomUITheme, [key]: value }
        }
      }));
    } else {
      settings.update(s => ({
        ...s,
        appearance: {
          ...s.appearance,
          customUITheme: {
            ...s.appearance.customUITheme!,
            [key]: value
          }
        }
      }));
    }
  }

  // ANSI Color Presets
  type AnsiColorPreset = AppSettings['appearance']['ansiColorPreset'];

  const defaultCustomAnsiColors = {
    foreground: '#f8fafc',
    red: '#ff4444',
    green: '#44ff44',
    yellow: '#ffff44',
    blue: '#4444ff',
    magenta: '#ff44ff',
  };

  const ansiColorPresets: Array<{
    id: AnsiColorPreset;
    name: string;
    description: string;
    previewColors: {
      black: string;
      red: string;
      green: string;
      yellow: string;
      blue: string;
      magenta: string;
      cyan: string;
      white: string;
    };
  }> = [
    {
      id: 'classic',
      name: '经典ANSI',
      description: '标准的16色终端配色',
      previewColors: {
        black: '#000000',
        red: '#cd0000',
        green: '#00cd00',
        yellow: '#cdcd00',
        blue: '#0000ee',
        magenta: '#cd00cd',
        cyan: '#00cdcd',
        white: '#f8fafc',
      }
    },
    {
      id: 'standard-light',
      name: '标准浅色',
      description: '专为浅色背景优化的配色',
      previewColors: {
        black: '#000000',
        red: '#cd3131',
        green: '#0dbc79',
        yellow: '#949800',
        blue: '#2472c8',
        magenta: '#bc3fbc',
        cyan: '#11a8cd',
        white: '#555555',
      }
    },
    {
      id: 'solarized',
      name: 'Solarized Dark',
      description: '精确设计的深色配色方案',
      previewColors: {
        black: '#073642',
        red: '#dc322f',
        green: '#859900',
        yellow: '#b58900',
        blue: '#268bd2',
        magenta: '#d33682',
        cyan: '#2aa198',
        white: '#eee8d5',
      }
    },
    {
      id: 'solarized-light',
      name: 'Solarized Light',
      description: '精确设计的浅色配色方案',
      previewColors: {
        black: '#073642',
        red: '#dc322f',
        green: '#859900',
        yellow: '#b58900',
        blue: '#268bd2',
        magenta: '#d33682',
        cyan: '#2aa198',
        white: '#eee8d5',
      }
    },
    {
      id: 'github-light',
      name: 'GitHub Light',
      description: 'GitHub 风格的浅色主题',
      previewColors: {
        black: '#24292f',
        red: '#cf222e',
        green: '#1a7f37',
        yellow: '#9a6700',
        blue: '#0969da',
        magenta: '#8250df',
        cyan: '#1b7c83',
        white: '#6e7781',
      }
    },
    {
      id: 'monokai',
      name: 'Monokai',
      description: '流行的深色主题,高对比度',
      previewColors: {
        black: '#1e1f1c',
        red: '#f92672',
        green: '#a6e22e',
        yellow: '#f4bf75',
        blue: '#66d9ef',
        magenta: '#ae81ff',
        cyan: '#a1efe4',
        white: '#f8f8f2',
      }
    },
    {
      id: 'gruvbox',
      name: 'Gruvbox',
      description: '复古风格,温暖舒适',
      previewColors: {
        black: '#282828',
        red: '#cc241d',
        green: '#98971a',
        yellow: '#d79921',
        blue: '#458588',
        magenta: '#b16286',
        cyan: '#689d6a',
        white: '#a89984',
      }
    },
    {
      id: 'dracula',
      name: 'Dracula',
      description: '非常常见的紫调深色终端配色',
      previewColors: {
        black: '#21222c',
        red: '#ff5555',
        green: '#50fa7b',
        yellow: '#f1fa8c',
        blue: '#bd93f9',
        magenta: '#ff79c6',
        cyan: '#8be9fd',
        white: '#f8f8f2',
      }
    },
    {
      id: 'one-dark',
      name: 'One Dark',
      description: 'Atom 与 VS Code 用户常见的深色配色',
      previewColors: {
        black: '#282c34',
        red: '#e06c75',
        green: '#98c379',
        yellow: '#e5c07b',
        blue: '#61afef',
        magenta: '#c678dd',
        cyan: '#56b6c2',
        white: '#dcdfe4',
      }
    },
    {
      id: 'tokyo-night',
      name: 'Tokyo Night',
      description: '近年很常用的蓝紫调深色配色',
      previewColors: {
        black: '#15161e',
        red: '#f7768e',
        green: '#9ece6a',
        yellow: '#e0af68',
        blue: '#7aa2f7',
        magenta: '#bb9af7',
        cyan: '#7dcfff',
        white: '#a9b1d6',
      }
    },
    {
      id: 'custom',
      name: '自定义',
      description: '完全自定义所有ANSI颜色',
      previewColors: {
        black: '#000000',
        red: '#cd0000',
        green: '#00cd00',
        yellow: '#cdcd00',
        blue: '#0000ee',
        magenta: '#cd00cd',
        cyan: '#00cdcd',
        white: '#f8fafc',
      }
    }
  ];

  type AnsiColorKey = keyof NonNullable<AppSettings['appearance']['customAnsiColors']>;

  const ansiColorLabels: Record<AnsiColorKey, string> = {
    foreground: '前景色',
    red: '红色',
    green: '绿色',
    yellow: '黄色',
    blue: '蓝色',
    magenta: '品红',
  };

  function updateAnsiColorPreset(preset: AnsiColorPreset) {
    settings.update(s => ({
      ...s,
      appearance: {
        ...s.appearance,
        ansiColorPreset: preset,
        customAnsiColors: preset === 'custom' ? s.appearance.customAnsiColors || { ...defaultCustomAnsiColors } : undefined
      }
    }));
  }

  function getAnsiColorPresetById(presetId: AnsiColorPreset) {
    return ansiColorPresets.find((preset) => preset.id === presetId);
  }

  function isLightAnsiPreset(presetId: AnsiColorPreset): boolean {
    return ['standard-light', 'solarized-light', 'github-light'].includes(presetId);
  }

  function selectAnsiPreset(preset: AnsiColorPreset) {
    updateAnsiColorPreset(preset);
    ansiPresetDropdownOpen = false;
  }

  function selectTerminalTheme(theme: TerminalThemePreset) {
    updateTerminalTheme(theme);
    terminalThemeDropdownOpen = false;
  }

  $: selectedAnsiPreset = getAnsiColorPresetById($settings.appearance.ansiColorPreset);
  $: selectedTerminalTheme = terminalThemes.find((theme) => theme.id === $settings.appearance.terminalTheme);

  function updateCustomAnsiColor(key: AnsiColorKey, value: string) {
    if (!$settings.appearance.customAnsiColors) {
      settings.update(s => ({
        ...s,
        appearance: {
          ...s.appearance,
          customAnsiColors: { ...defaultCustomAnsiColors }
        }
      }));
      return;
    }
    settings.update(s => ({
      ...s,
      appearance: {
        ...s.appearance,
        customAnsiColors: {
          ...s.appearance.customAnsiColors!,
          [key]: value
        }
      }
    }));
  }

  function resetCustomAnsiColors() {
    settings.update(s => ({
      ...s,
      appearance: {
        ...s.appearance,
        customAnsiColors: { ...defaultCustomAnsiColors }
      }
    }));
  }

  function resetCustomUITheme() {
    settings.update(s => ({
      ...s,
      appearance: {
        ...s.appearance,
        customUITheme: { ...defaultCustomUITheme }
      }
    }));
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4" role="button" tabindex="0" on:click|self={handleClose} on:keydown={(e) => e.key === 'Escape' && handleClose()}>
  <div class="bg-app-surface border border-app-border rounded-xl shadow-2xl w-full max-w-3xl h-[600px] flex overflow-hidden text-app-text">
    
    <!-- Sidebar -->
    <div class="w-48 border-r border-app-border bg-app-surface-light p-4 flex flex-col gap-1">
      <h2 class="text-lg font-semibold text-app-text px-3 py-2 mb-2">设置</h2>
      
      {#each tabs as tab}
        <button
          class="text-left px-3 py-2 rounded-lg text-sm font-medium transition-colors {activeTab === tab.id ? 'bg-primary-600 text-white' : 'text-app-text-secondary hover:text-app-text hover:bg-app-bg'}"
          on:click={() => activeTab = tab.id}
        >
          {tab.label}
        </button>
      {/each}
    </div>

    <!-- Content -->
    <div class="flex-1 flex flex-col min-w-0 bg-app-surface">
      <!-- Header -->
      <div class="flex items-center justify-between px-6 py-4 border-b border-app-border">
        <h3 class="text-base font-medium text-app-text">
          {tabs.find(t => t.id === activeTab)?.label}
        </h3>
        <button 
          class="text-app-text-secondary hover:text-app-text transition-colors p-1 rounded-md hover:bg-app-bg"
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
              <label class="block text-sm font-medium text-app-text-secondary mb-2" for="theme">
                主题
              </label>
              <select
                id="theme"
                value={$settings.theme}
                on:change={(e) => updateTheme(e.currentTarget.value as any)}
                class="settings-select w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none"
              >
                <option value="system">跟随系统</option>
                <option value="dark">深色</option>
                <option value="light">浅色</option>
                <option value="custom">自定义</option>
              </select>
            </div>

            <!-- Language -->
            <div>
              <label class="block text-sm font-medium text-app-text-secondary mb-2" for="language">
                语言
              </label>
              <select
                id="language"
                class="settings-select w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none"
                disabled
              >
                <option value="zh-CN">简体中文</option>
                <option value="en-US" disabled>English (Coming Soon)</option>
              </select>
              <p class="mt-2 text-xs text-app-text-secondary">语言设置暂未开放</p>
            </div>

            <!-- UI -->
            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-app-text-secondary" for="sidebarCollapsed">
                  折叠侧边栏
                </label>
                <p class="text-xs text-app-text-secondary mt-0.5">在窗口左侧显示紧凑模式</p>
              </div>
              <label class="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  id="sidebarCollapsed"
                  checked={$settings.ui.sidebarCollapsed}
                  on:change={(e) => updateUiSetting('sidebarCollapsed', (e.target as HTMLInputElement).checked)}
                  class="sr-only peer"
                >
                <div class="w-11 h-6 bg-app-surface-light peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-primary-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary-600"></div>
              </label>
            </div>

            <!-- App Info -->
            <div class="pt-6 border-t border-app-border">
               <div class="flex justify-between items-center">
                 <div>
                   <h4 class="text-sm font-medium text-app-text">关于 Star Shuttle</h4>
                   <p class="text-xs text-app-text-secondary mt-1">Version 0.1.0</p>
                 </div>
                 <button class="text-xs text-primary-500 hover:text-primary-400 transition-colors">
                   检查更新
                 </button>
               </div>
            </div>
          </div>

        {:else if activeTab === 'shortcuts'}
           <div class="space-y-6" in:slide={{ duration: 200 }}>
             <h3 class="text-lg font-medium text-app-text">快捷键设置</h3>
             
             <div class="space-y-4">
                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">命令面板</span>
                    <span class="text-xs text-app-text-secondary">快速访问所有命令</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.commandPalette}
                      on:keydown={(e) => handleShortcutKeydown('commandPalette', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.commandPalette}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.commandPalette}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">切换侧边栏</span>
                    <span class="text-xs text-app-text-secondary">显示/隐藏左侧侧边栏</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.toggleSidebar}
                      on:keydown={(e) => handleShortcutKeydown('toggleSidebar', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.toggleSidebar}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.toggleSidebar}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">切换文件浏览器面板</span>
                    <span class="text-xs text-app-text-secondary">显示/隐藏右侧文件浏览器面板</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.toggleFileBrowser}
                      on:keydown={(e) => handleShortcutKeydown('toggleFileBrowser', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.toggleFileBrowser}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.toggleFileBrowser}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">新建连接</span>
                    <span class="text-xs text-app-text-secondary">打开新建连接窗口</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.newConnection}
                      on:keydown={(e) => handleShortcutKeydown('newConnection', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.newConnection}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.newConnection}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">设置</span>
                    <span class="text-xs text-app-text-secondary">打开设置窗口</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.settings}
                      on:keydown={(e) => handleShortcutKeydown('settings', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.settings}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.settings}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">关闭终端</span>
                    <span class="text-xs text-app-text-secondary">关闭当前活动的终端会话</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.closeTerminal}
                      on:keydown={(e) => handleShortcutKeydown('closeTerminal', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.closeTerminal}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.closeTerminal}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">上一个标签页</span>
                    <span class="text-xs text-app-text-secondary">切换到左侧终端标签</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.prevTab}
                      on:keydown={(e) => handleShortcutKeydown('prevTab', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.prevTab}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.prevTab}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">下一个标签页</span>
                    <span class="text-xs text-app-text-secondary">切换到右侧终端标签</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.nextTab}
                      on:keydown={(e) => handleShortcutKeydown('nextTab', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.nextTab}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.nextTab}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">复制</span>
                    <span class="text-xs text-app-text-secondary">复制选中文件到剪贴板</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.copy}
                      on:keydown={(e) => handleShortcutKeydown('copy', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.copy}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.copy}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">粘贴</span>
                    <span class="text-xs text-app-text-secondary">从剪贴板粘贴文件</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.paste}
                      on:keydown={(e) => handleShortcutKeydown('paste', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.paste}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.paste}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">文件浏览器：刷新</span>
                    <span class="text-xs text-app-text-secondary">刷新当前目录</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.fileBrowserRefresh}
                      on:keydown={(e) => handleShortcutKeydown('fileBrowserRefresh', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.fileBrowserRefresh}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.fileBrowserRefresh}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">文件浏览器：新建文件夹</span>
                    <span class="text-xs text-app-text-secondary">在当前目录新建文件夹</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.fileBrowserNewFolder}
                      on:keydown={(e) => handleShortcutKeydown('fileBrowserNewFolder', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.fileBrowserNewFolder}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.fileBrowserNewFolder}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">文件浏览器：新建文件</span>
                    <span class="text-xs text-app-text-secondary">在当前目录新建文件</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.fileBrowserNewFile}
                      on:keydown={(e) => handleShortcutKeydown('fileBrowserNewFile', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.fileBrowserNewFile}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.fileBrowserNewFile}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">文件浏览器：重命名</span>
                    <span class="text-xs text-app-text-secondary">重命名选中的文件或文件夹</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.fileBrowserRename}
                      on:keydown={(e) => handleShortcutKeydown('fileBrowserRename', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.fileBrowserRename}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.fileBrowserRename}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">文件浏览器：删除</span>
                    <span class="text-xs text-app-text-secondary">删除选中的文件或文件夹</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.fileBrowserDelete}
                      on:keydown={(e) => handleShortcutKeydown('fileBrowserDelete', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.fileBrowserDelete}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.fileBrowserDelete}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">文件浏览器：全选</span>
                    <span class="text-xs text-app-text-secondary">选中所有文件和文件夹</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.fileBrowserSelectAll}
                      on:keydown={(e) => handleShortcutKeydown('fileBrowserSelectAll', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.fileBrowserSelectAll}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.fileBrowserSelectAll}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">文件浏览器：打开/进入</span>
                    <span class="text-xs text-app-text-secondary">进入文件夹或打开文件</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.fileBrowserOpen}
                      on:keydown={(e) => handleShortcutKeydown('fileBrowserOpen', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.fileBrowserOpen}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.fileBrowserOpen}</div>
                    {/if}
                  </div>
                </div>

                <div class="grid grid-cols-2 gap-4 items-center border-b border-app-border pb-4">
                  <div>
                    <span class="block text-sm font-medium text-app-text-secondary">文件浏览器：返回上一级</span>
                    <span class="text-xs text-app-text-secondary">返回上级目录</span>
                  </div>
                  <div class="space-y-1">
                    <input
                      type="text"
                      readonly
                      value={shortcutDrafts.fileBrowserBack}
                      on:keydown={(e) => handleShortcutKeydown('fileBrowserBack', e)}
                      class="bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text text-sm font-mono focus:border-primary-500 outline-none w-full"
                    />
                    {#if shortcutErrors.fileBrowserBack}
                      <div class="text-xs text-red-500 dark:text-red-400">{shortcutErrors.fileBrowserBack}</div>
                    {/if}
                  </div>
                </div>
              </div>
              
              <div class="bg-app-surface/80 border border-app-border rounded-lg p-3 text-xs text-app-text-secondary backdrop-blur-sm">
               <p>提示：点击输入框后直接按下组合键即可录制，按 `Esc` 可清空。支持 Ctrl、Shift、Alt、Meta (Cmd) 和 F 键、Enter、Delete 等特殊键。</p>
             </div>
           </div>

        {:else if activeTab === 'terminal'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            <!-- Font Size -->
            <div>
              <label class="block text-sm font-medium text-app-text-secondary mb-2" for="fontSize">
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
                  class="flex-1 h-2 bg-app-border rounded-lg appearance-none cursor-pointer accent-primary-600"
                />
                <input 
                  type="number" 
                  value={$settings.terminal.fontSize}
                  min="10"
                  max="24"
                  step="1"
                  on:input={(e) => updateTerminalSetting('fontSize', Number((e.target as HTMLInputElement).value))}
                  class="w-16 bg-app-bg border border-app-border rounded-lg px-2 py-1 text-center text-app-text focus:border-primary-500 outline-none"
                />
              </div>
            </div>

            <!-- Font Family -->
            <div>
              <label class="block text-sm font-medium text-app-text-secondary mb-2" for="fontFamily">
                字体
              </label>
              <select
                id="fontFamily"
                value={getSelectedFontFamilyOption($settings.terminal.fontFamily)}
                on:change={(e) => {
                  const value = (e.target as HTMLSelectElement).value;
                  if (value !== '__custom__') {
                    updateTerminalSetting('fontFamily', value);
                  }
                }}
                class="settings-select w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none"
              >
                {#each fontFamilies as font}
                  <option value={font.value}>{font.label}</option>
                {/each}
                <option value="__custom__">自定义字体</option>
              </select>
              <div class="mt-3">
                <label class="block text-xs text-app-text-secondary mb-1.5" for="customFontFamily">
                  自定义字体
                </label>
                <input
                  id="customFontFamily"
                  type="text"
                  value={$settings.terminal.fontFamily}
                  placeholder={'例如: "Maple Mono", monospace'}
                  on:input={(e) => updateTerminalSetting('fontFamily', (e.target as HTMLInputElement).value)}
                  class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text font-mono focus:border-primary-500 outline-none"
                />
                <p class="mt-1.5 text-xs text-app-text-secondary">
                  可输入系统已安装的字体名，支持字体回退写法，例如 `"Maple Mono", "Microsoft YaHei", monospace`
                </p>
              </div>
              <p class="mt-2 text-xs text-app-text-secondary">
                当前字体预览: <span style="font-family: {$settings.terminal.fontFamily}">The quick brown fox jumps over the lazy dog 0123456789</span>
              </p>
            </div>

            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-app-text-secondary" for="scrollback">
                  滚动行数
                </label>
                <p class="text-xs text-app-text-secondary mt-0.5">保留的历史输出行数</p>
              </div>
              <input
                type="number"
                id="scrollback"
                min="1000"
                max="50000"
                step="500"
                value={$settings.terminal.scrollback}
                on:input={(e) => updateTerminalScrollback((e.target as HTMLInputElement).value)}
                class="w-24 bg-app-bg border border-app-border rounded-lg px-2 py-1 text-center text-app-text focus:border-primary-500 outline-none"
              />
            </div>

            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-app-text-secondary" for="cursorStyle">
                  光标样式
                </label>
                <p class="text-xs text-app-text-secondary mt-0.5">设置光标的形状</p>
              </div>
              <select
                id="cursorStyle"
                value={$settings.terminal.cursorStyle}
                on:change={(e) => updateTerminalSetting('cursorStyle', (e.target as HTMLSelectElement).value as 'block' | 'underline' | 'bar')}
                class="settings-select bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none"
              >
                <option value="block">方块</option>
                <option value="underline">下划线</option>
                <option value="bar">竖线</option>
              </select>
            </div>

            <!-- Cursor Blink -->
            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-app-text-secondary" for="cursorBlink">
                  光标闪烁
                </label>
                <p class="text-xs text-app-text-secondary mt-0.5">启用后光标将闪烁</p>
              </div>
              <label class="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  id="cursorBlink"
                  checked={$settings.terminal.cursorBlink}
                  on:change={(e) => updateTerminalSetting('cursorBlink', (e.target as HTMLInputElement).checked)}
                  class="sr-only peer"
                >
                <div class="w-11 h-6 bg-app-border peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-primary-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary-600"></div>
              </label>
            </div>
          </div>

        {:else if activeTab === 'connection'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            <!-- Auto Reconnect -->
            <div class="flex items-center justify-between">
              <div>
                <label class="block text-sm font-medium text-app-text-secondary" for="autoReconnect">
                  自动重连
                </label>
                <p class="text-xs text-app-text-secondary mt-0.5">意外断开连接时尝试自动重新连接</p>
              </div>
              <label class="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  id="autoReconnect"
                  checked={$settings.connection.autoReconnect}
                  on:change={(e) => updateConnectionSetting('autoReconnect', (e.target as HTMLInputElement).checked)}
                  class="sr-only peer"
                >
                <div class="w-11 h-6 bg-app-border peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-primary-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary-600"></div>
              </label>
            </div>
          </div>

        {:else if activeTab === 'appearance'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            {#if $settings.theme === 'custom'}
              <div class="p-4 border border-slate-200 dark:border-slate-700 rounded-xl space-y-3" transition:slide={{ duration: 200 }}>
                 <div class="flex items-center justify-between">
                   <h4 class="text-sm font-medium text-slate-900 dark:text-slate-200">自定义界面颜色</h4>
                   <button
                     class="text-xs px-3 py-1 bg-primary-600 hover:bg-primary-500 text-white rounded transition-colors"
                     on:click={resetCustomUITheme}
                   >
                     重置
                   </button>
                 </div>
                 <div class="grid grid-cols-2 gap-4">
                  {#each Object.entries(customUIColorLabels) as [k, label] (k)}
                     {@const key = k as CustomUIThemeKey}
                     {@const inputId = `custom-ui-color-${key}`}
                     <div>
                       <label for={inputId} class="block text-xs text-slate-500 mb-1">{label}</label>
                       <div class="flex gap-2">
                         <input
                           id={inputId}
                           type="color"
                           value={$settings.appearance.customUITheme?.[key] || defaultCustomUITheme[key]}
                           on:input={(e) => updateCustomUITheme(key, e.currentTarget.value)}
                           class="h-8 w-12 rounded cursor-pointer border border-slate-300 dark:border-slate-600 p-0.5 bg-transparent"
                         />
                         <input
                           type="text"
                           value={$settings.appearance.customUITheme?.[key] || defaultCustomUITheme[key]}
                           on:input={(e) => updateCustomUITheme(key, e.currentTarget.value)}
                           class="flex-1 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 text-xs font-mono"
                         />
                         {#if 'EyeDropper' in window}
                           <button
                             class="p-1.5 border border-slate-200 dark:border-slate-700 rounded bg-white dark:bg-slate-800 hover:bg-slate-50 dark:hover:bg-slate-700 text-slate-500 hover:text-primary-500 transition-colors"
                             title="取色器"
                             on:click={() => handleEyeDropper((c) => updateCustomUITheme(key, c))}
                           >
                             <EyeDropperIcon class="w-4 h-4" />
                           </button>
                         {/if}
                       </div>
                     </div>
                   {/each}
                 </div>
              </div>
            {/if}

            <!-- Accent Color -->
            <div>
              <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">主题色</span>
              <div class="flex flex-wrap gap-3">
                {#each Object.entries(themeColors) as [key, colors]}
                  <button
                    class="w-8 h-8 rounded-full border-2 transition-all {($settings.appearance.accentColor || 'blue') === key ? 'border-slate-900 dark:border-white scale-110' : 'border-transparent hover:scale-105'}"
                    style="background-color: {colors[500]}"
                    on:click={() => updateAccentColorSetting(key)}
                    title={key}
                    aria-label={`Select ${key} accent color`}
                  >
                  </button>
                {/each}
              </div>
            </div>

            <!-- ANSI Color Presets -->
            <div>
              <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">ANSI颜色预设</span>
              <div class="relative" bind:this={ansiPresetDropdownEl}>
                <button
                  type="button"
                  class="w-full flex items-center justify-between gap-3 bg-app-bg border border-app-border rounded-lg px-3 py-2 text-left text-app-text focus:border-primary-500 outline-none"
                  aria-haspopup="listbox"
                  aria-expanded={ansiPresetDropdownOpen}
                  on:click={() => ansiPresetDropdownOpen = !ansiPresetDropdownOpen}
                >
                  {#if selectedAnsiPreset}
                    <div class="flex items-center gap-3 min-w-0 flex-1">
                      {#if selectedAnsiPreset.id === 'custom'}
                        <div class="w-20 h-6 rounded border border-slate-300 dark:border-slate-600 bg-slate-100 dark:bg-slate-800 shrink-0"></div>
                      {:else}
                        <div
                          class="flex gap-0.5 p-1 rounded border border-slate-300 dark:border-slate-600 shrink-0"
                          style="background-color: {isLightAnsiPreset(selectedAnsiPreset.id) ? '#f8fafc' : '#1a1a1a'}"
                        >
                          {#each Object.entries(selectedAnsiPreset.previewColors) as [name, color]}
                            <div class="w-2.5 h-4 rounded-sm" style="background-color: {color}" title={name}></div>
                          {/each}
                        </div>
                      {/if}
                      <div class="min-w-0 flex-1">
                        <div class="text-sm font-medium text-app-text truncate">{selectedAnsiPreset.name}</div>
                      </div>
                    </div>
                  {/if}
                  <svg
                    class="w-4 h-4 text-app-text-secondary shrink-0 transition-transform {ansiPresetDropdownOpen ? 'rotate-180' : ''}"
                    viewBox="0 0 20 20"
                    fill="none"
                  >
                    <path d="m5 7.5 5 5 5-5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"></path>
                  </svg>
                </button>

                {#if ansiPresetDropdownOpen}
                  <div
                    class="absolute left-0 right-0 top-full mt-2 z-30 rounded-lg border border-app-border bg-app-surface shadow-xl overflow-hidden"
                    role="listbox"
                  >
                    <div class="max-h-80 overflow-y-auto custom-scrollbar py-1">
                      {#each ansiColorPresets as preset}
                        <button
                          type="button"
                          class="w-full px-3 py-2 flex items-center justify-between gap-3 text-left transition-colors {$settings.appearance.ansiColorPreset === preset.id ? 'bg-primary-50 dark:bg-primary-500/10' : 'hover:bg-app-bg'}"
                          role="option"
                          aria-selected={$settings.appearance.ansiColorPreset === preset.id}
                          on:click={() => selectAnsiPreset(preset.id)}
                        >
                          <div class="flex items-center gap-3 min-w-0 flex-1">
                            {#if preset.id === 'custom'}
                              <div class="w-20 h-6 rounded border border-slate-300 dark:border-slate-600 bg-slate-100 dark:bg-slate-800 shrink-0"></div>
                            {:else}
                              <div
                                class="flex gap-0.5 p-1 rounded border border-slate-300 dark:border-slate-600 shrink-0"
                                style="background-color: {isLightAnsiPreset(preset.id) ? '#f8fafc' : '#1a1a1a'}"
                              >
                                {#each Object.entries(preset.previewColors) as [name, color]}
                                  <div class="w-2.5 h-4 rounded-sm" style="background-color: {color}" title={name}></div>
                                {/each}
                              </div>
                            {/if}
                            <div class="min-w-0 flex-1">
                              <div class="text-sm font-medium text-slate-700 dark:text-slate-300 truncate">{preset.name}</div>
                              <div class="text-xs text-slate-500 dark:text-slate-400 truncate">{preset.description}</div>
                            </div>
                          </div>

                          {#if $settings.appearance.ansiColorPreset === preset.id}
                            <div class="w-2 h-2 bg-primary-500 rounded-full shrink-0"></div>
                          {/if}
                        </button>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>

              {#if selectedAnsiPreset}
                <div class="mt-3 p-3 border border-slate-200 dark:border-slate-700 rounded-lg bg-slate-50 dark:bg-slate-900/50">
                  <div class="flex items-center justify-between gap-4">
                    <div class="min-w-0">
                      <div class="text-sm font-medium text-slate-700 dark:text-slate-300 truncate">{selectedAnsiPreset.name}</div>
                      <div class="text-xs text-slate-500 dark:text-slate-400 mt-0.5">{selectedAnsiPreset.description}</div>
                    </div>

                    <div class="shrink-0">
                      {#if selectedAnsiPreset.id === 'custom'}
                        <div class="w-48 h-8 rounded shadow-sm border border-slate-300 dark:border-slate-600 bg-slate-100 dark:bg-slate-800 flex items-center justify-center">
                          <svg class="w-5 h-5 text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6v6m0 0v6m0-6h6m-6 0H6"></path>
                          </svg>
                        </div>
                      {:else}
                        <div
                          class="flex gap-1 p-1 rounded border border-slate-300 dark:border-slate-600"
                          style="background-color: {isLightAnsiPreset(selectedAnsiPreset.id) ? '#f8fafc' : '#1a1a1a'}"
                        >
                          {#each Object.entries(selectedAnsiPreset.previewColors) as [name, color]}
                            <div class="w-5 h-6 rounded-sm" style="background-color: {color}" title={name}></div>
                          {/each}
                        </div>
                      {/if}
                    </div>
                  </div>
                </div>
              {/if}

              {#if $settings.appearance.ansiColorPreset === 'custom'}
                <div class="mt-4 p-4 border border-slate-200 dark:border-slate-700 rounded-lg bg-slate-50 dark:bg-slate-900/50" transition:slide={{ duration: 200 }}>
                  <div class="flex items-center justify-between mb-4">
                    <h4 class="text-sm font-medium text-slate-700 dark:text-slate-300">自定义ANSI颜色</h4>
                    <button
                      class="text-xs px-3 py-1 bg-primary-600 hover:bg-primary-500 text-white rounded transition-colors"
                      on:click={resetCustomAnsiColors}
                    >
                      重置
                    </button>
                  </div>

                  <div class="grid grid-cols-2 sm:grid-cols-3 gap-3">
                    {#each ['foreground', 'red', 'green', 'yellow', 'blue', 'magenta'] as key}
                      {@const label = ansiColorLabels[key as AnsiColorKey]}
                      {@const value = $settings.appearance.customAnsiColors?.[key as AnsiColorKey] || defaultCustomAnsiColors[key as AnsiColorKey]}
                      {@const inputId = `ansi-color-${key}`}
                      <div class="flex items-center gap-2">
                        <input
                          id={inputId}
                          type="color"
                          value={value}
                          on:input={(e) => updateCustomAnsiColor(key as AnsiColorKey, (e.target as HTMLInputElement).value)}
                          class="w-8 h-8 rounded cursor-pointer border border-slate-300 dark:border-slate-600"
                        />
                        <label for={inputId} class="text-xs text-slate-600 dark:text-slate-400">{label}</label>
                      </div>
                    {/each}
                  </div>
                </div>
              {/if}
            </div>

            <!-- Terminal Theme -->
            <div>
              <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">终端主题</span>
              <div class="relative" bind:this={terminalThemeDropdownEl}>
                <button
                  type="button"
                  class="w-full flex items-center justify-between gap-3 bg-app-bg border border-app-border rounded-lg px-3 py-2 text-left text-app-text focus:border-primary-500 outline-none"
                  aria-haspopup="listbox"
                  aria-expanded={terminalThemeDropdownOpen}
                  on:click={() => terminalThemeDropdownOpen = !terminalThemeDropdownOpen}
                >
                  {#if selectedTerminalTheme}
                    <div class="flex items-center gap-3 min-w-0 flex-1">
                      <div
                        class="w-20 h-6 rounded border border-slate-300 dark:border-slate-600 shrink-0"
                        style="background-color: {selectedTerminalTheme.preview.background}"
                      >
                        <div class="px-1 py-0.5 text-[10px] font-mono truncate" style="color: {selectedTerminalTheme.preview.foreground}">
                          Aa
                        </div>
                      </div>
                      <div class="min-w-0 flex-1">
                        <div class="text-sm font-medium text-app-text truncate">{selectedTerminalTheme.name}</div>
                      </div>
                    </div>
                  {/if}
                  <svg
                    class="w-4 h-4 text-app-text-secondary shrink-0 transition-transform {terminalThemeDropdownOpen ? 'rotate-180' : ''}"
                    viewBox="0 0 20 20"
                    fill="none"
                  >
                    <path d="m5 7.5 5 5 5-5" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"></path>
                  </svg>
                </button>

                {#if terminalThemeDropdownOpen}
                  <div
                    class="absolute left-0 right-0 top-full mt-2 z-30 rounded-lg border border-app-border bg-app-surface shadow-xl overflow-hidden"
                    role="listbox"
                  >
                    <div class="max-h-80 overflow-y-auto custom-scrollbar py-1">
                      {#each terminalThemes as theme}
                        <button
                          type="button"
                          class="w-full px-3 py-2 flex items-center justify-between gap-3 text-left transition-colors {$settings.appearance.terminalTheme === theme.id ? 'bg-primary-50 dark:bg-primary-500/10' : 'hover:bg-app-bg'}"
                          role="option"
                          aria-selected={$settings.appearance.terminalTheme === theme.id}
                          on:click={() => selectTerminalTheme(theme.id)}
                        >
                          <div class="flex items-center gap-3 min-w-0 flex-1">
                            <div
                              class="w-20 h-6 rounded border border-slate-300 dark:border-slate-600 shrink-0"
                              style="background-color: {theme.preview.background}"
                            >
                              <div class="px-1 py-0.5 text-[10px] font-mono truncate" style="color: {theme.preview.foreground}">
                                Aa
                              </div>
                            </div>
                            <div class="min-w-0 flex-1">
                              <div class="text-sm font-medium text-slate-700 dark:text-slate-300 truncate">{theme.name}</div>
                            </div>
                          </div>

                          {#if $settings.appearance.terminalTheme === theme.id}
                            <div class="w-2 h-2 bg-primary-500 rounded-full shrink-0"></div>
                          {/if}
                        </button>
                      {/each}
                    </div>
                  </div>
                {/if}
              </div>

              {#if selectedTerminalTheme}
                <div class="mt-3 p-3 border border-slate-200 dark:border-slate-700 rounded-lg bg-slate-50 dark:bg-slate-900/50">
                  <div
                    class="w-full h-12 rounded shadow-sm"
                    style="background-color: {selectedTerminalTheme.preview.background}"
                  >
                    <div class="p-2 text-xs font-mono" style="color: {selectedTerminalTheme.preview.foreground}">
                      The quick brown fox
                    </div>
                  </div>
                </div>
              {/if}
            </div>

            <!-- Custom Theme Editor -->
            {#if $settings.appearance.terminalTheme === 'custom'}
              <div class="border border-slate-200 dark:border-slate-700 rounded-lg p-4 bg-slate-50 dark:bg-slate-900/50">
                <div class="flex items-center justify-between mb-4">
                  <span class="text-sm font-medium text-slate-700 dark:text-slate-300">自定义主题</span>
                  <button
                    class="text-xs px-3 py-1 bg-primary-600 hover:bg-primary-500 text-white rounded transition-colors"
                    on:click={resetCustomTheme}
                  >
                    重置
                  </button>
                </div>

                <div class="grid grid-cols-2 gap-4">
                  <!-- Basic Colors -->
                  <div class="space-y-2">
                    <h4 class="text-xs font-medium text-slate-600 dark:text-slate-400 uppercase">基础颜色</h4>
                  {#each basicColorKeys as key (key)}
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
                          class="flex-1 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 py-1 text-xs font-mono text-slate-900 dark:text-slate-200 focus:border-primary-500 outline-none"
                        />
                        {#if 'EyeDropper' in window}
                          <button
                            class="p-1.5 border border-slate-200 dark:border-slate-700 rounded bg-white dark:bg-slate-800 hover:bg-slate-50 dark:hover:bg-slate-700 text-slate-500 hover:text-primary-500 transition-colors"
                            title="取色器"
                            on:click={() => handleEyeDropper((c) => updateCustomColor(key, c))}
                          >
                            <EyeDropperIcon class="w-4 h-4" />
                          </button>
                        {/if}
                      </div>
                    {/each}
                  </div>

                  <!-- Standard Colors -->
                  <div class="space-y-2">
                    <h4 class="text-xs font-medium text-slate-600 dark:text-slate-400 uppercase">标准颜色</h4>
                  {#each standardColorKeys as key (key)}
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
                          class="flex-1 bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 py-1 text-xs font-mono text-slate-900 dark:text-slate-200 focus:border-primary-500 outline-none"
                        />
                        {#if 'EyeDropper' in window}
                          <button
                            class="p-1.5 border border-slate-200 dark:border-slate-700 rounded bg-white dark:bg-slate-800 hover:bg-slate-50 dark:hover:bg-slate-700 text-slate-500 hover:text-primary-500 transition-colors"
                            title="取色器"
                            on:click={() => handleEyeDropper((c) => updateCustomColor(key, c))}
                          >
                            <EyeDropperIcon class="w-4 h-4" />
                          </button>
                        {/if}
                      </div>
                    {/each}
                  </div>

                  <!-- Bright Colors -->
                  <div class="col-span-2 space-y-2">
                    <h4 class="text-xs font-medium text-slate-600 dark:text-slate-400 uppercase">亮色变体</h4>
                    <div class="grid grid-cols-4 gap-2">
                      {#each brightColorKeys as key (key)}
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

            <!-- Background Image -->
            <div>
              <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">终端背景</span>
              <div class="border border-slate-200 dark:border-slate-700 rounded-lg p-4 bg-slate-50 dark:bg-slate-900/50 space-y-4">
                <div class="flex items-start gap-4">
                  <!-- Preview -->
                  <div class="relative w-32 h-20 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 overflow-hidden shrink-0 group">
                    {#if $settings.appearance.backgroundImage}
                      <img src={$settings.appearance.backgroundImage} alt="Background Preview" class="w-full h-full object-cover" />
                      <button
                        class="absolute inset-0 bg-black/50 flex items-center justify-center opacity-0 group-hover:opacity-100 transition-opacity"
                        on:click={clearBackgroundImage}
                      >
                        <TrashIcon class="w-5 h-5 text-white" />
                      </button>
                    {:else}
                      <div class="w-full h-full flex items-center justify-center text-slate-400">
                        <span class="text-xs">无背景</span>
                      </div>
                    {/if}
                  </div>

                  <!-- Controls -->
                  <div class="flex-1 space-y-3">
                    <div>
                      <div class="block text-xs text-slate-500 mb-1">上传图片</div>
                      <label class="inline-flex items-center gap-2 px-3 py-1.5 bg-white dark:bg-slate-800 border border-slate-200 dark:border-slate-700 rounded text-xs font-medium cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-700 transition-colors text-slate-700 dark:text-slate-300">
                        <UploadIcon class="w-4 h-4" />
                        <span>选择文件...</span>
                        <input type="file" accept="image/*" class="hidden" on:change={handleBackgroundImageUpload} />
                      </label>
                    </div>
                  </div>
                </div>

                <div class="pt-4 border-t border-slate-200 dark:border-slate-700 space-y-4">
                  <!-- Opacity Control (Global) -->
                  <div>
                    <div class="flex justify-between mb-1">
                      <label for="bg-opacity-slider" class="text-xs text-slate-500">背景不透明度</label>
                      <span class="text-xs text-slate-500">{Math.round(($settings.appearance.backgroundOpacity ?? ($settings.appearance.backgroundImage ? 0.5 : 1)) * 100)}%</span>
                    </div>
                    <input
                      id="bg-opacity-slider"
                      type="range"
                      min="0"
                      max="1"
                      step="0.05"
                      value={$settings.appearance.backgroundOpacity ?? ($settings.appearance.backgroundImage ? 0.5 : 1)}
                      on:input={(e) => updateAppearanceSetting('backgroundOpacity', Number(e.currentTarget.value))}
                      class="w-full h-1 bg-slate-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer"
                    />
                  </div>

                  {#if $settings.appearance.backgroundImage}
                    <div class="grid grid-cols-2 gap-4">
                      <!-- Blur Control (Image only) -->
                      <div>
                        <div class="flex justify-between mb-1">
                          <label for="bg-blur-slider" class="text-xs text-slate-500">模糊度</label>
                          <span class="text-xs text-slate-500">{$settings.appearance.backgroundBlur ?? 0}px</span>
                        </div>
                        <input
                          id="bg-blur-slider"
                          type="range"
                          min="0"
                          max="20"
                          step="1"
                          value={$settings.appearance.backgroundBlur ?? 0}
                          on:input={(e) => updateAppearanceSetting('backgroundBlur', Number(e.currentTarget.value))}
                          class="w-full h-1 bg-slate-200 dark:bg-slate-700 rounded-lg appearance-none cursor-pointer"
                        />
                      </div>
                    </div>
                  {/if}
              </div>
            </div>
          </div>
        </div>
        {:else if activeTab === 'security'}
           <div class="space-y-6" in:slide={{ duration: 200 }}>
             <h3 class="text-lg font-medium text-app-text">应用安全锁</h3>
             <p class="text-sm text-app-text-secondary">设置启动密码以保护您的连接信息。</p>
             
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

             <div class="space-y-4 border border-app-border rounded-lg p-4 bg-app-surface">
               <h4 class="font-medium text-app-text">调试工具</h4>

               <div class="flex items-center justify-between">
                 <div>
                   <label class="block text-sm font-medium text-app-text-secondary" for="disableDevToolsShortcuts">
                     禁用调试工具快捷键
                   </label>
                   <p class="text-xs text-app-text-secondary mt-0.5">阻止 `F12`、`Ctrl+Shift+I/J/C` 打开 WebView 调试工具</p>
                 </div>
                 <label class="relative inline-flex items-center cursor-pointer">
                   <input
                     type="checkbox"
                     id="disableDevToolsShortcuts"
                     checked={$settings.security.disableDevToolsShortcuts}
                     on:change={(e) => updateSecuritySetting('disableDevToolsShortcuts', (e.target as HTMLInputElement).checked)}
                     class="sr-only peer"
                   >
                   <div class="w-11 h-6 bg-app-border peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-primary-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary-600"></div>
                 </label>
               </div>
             </div>

             {#if !hasLock}
               <!-- Setup Lock -->
               <div class="space-y-4 border border-app-border rounded-lg p-4 bg-app-surface">
                 <h4 class="font-medium text-app-text">设置新密码</h4>
                 <div>
                   <label class="block text-sm font-medium text-app-text-secondary mb-1" for="new-pwd">新密码</label>
                   <input type="password" id="new-pwd" bind:value={newPassword} class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none" />
                 </div>
                 <div>
                   <label class="block text-sm font-medium text-app-text-secondary mb-1" for="confirm-pwd">确认密码</label>
                   <input type="password" id="confirm-pwd" bind:value={confirmPassword} class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none" />
                 </div>
                 <button class="px-4 py-2 bg-primary-600 hover:bg-primary-500 text-white rounded-lg text-sm font-medium transition-colors" on:click={handleSetLock}>
                   启用应用锁
                 </button>
               </div>
             {:else}
               <!-- Auto Lock Settings -->
               <div class="space-y-4 border border-app-border rounded-lg p-4 bg-app-surface">
                 <h4 class="font-medium text-app-text">自动锁定</h4>
                 
                 <div class="flex items-center justify-between">
                    <div>
                      <label class="block text-sm font-medium text-app-text-secondary" for="autoLockTime">
                        自动锁定时间
                      </label>
                      <p class="text-xs text-app-text-secondary mt-0.5">无操作指定时间后自动锁定 (0 为禁用)</p>
                    </div>
                    <div class="flex items-center gap-2">
                      <input 
                        type="number" 
                        id="autoLockTime"
                        min="0"
                        max="120"
                        value={$settings.security.autoLockMinutes}
                        on:input={(e) => updateSecuritySetting('autoLockMinutes', Number((e.target as HTMLInputElement).value))}
                        class="w-16 bg-app-bg border border-app-border rounded-lg px-2 py-1 text-center text-app-text focus:border-primary-500 outline-none"
                      />
                      <span class="text-sm text-app-text-secondary">分钟</span>
                   </div>
                </div>

                <div class="flex items-center justify-between border-t border-app-border pt-4">
                   <div>
                     <label class="block text-sm font-medium text-app-text-secondary" for="lockOnBlur">
                       失去焦点时锁定
                     </label>
                     <p class="text-xs text-app-text-secondary mt-0.5">当切换到其他应用窗口时自动锁定</p>
                   </div>
                   <label class="relative inline-flex items-center cursor-pointer">
                     <input
                       type="checkbox"
                       id="lockOnBlur"
                       checked={$settings.security.lockOnBlur}
                       on:change={(e) => updateSecuritySetting('lockOnBlur', (e.target as HTMLInputElement).checked)}
                       class="sr-only peer"
                     >
                     <div class="w-11 h-6 bg-app-border peer-focus:outline-none peer-focus:ring-2 peer-focus:ring-primary-500 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-primary-600"></div>
                   </label>
                </div>
              </div>

              <!-- Change/Remove Lock -->
              <div class="space-y-4 border border-app-border rounded-lg p-4 bg-app-surface">
                <h4 class="font-medium text-app-text">管理密码</h4>
                <div>
                  <label class="block text-sm font-medium text-app-text-secondary mb-1" for="curr-pwd">当前密码</label>
                  <input type="password" id="curr-pwd" bind:value={oldPassword} class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none" />
                </div>
                
                <div class="pt-4 border-t border-app-border">
                   <h5 class="text-sm font-medium text-app-text-secondary mb-3">修改密码（可选）</h5>
                   <div class="space-y-3">
                       <div>
                       <label class="block text-sm font-medium text-app-text-secondary mb-1" for="new-pwd-change">新密码</label>
                       <input type="password" id="new-pwd-change" bind:value={newPassword} class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none" />
                       </div>
                       <div>
                       <label class="block text-sm font-medium text-app-text-secondary mb-1" for="confirm-pwd-change">确认新密码</label>
                       <input type="password" id="confirm-pwd-change" bind:value={confirmPassword} class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none" />
                       </div>
                       <div class="flex gap-3 pt-2">
                           <button class="px-4 py-2 bg-app-surface-light hover:bg-app-border text-app-text rounded-lg text-sm font-medium transition-colors" on:click={handleChangeLock}>
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
  .settings-select {
    appearance: none;
    -webkit-appearance: none;
    -moz-appearance: none;
    padding-right: 2.5rem;
    background-color: var(--color-bg);
    color: var(--color-text);
    -webkit-text-fill-color: var(--color-text);
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 20 20' fill='none'%3E%3Cpath d='m5 7.5 5 5 5-5' stroke='%2364748b' stroke-width='1.8' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
    background-position: right 0.875rem center;
    background-repeat: no-repeat;
    background-size: 0.875rem;
  }

  .settings-select:disabled {
    cursor: not-allowed;
    opacity: 0.7;
  }

  .settings-select option {
    background-color: var(--color-surface);
    color: var(--color-text);
  }

  :global(.dark) .settings-select {
    color-scheme: dark;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 20 20' fill='none'%3E%3Cpath d='m5 7.5 5 5 5-5' stroke='%2394a3b8' stroke-width='1.8' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E");
  }

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
