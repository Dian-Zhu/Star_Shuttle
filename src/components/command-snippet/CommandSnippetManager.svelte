<script lang="ts">
  import { onMount } from 'svelte';
  import type { CommandSnippet } from '../../types';
  import { commandSnippetService } from '../../lib/commandSnippetService';
  import { successMessage, errorMessage } from '../../lib/store';

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
      errorMessage.set('加载命令片段失败: ' + err.message);
      setTimeout(() => errorMessage.set(null), 5000);
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
      errorMessage.set('名称和命令不能为空');
      setTimeout(() => errorMessage.set(null), 3000);
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
      successMessage.set(editingSnippet ? '命令片段已更新' : '命令片段已添加');
      setTimeout(() => successMessage.set(null), 3000);
      await loadSnippets();
      showForm = false;
    } catch (err: any) {
      errorMessage.set('保存失败: ' + err.message);
      setTimeout(() => errorMessage.set(null), 5000);
    }
  }

  async function deleteSnippet(id: string) {
    if (!confirm('确定删除此命令片段吗？')) return;
    try {
      await commandSnippetService.delete(id);
      successMessage.set('命令片段已删除');
      setTimeout(() => successMessage.set(null), 3000);
      await loadSnippets();
    } catch (err: any) {
      errorMessage.set('删除失败: ' + err.message);
      setTimeout(() => errorMessage.set(null), 5000);
    }
  }

  async function useSnippet(snippet: CommandSnippet) {
    // Emit event to parent component (terminal) to insert command
    const event = new CustomEvent('useSnippet', { detail: snippet });
    window.dispatchEvent(event);
    // Increment usage count
    try {
      await commandSnippetService.incrementUsage(snippet.id);
      snippet.usage_count++;
    } catch (err) {
      // silent fail
    }
  }

  onMount(() => {
    loadSnippets();
  });
</script>

<div class="p-4 bg-gray-900 text-white h-full">
  <!-- Header -->
  <div class="flex justify-between items-center mb-6">
    <div>
      <h2 class="text-2xl font-bold">快捷命令片段库</h2>
      <p class="text-gray-400 text-sm mt-1">存储和管理常用命令模板，支持参数化</p>
    </div>
    <button
      class="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded font-medium"
      on:click={startAdd}
    >
      添加新片段
    </button>
  </div>

  <!-- Form Modal -->
  {#if showForm}
    <div class="fixed inset-0 bg-black/70 flex items-center justify-center z-50 p-4">
      <div class="bg-gray-800 rounded-lg shadow-xl w-full max-w-2xl">
        <div class="p-6">
          <h3 class="text-xl font-bold mb-4">
            {editingSnippet ? '编辑命令片段' : '新建命令片段'}
          </h3>
          <div class="space-y-4">
            <div>
              <label for="snippet-name" class="block text-sm font-medium text-gray-300 mb-1">名称 *</label>
              <input
                id="snippet-name"
                type="text"
                class="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 text-white"
                bind:value={name}
                placeholder="例如：重启 Nginx"
              />
            </div>
            <div>
              <label for="snippet-command" class="block text-sm font-medium text-gray-300 mb-1">命令 *</label>
              <textarea
                id="snippet-command"
                class="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 text-white font-mono text-sm h-32"
                bind:value={command}
                placeholder="例如：sudo systemctl restart nginx"
              ></textarea>
              <p class="text-gray-400 text-xs mt-1">
                使用 {'{{'}variable{'}}'} 作为参数占位符，例如：cd {'{{'}path{'}}'}
              </p>
            </div>
            <div class="grid grid-cols-2 gap-4">
              <div>
                <label for="snippet-category" class="block text-sm font-medium text-gray-300 mb-1">分类</label>
                <input
                  id="snippet-category"
                  type="text"
                  class="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 text-white"
                  bind:value={category}
                  placeholder="例如：系统管理"
                />
              </div>
              <div>
                <label for="snippet-tags" class="block text-sm font-medium text-gray-300 mb-1">标签</label>
                <input
                  id="snippet-tags"
                  type="text"
                  class="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 text-white"
                  bind:value={tags}
                  placeholder="逗号分隔，例如：nginx,重启,服务"
                />
              </div>
            </div>
            <div>
              <label for="snippet-description" class="block text-sm font-medium text-gray-300 mb-1">描述</label>
              <textarea
                id="snippet-description"
                class="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 text-white text-sm h-20"
                bind:value={description}
                placeholder="可选描述"
              ></textarea>
            </div>
          </div>
          <div class="flex justify-end space-x-3 mt-8">
            <button
              class="px-4 py-2 bg-gray-700 hover:bg-gray-600 rounded"
              on:click={() => showForm = false}
            >
              取消
            </button>
            <button
              class="px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded"
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
    <div class="bg-gray-800/50 rounded p-4">
      <div class="text-2xl font-bold">{snippets.length}</div>
      <div class="text-gray-400 text-sm">总片段数</div>
    </div>
    <div class="bg-gray-800/50 rounded p-4">
      <div class="text-2xl font-bold">
        {snippets.reduce((sum, s) => sum + s.usage_count, 0)}
      </div>
      <div class="text-gray-400 text-sm">总使用次数</div>
    </div>
    <div class="bg-gray-800/50 rounded p:4">
      <div class="text-2xl font-bold">
        {[...new Set(snippets.map(s => s.category).filter(Boolean))].length}
      </div>
      <div class="text-gray-400 text-sm">分类数量</div>
    </div>
    <div class="bg-gray-800/50 rounded p:4">
      <div class="text-2xl font-bold">
        {snippets.filter(s => s.usage_count > 0).length}
      </div>
      <div class="text-gray-400 text-sm">已使用片段</div>
    </div>
  </div>

  <!-- Snippets Table -->
  <div class="bg-gray-800 rounded-lg overflow-hidden">
    {#if loading}
      <div class="p-8 text-center">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-white mx-auto"></div>
        <p class="mt-2 text-gray-400">加载中...</p>
      </div>
    {:else if snippets.length === 0}
      <div class="p-8 text-center">
        <div class="text-4xl mb-4">📝</div>
        <h3 class="text-lg font-medium mb-2">暂无命令片段</h3>
        <p class="text-gray-400 max-w-sm mx-auto">
          点击上方“添加新片段”按钮来创建你的第一个命令模板。
        </p>
      </div>
    {:else}
      <table class="w-full">
        <thead class="bg-gray-900 text-gray-400 text-sm font-semibold">
          <tr>
            <th class="p-3 text-left">名称</th>
            <th class="p-3 text-left">命令</th>
            <th class="p-3 text-left">分类</th>
            <th class="p-3 text-left">使用次数</th>
            <th class="p-3 text-left">操作</th>
          </tr>
        </thead>
        <tbody>
          {#each snippets as snippet (snippet.id)}
            <tr class="border-t border-gray-700 hover:bg-gray-750">
              <td class="p-3">
                <div class="font-medium">{snippet.name}</div>
                {#if snippet.description}
                  <div class="text-gray-400 text-xs mt-1">{snippet.description}</div>
                {/if}
              </td>
              <td class="p-3">
                <code class="bg-gray-900/50 px-2 py-1 rounded text-sm font-mono break-all">
                  {snippet.command}
                </code>
              </td>
              <td class="p-3">
                {#if snippet.category}
                  <span class="px-2 py-1 bg-blue-900/30 text-blue-300 rounded text-xs">
                    {snippet.category}
                  </span>
                {:else}
                  <span class="text-gray-500 text-xs">未分类</span>
                {/if}
              </td>
              <td class="p-3">
                <div class="text-lg font-bold">{snippet.usage_count}</div>
                <div class="text-gray-500 text-xs">次</div>
              </td>
              <td class="p-3">
                <div class="flex space-x-2">
                  <button
                    class="px-3 py-1 bg-green-700 hover:bg-green-600 rounded text-sm"
                    on:click={() => useSnippet(snippet)}
                  >
                    使用
                  </button>
                  <button
                    class="px-3 py-1 bg-gray-700 hover:bg-gray-600 rounded text-sm"
                    on:click={() => startEdit(snippet)}
                  >
                    编辑
                  </button>
                  <button
                    class="px-3 py-1 bg-red-800/30 hover:bg-red-700/30 text-red-400 hover:text-red-300 rounded text-sm"
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
