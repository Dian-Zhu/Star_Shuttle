<script lang="ts">
  import { get } from 'svelte/store';
  import { onMount } from 'svelte';
  import { confirm } from '@tauri-apps/plugin-dialog';
  import type { CommandSnippet } from '../../types';
  import { commandSnippetService } from '../../lib/commandSnippetService';
  import { activeTerminals, selectedTerminalIndex, showSuccessMessage, showErrorMessage } from '../../lib/store';
  import { handleTerminalInputSingle } from '../../lib/terminalService';

  let snippets: CommandSnippet[] = [];
  let loading = false;
  let showForm = false;
  let editingSnippet: CommandSnippet | null = null;

  // Form fields
  let name = '';
  let command = '';
  let description = '';
  let category = '';
  let tags = '';

  async function loadSnippets() {
    loading = true;
    try {
      snippets = await commandSnippetService.getAll();
    } catch (err: any) {
      showErrorMessage('加载命令片段失败: ' + err.message, 5000);
    } finally {
      loading = false;
    }
  }

  function startAdd() {
    editingSnippet = null;
    resetForm();
    showForm = true;
  }

  function startEdit(snippet: CommandSnippet) {
    editingSnippet = snippet;
    name = snippet.name;
    command = snippet.command;
    description = snippet.description || '';
    category = snippet.category || '';
    tags = snippet.tags || '';
    showForm = true;
  }

  function resetForm() {
    name = '';
    command = '';
    description = '';
    category = '';
    tags = '';
  }

  async function saveSnippet() {
    if (!name.trim() || !command.trim()) {
      showErrorMessage('名称和命令不能为空', 3000);
      return;
    }

    const snippetData: Partial<CommandSnippet> = {
      id: editingSnippet?.id || crypto.randomUUID(),
      name,
      command,
      description: description.trim() || undefined,
      category: category.trim() || undefined,
      tags: tags.trim() || undefined,
      created_at: editingSnippet?.created_at || Date.now(),
      updated_at: Date.now(),
      usage_count: editingSnippet?.usage_count || 0,
    };

    try {
      await commandSnippetService.save(snippetData as CommandSnippet);
      showSuccessMessage(editingSnippet ? '命令片段已更新' : '命令片段已添加', 3000);
      await loadSnippets();
      showForm = false;
    } catch (err: any) {
      showErrorMessage('保存失败: ' + err.message, 5000);
    }
  }

  async function deleteSnippet(id: string) {
    const confirmed = await confirm('确定删除此命令片段吗？', { title: '删除命令片段', kind: 'warning' });
    if (!confirmed) return;
    try {
      await commandSnippetService.delete(id);
      showSuccessMessage('命令片段已删除', 3000);
      await loadSnippets();
    } catch (err: any) {
      showErrorMessage('删除失败: ' + err.message, 5000);
    }
  }

  async function useSnippet(snippet: CommandSnippet) {
    const terminal = get(activeTerminals)[get(selectedTerminalIndex)];
    if (!terminal?.sessionId) {
      showErrorMessage('请先选择一个活动终端', 3000);
      return;
    }

    handleTerminalInputSingle(terminal.sessionId, snippet.command);

    try {
      await commandSnippetService.incrementUsage(snippet.id);
      snippets = snippets.map((item) =>
        item.id === snippet.id
          ? { ...item, usage_count: item.usage_count + 1 }
          : item
      );
    } catch {
      // silent fail
    }
  }

  onMount(() => {
    loadSnippets();
  });
</script>

<div class="p-4 bg-app-bg text-app-text h-full transition-colors duration-200">
  <!-- Header -->
  <div class="flex justify-between items-center mb-6">
    <div>
      <h2 class="text-2xl font-bold">快捷命令片段库</h2>
      <p class="text-app-text-secondary text-sm mt-1">存储和管理常用命令模板，支持参数化</p>
    </div>
    <button
      class="px-4 py-2 bg-primary-600 hover:bg-primary-700 text-white rounded font-medium shadow-sm transition-colors"
      on:click={startAdd}
    >
      添加新片段
    </button>
  </div>

  <!-- Form Modal -->
  {#if showForm}
    <div class="fixed inset-0 bg-slate-900/50 dark:bg-black/70 flex items-center justify-center z-50 p-4 backdrop-blur-sm">
      <div class="bg-app-surface rounded-lg shadow-xl w-full max-w-2xl border border-app-border">
        <div class="p-6">
          <h3 class="text-xl font-bold mb-4 text-app-text">
            {editingSnippet ? '编辑命令片段' : '新建命令片段'}
          </h3>
          <div class="space-y-4">
            <div>
              <label for="snippet-name" class="block text-sm font-medium text-app-text mb-1">名称 *</label>
              <input
                id="snippet-name"
                type="text"
                class="w-full bg-app-bg border border-app-border rounded px-3 py-2 text-app-text focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all"
                bind:value={name}
                placeholder="例如：重启 Nginx"
              />
            </div>
            <div>
              <label for="snippet-command" class="block text-sm font-medium text-app-text mb-1">命令 *</label>
              <textarea
                id="snippet-command"
                class="w-full bg-app-bg border border-app-border rounded px-3 py-2 text-app-text font-mono text-sm h-32 focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all"
                bind:value={command}
                placeholder="例如：sudo systemctl restart nginx"
              ></textarea>
              <p class="text-app-text-secondary text-xs mt-1">
                使用 {'{{'}variable{'}}'} 作为参数占位符，例如：cd {'{{'}path{'}}'}
              </p>
            </div>
            <div class="grid grid-cols-2 gap-4">
              <div>
                <label for="snippet-category" class="block text-sm font-medium text-app-text mb-1">分类</label>
                <input
                  id="snippet-category"
                  type="text"
                  class="w-full bg-app-bg border border-app-border rounded px-3 py-2 text-app-text focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all"
                  bind:value={category}
                  placeholder="例如：系统管理"
                />
              </div>
              <div>
                <label for="snippet-tags" class="block text-sm font-medium text-app-text mb-1">标签</label>
                <input
                  id="snippet-tags"
                  type="text"
                  class="w-full bg-app-bg border border-app-border rounded px-3 py-2 text-app-text focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all"
                  bind:value={tags}
                  placeholder="逗号分隔，例如：nginx,重启,服务"
                />
              </div>
            </div>
            <div>
              <label for="snippet-description" class="block text-sm font-medium text-app-text mb-1">描述</label>
              <textarea
                id="snippet-description"
                class="w-full bg-app-bg border border-app-border rounded px-3 py-2 text-app-text text-sm h-20 focus:ring-2 focus:ring-primary-500 focus:border-transparent outline-none transition-all"
                bind:value={description}
                placeholder="可选描述"
              ></textarea>
            </div>
          </div>
          <div class="flex justify-end space-x-3 mt-8">
            <button
              class="px-4 py-2 bg-app-bg hover:bg-app-bg-hover text-app-text rounded transition-colors"
              on:click={() => showForm = false}
            >
              取消
            </button>
            <button
              class="px-4 py-2 bg-primary-600 hover:bg-primary-700 text-white rounded transition-colors"
              on:click={saveSnippet}
            >
              {editingSnippet ? '更新' : '保存'}
            </button>
          </div>
        </div>
      </div>
    </div>
  {/if}

  <!-- Statistics -->
  <div class="grid grid-cols-4 gap-4 mb-6">
    <div class="bg-app-surface rounded-lg p-4 border border-app-border">
      <div class="text-2xl font-bold text-app-text">{snippets.length}</div>
      <div class="text-app-text-secondary text-sm">总片段数</div>
    </div>
    <div class="bg-app-surface rounded-lg p-4 border border-app-border">
      <div class="text-2xl font-bold text-app-text">
        {snippets.reduce((sum, s) => sum + s.usage_count, 0)}
      </div>
      <div class="text-app-text-secondary text-sm">总使用次数</div>
    </div>
    <div class="bg-app-surface rounded-lg p-4 border border-app-border">
      <div class="text-2xl font-bold text-app-text">
        {[...new Set(snippets.map(s => s.category).filter(Boolean))].length}
      </div>
      <div class="text-app-text-secondary text-sm">分类数量</div>
    </div>
    <div class="bg-app-surface rounded-lg p-4 border border-app-border">
      <div class="text-2xl font-bold text-app-text">
        {snippets.filter(s => s.usage_count > 0).length}
      </div>
      <div class="text-app-text-secondary text-sm">已使用片段</div>
    </div>
  </div>

  <!-- Snippets Table -->
  <div class="bg-app-surface rounded-lg overflow-hidden border border-app-border shadow-sm">
    {#if loading}
      <div class="p-8 text-center">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-500 mx-auto"></div>
        <p class="mt-2 text-app-text-secondary">加载中...</p>
      </div>
    {:else if snippets.length === 0}
      <div class="p-8 text-center">
        <div class="text-4xl mb-4">📝</div>
        <h3 class="text-lg font-medium mb-2 text-app-text">暂无命令片段</h3>
        <p class="text-app-text-secondary max-w-sm mx-auto">
          点击上方“添加新片段”按钮来创建你的第一个命令模板。
        </p>
      </div>
    {:else}
      <table class="w-full">
        <thead class="bg-app-bg text-app-text-secondary text-sm font-semibold border-b border-app-border">
          <tr>
            <th class="p-3 text-left">名称</th>
            <th class="p-3 text-left">命令</th>
            <th class="p-3 text-left">分类</th>
            <th class="p-3 text-left">使用次数</th>
            <th class="p-3 text-left">操作</th>
          </tr>
        </thead>
        <tbody class="divide-y divide-app-border">
          {#each snippets as snippet (snippet.id)}
            <tr class="hover:bg-app-bg-hover transition-colors">
              <td class="p-3">
                <div class="font-medium text-app-text">{snippet.name}</div>
                {#if snippet.description}
                  <div class="text-app-text-secondary text-xs mt-1">{snippet.description}</div>
                {/if}
              </td>
              <td class="p-3">
                <code class="bg-app-bg text-app-text px-2 py-1 rounded text-sm font-mono break-all border border-app-border">
                  {snippet.command}
                </code>
              </td>
              <td class="p-3">
                {#if snippet.category}
                  <span class="px-2 py-1 bg-primary-100 text-primary-700 dark:bg-primary-900/30 dark:text-primary-300 rounded text-xs font-medium">
                    {snippet.category}
                  </span>
                {:else}
                  <span class="text-app-text-secondary text-xs">未分类</span>
                {/if}
              </td>
              <td class="p-3">
                <div class="text-lg font-bold text-app-text">{snippet.usage_count}</div>
                <div class="text-app-text-secondary text-xs">次</div>
              </td>
              <td class="p-3">
                <div class="flex space-x-2">
                  <button
                    class="px-3 py-1 bg-green-600 hover:bg-green-700 text-white rounded text-sm shadow-sm transition-colors"
                    on:click={() => useSnippet(snippet)}
                  >
                    使用
                  </button>
                  <button
                    class="px-3 py-1 bg-app-bg hover:bg-app-bg-hover text-app-text rounded text-sm transition-colors"
                    on:click={() => startEdit(snippet)}
                  >
                    编辑
                  </button>
                  <button
                    class="px-3 py-1 bg-red-100 hover:bg-red-200 text-red-600 dark:bg-red-800/30 dark:hover:bg-red-700/30 dark:text-red-400 rounded text-sm transition-colors"
                    on:click={() => deleteSnippet(snippet.id)}
                  >
                    删除
                  </button>
                </div>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</div>