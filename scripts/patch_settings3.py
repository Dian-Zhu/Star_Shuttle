#!/usr/bin/env python3

path = '/home/rust/Star_Shuttle/src/components/SettingsModal.svelte'
content = open(path, 'r', encoding='utf-8').read()

# Add AI imports after existing imports
old_import = "  import { getShortcutFromKeyboardEvent, normalizeShortcut } from '../lib/shortcuts';"
new_import = """  import { getShortcutFromKeyboardEvent, normalizeShortcut } from '../lib/shortcuts';
  import { getAiConfig, saveAiConfig, getProviderDefaults, testAiConnection, type AiConfig, DEFAULT_CONFIG } from '../lib/aiConfigService';"""
content = content.replace(old_import, new_import)

# Add AI state variables after `let activeTab = 'terminal';`
old_state = "  let activeTab = 'terminal';"
new_state = """  let activeTab = 'terminal';

  // AI Config State
  let aiConfig: AiConfig = { ...DEFAULT_CONFIG };
  let aiSaveMessage = '';
  let aiError = '';
  let aiTesting = false;

  async function loadAiConfig() {
    try {
      aiConfig = await getAiConfig();
    } catch (e) {
      // use defaults
    }
  }

  async function onAiProviderChange() {
    try {
      const defaults = await getProviderDefaults(aiConfig.provider);
      aiConfig.base_url = defaults.base_url;
      aiConfig.model = defaults.model;
    } catch {}
  }

  async function saveAiConfigHandler() {
    try {
      aiSaveMessage = '';
      aiError = '';
      await saveAiConfig(aiConfig);
      aiSaveMessage = '配置已保存';
      setTimeout(() => aiSaveMessage = '', 3000);
    } catch (e: any) {
      aiError = e?.message || '保存失败';
    }
  }

  async function testAiConnectionHandler() {
    try {
      aiTesting = true;
      aiError = '';
      aiSaveMessage = '';
      await testAiConnection();
      aiSaveMessage = '连接测试成功！';
      setTimeout(() => aiSaveMessage = '', 3000);
    } catch (e: any) {
      aiError = `连接失败：${e?.message || e}`;
    } finally {
      aiTesting = false;
    }
  }"""
content = content.replace(old_state, new_state)

# Load AI config in onMount
old_mount = "  onMount(() => {\n    checkLockStatus();"
new_mount = "  onMount(() => {\n    checkLockStatus();\n    loadAiConfig();"
content = content.replace(old_mount, new_mount)

print("imports:", "aiConfigService" in content)
print("state:", "aiConfig" in content)
print("onMount:", "loadAiConfig()" in content)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print("done")
