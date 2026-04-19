import { invoke } from '@tauri-apps/api/core'

export type AiProvider = 'openai' | 'claude' | 'deepseek' | 'ollama' | 'custom'

export interface AiConfig {
  provider: AiProvider
  api_key: string
  model: string
  base_url: string
  temperature: number
  max_tokens: number
  context_lines: number
}

export interface ProviderDefaults {
  base_url: string
  model: string
}

export const DEFAULT_CONFIG: AiConfig = {
  provider: 'openai',
  api_key: '',
  model: 'gpt-4o',
  base_url: 'https://api.openai.com/v1',
  temperature: 0.7,
  max_tokens: 4096,
  context_lines: 100,
}

export const PROVIDER_LABELS: Record<AiProvider, string> = {
  openai: 'OpenAI',
  claude: 'Anthropic Claude',
  deepseek: 'DeepSeek',
  ollama: 'Ollama (Local)',
  custom: 'Custom',
}

/** 从后端加载 AI 配置 */
export async function getAiConfig(): Promise<AiConfig> {
  try {
    return await invoke<AiConfig>('ai_get_config')
  } catch {
    return { ...DEFAULT_CONFIG }
  }
}

/** 保存 AI 配置到后端 */
export async function saveAiConfig(config: AiConfig): Promise<void> {
  await invoke('ai_save_config', { config })
}

/** 获取指定 provider 的默认参数 */
export async function getProviderDefaults(provider: AiProvider): Promise<ProviderDefaults> {
  return invoke<ProviderDefaults>('ai_get_provider_defaults', { provider })
}

/** 测试指定或已保存的 AI 配置是否可用 */
export async function testAiConnection(config?: AiConfig): Promise<void> {
  await invoke('ai_test_connection', config ? { config } : {})
}
