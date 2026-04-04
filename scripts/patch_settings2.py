#!/usr/bin/env python3

path = '/home/rust/Star_Shuttle/src/components/SettingsModal.svelte'
content = open(path, 'r', encoding='utf-8').read()

ai_panel = """        {:else if activeTab === 'ai'}
          <div class="space-y-6" in:slide={{ duration: 200 }}>
            <div>
              <h3 class="text-lg font-medium text-app-text">AI 助手配置</h3>
              <p class="text-sm text-app-text-secondary mt-1">配置 AI 服务商以启用 Chat 和 Agent 模式。</p>
            </div>

            {#if aiSaveMessage}
              <div class="p-3 bg-green-50 dark:bg-green-500/10 border border-green-200 dark:border-green-500/20 text-green-600 dark:text-green-400 rounded-lg text-sm">
                {aiSaveMessage}
              </div>
            {/if}
            {#if aiError}
              <div class="p-3 bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/20 text-red-600 dark:text-red-400 rounded-lg text-sm">
                {aiError}
              </div>
            {/if}

            <!-- Provider Selection -->
            <div class="space-y-4 border border-app-border rounded-lg p-4 bg-app-surface">
              <h4 class="font-medium text-app-text">服务商</h4>
              <div>
                <label class="block text-sm font-medium text-app-text-secondary mb-1" for="ai-provider">AI 服务商</label>
                <select
                  id="ai-provider"
                  class="settings-select w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none"
                  bind:value={aiConfig.provider}
                  on:change={onAiProviderChange}
                >
                  <option value="openai">OpenAI</option>
                  <option value="claude">Anthropic Claude</option>
                  <option value="deepseek">DeepSeek</option>
                  <option value="ollama">Ollama (本地)</option>
                  <option value="custom">自定义</option>
                </select>
              </div>
              <div>
                <label class="block text-sm font-medium text-app-text-secondary mb-1" for="ai-base-url">API Base URL</label>
                <input
                  type="text"
                  id="ai-base-url"
                  bind:value={aiConfig.base_url}
                  class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none font-mono text-sm"
                  placeholder="https://api.openai.com/v1"
                />
              </div>
              <div>
                <label class="block text-sm font-medium text-app-text-secondary mb-1" for="ai-api-key">API Key</label>
                <input
                  type="password"
                  id="ai-api-key"
                  bind:value={aiConfig.api_key}
                  class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none font-mono text-sm"
                  placeholder="sk-..."
                />
              </div>
              <div>
                <label class="block text-sm font-medium text-app-text-secondary mb-1" for="ai-model">模型</label>
                <input
                  type="text"
                  id="ai-model"
                  bind:value={aiConfig.model}
                  class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none"
                  placeholder="gpt-4o"
                />
              </div>
            </div>

            <!-- Parameters -->
            <div class="space-y-4 border border-app-border rounded-lg p-4 bg-app-surface">
              <h4 class="font-medium text-app-text">参数</h4>
              <div class="grid grid-cols-2 gap-4">
                <div>
                  <label class="block text-sm font-medium text-app-text-secondary mb-1" for="ai-temp">
                    温度 <span class="text-xs text-app-text-secondary">({aiConfig.temperature})</span>
                  </label>
                  <input
                    type="range" id="ai-temp" min="0" max="2" step="0.1"
                    bind:value={aiConfig.temperature}
                    class="w-full"
                  />
                </div>
                <div>
                  <label class="block text-sm font-medium text-app-text-secondary mb-1" for="ai-max-tokens">最大 Token 数</label>
                  <input
                    type="number" id="ai-max-tokens" min="512" max="32768" step="512"
                    bind:value={aiConfig.max_tokens}
                    class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none"
                  />
                </div>
              </div>
              <div>
                <label class="block text-sm font-medium text-app-text-secondary mb-1" for="ai-context-lines">
                  终端上下文行数 <span class="text-xs text-app-text-secondary">(附加给 AI 的终端历史行数)</span>
                </label>
                <input
                  type="number" id="ai-context-lines" min="10" max="500" step="10"
                  bind:value={aiConfig.context_lines}
                  class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-app-text focus:border-primary-500 outline-none"
                />
              </div>
            </div>

            <!-- Actions -->
            <div class="flex gap-3">
              <button
                class="px-4 py-2 bg-primary-600 hover:bg-primary-500 text-white rounded-lg text-sm font-medium transition-colors"
                on:click={saveAiConfigHandler}
              >
                保存配置
              </button>
              <button
                class="px-4 py-2 bg-app-surface-light hover:bg-app-border text-app-text rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
                on:click={testAiConnectionHandler}
                disabled={aiTesting}
              >
                {aiTesting ? '测试中...' : '测试连接'}
              </button>
            </div>
          </div>
"""

# Insert AI panel before the closing {/if} of the tab switching block
old = "        {/if}\n      </div>\n    </div>\n  </div>\n</div>"
new = ai_panel + "        {/if}\n      </div>\n    </div>\n  </div>\n</div>"
content = content.replace(old, new)
print("AI panel injected:", "activeTab === 'ai'" in content)

with open(path, 'w', encoding='utf-8') as f:
    f.write(content)
print("done")
