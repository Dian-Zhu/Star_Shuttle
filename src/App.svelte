<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'

  let showConnectionForm = false

  // 简化的连接对象，避免在模板中使用类型断言
  let newConnection = {
    name: '',
    host: '',
    port: 22,
    username: '',
    password: '',
    savePassword: false,
  }

  function handleNewConnection() {
    showConnectionForm = true
  }

  async function handleSaveConnection() {
    try {
      // Convert simplified connection object to backend format
      // Note: Rust enum variants use PascalCase (Password, PrivateKey, Agent)
      const backendAuthMethod = {
        Password: {
          password: newConnection.password,
          save_password: newConnection.savePassword,
        },
      }

      // Generate a UUID for the connection
      const { v4: uuidv4 } = await import('uuid');
      const connectionId = uuidv4();

      // Call backend save_connection_config command
      await invoke('save_connection_config', {
        config: {
          id: connectionId,
          name: newConnection.name,
          host: newConnection.host,
          port: newConnection.port,
          username: newConnection.username,
          auth_method: backendAuthMethod,
          description: null, // Use null instead of undefined for optional fields
          tags: [],
          group_id: null, // Use null instead of undefined for optional fields
          created_at: new Date().toISOString(),
          updated_at: new Date().toISOString(),
        },
      })

      console.log('连接保存成功！')

      // Reset form and hide
      newConnection = {
        name: '',
        host: '',
        port: 22,
        username: '',
        password: '',
        savePassword: false,
      }
      showConnectionForm = false
    } catch (error) {
      console.error('Error saving connection:', error)
      alert(`保存连接失败：${error}`)
    }
  }

  function handleCancel() {
    showConnectionForm = false
  }
</script>

<div class="h-screen w-screen flex flex-col bg-slate-950 text-slate-50">
  <!-- Header -->
  <header class="h-14 bg-slate-900 border-b border-slate-800 flex items-center justify-between px-5">
    <div class="flex items-center gap-3">
      <div class="w-8 h-8 bg-blue-600 rounded-md flex items-center justify-center">
        <span class="font-bold text-xs text-white">SSH</span>
      </div>
      <h1 class="text-lg font-semibold text-slate-100">
        SSH 远程管理器
      </h1>
    </div>
    <div class="flex items-center gap-3">
      <!-- Settings button will be added here -->
    </div>
  </header>

  <!-- Main Content -->
  <div class="flex-1 flex overflow-hidden">
    <!-- Sidebar -->
    <aside class="w-64 bg-slate-900 border-r border-slate-800 overflow-y-auto">
      <!-- Connection list will go here -->
      <div class="p-5">
        <h2 class="text-base font-semibold mb-3 text-slate-200">连接列表</h2>
        <button
          class="w-full bg-blue-600 hover:bg-blue-700 text-white py-2 px-3 rounded-md mb-4 font-medium transition-colors"
          on:click={handleNewConnection}
        >
          + 新建连接
        </button>
        <div class="space-y-2">
          <!-- Connection items will be dynamically added here -->
          <div class="p-3 bg-slate-800 rounded-md cursor-pointer hover:bg-slate-700 transition-colors">
            <div class="font-medium text-slate-100">示例连接</div>
            <div class="text-xs text-slate-400 mt-0.5">user@example.com:22</div>
          </div>
        </div>
      </div>
    </aside>

    <!-- Main Content Area -->
    <main class="flex-1 overflow-hidden flex flex-col">
      <!-- Tabs for terminal, file transfer, etc. -->
      <div class="flex border-b border-slate-800 bg-slate-900">
        <button class="px-5 py-2.5 font-medium text-blue-400 border-b-2 border-blue-500">
          终端
        </button>
        <button class="px-5 py-2.5 font-medium text-slate-400 hover:text-slate-200 transition-colors">
          文件传输
        </button>
      </div>

      <!-- Content based on selected tab -->
      <div class="flex-1 overflow-auto p-6">
        {#if showConnectionForm}
          <div class="max-w-2xl mx-auto bg-slate-900 rounded-lg border border-slate-800 p-6 shadow-subtle">
            <h2 class="text-xl font-semibold mb-5 text-slate-100">
              新建连接
            </h2>
            <form on:submit|preventDefault={handleSaveConnection}>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-5">
                <div class="md:col-span-2">
                <label for="name" class="block text-sm font-medium text-slate-300 mb-2">
                  连接名称
                </label>
                  <input
                    type="text"
                    id="name"
                    bind:value={newConnection.name}
                    class="w-full bg-slate-800 border border-slate-700 rounded-md px-3 py-2 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    placeholder="输入连接名称"
                    required
                  />
                </div>

                <div>
                  <label for="host" class="block text-sm font-medium text-slate-300 mb-2">主机</label>
                  <input
                    type="text"
                    id="host"
                    bind:value={newConnection.host}
                    class="w-full bg-slate-800 border border-slate-700 rounded-md px-3 py-2 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    placeholder="例如：example.com 或 192.168.1.1"
                    required
                  />
                </div>

                <div>
                  <label for="port" class="block text-sm font-medium text-slate-300 mb-2">端口</label>
                  <input
                    type="number"
                    id="port"
                    bind:value={newConnection.port}
                    class="w-full bg-slate-800 border border-slate-700 rounded-md px-3 py-2 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    min="1"
                    max="65535"
                    required
                  />
                </div>

                <div>
                  <label for="username" class="block text-sm font-medium text-slate-300 mb-2">
                    用户名
                  </label>
                  <input
                    type="text"
                    id="username"
                    bind:value={newConnection.username}
                    class="w-full bg-slate-800 border border-slate-700 rounded-md px-3 py-2 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    placeholder="输入用户名"
                    required
                  />
                </div>

                <div class="md:col-span-2">
                  <label for="password" class="block text-sm font-medium text-slate-300 mb-2">
                    密码
                  </label>
                  <input
                    type="password"
                    id="password"
                    bind:value={newConnection.password}
                    class="w-full bg-slate-800 border border-slate-700 rounded-md px-3 py-2 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent"
                    placeholder="输入密码"
                    required
                  />
                </div>

                <div class="md:col-span-2 flex items-center mt-2">
                  <input
                    type="checkbox"
                    id="savePassword"
                    bind:checked={newConnection.savePassword}
                    class="mr-3 h-4 w-4 text-blue-600 bg-slate-700 border-slate-600 rounded focus:ring-blue-500 focus:ring-offset-0"
                  />
                  <label for="savePassword" class="text-sm text-slate-300 cursor-pointer">保存密码</label>
                </div>
              </div>

              <div class="flex justify-end gap-3 pt-4 border-t border-slate-800">
                <button
                  type="button"
                  on:click={handleCancel}
                  class="px-5 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 rounded-md font-medium focus:outline-none focus:ring-2 focus:ring-slate-500"
                >
                  取消
                </button>
                <button
                  type="submit"
                  class="px-5 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded-md font-medium focus:outline-none focus:ring-2 focus:ring-blue-500"
                >
                  保存连接
                </button>
              </div>
            </form>
          </div>
        {:else}
          <div class="flex flex-col items-center justify-center h-full text-center p-6">
            <div class="w-20 h-20 bg-slate-800 rounded-full flex items-center justify-center mb-5">
              <svg class="w-10 h-10 text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
              </svg>
            </div>
            <h2 class="text-2xl font-semibold mb-3 text-slate-100">
              欢迎使用 SSH 远程管理器
            </h2>
            <p class="text-slate-400 text-base max-w-sm">
              从侧边栏选择连接或创建新连接以开始使用。
            </p>
          </div>
        {/if}
      </div>
    </main>
  </div>
</div>
