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
      const backendAuthMethod = {
        password: {
          password: newConnection.password,
          save_password: newConnection.savePassword,
        },
      }

      // Call backend save_connection_config command
      await invoke('save_connection_config', {
        config: {
          name: newConnection.name,
          host: newConnection.host,
          port: newConnection.port,
          username: newConnection.username,
          auth_method: backendAuthMethod,
          description: undefined,
          tags: [],
          group_id: undefined,
        },
      })

      console.log('Connection saved successfully!')

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
      alert(`Failed to save connection: ${error}`)
    }
  }

  function handleCancel() {
    showConnectionForm = false
  }
</script>

<div class="h-screen w-screen flex flex-col text-white">
  <!-- Header -->
  <header class="h-16 bg-gradient-to-r from-slate-900 via-slate-800 to-slate-900 border-b border-slate-700/50 flex items-center justify-between px-6 shadow-lg">
    <div class="flex items-center gap-3">
      <div class="w-10 h-10 bg-gradient-to-br from-blue-500 to-blue-600 rounded-xl flex items-center justify-center shadow-lg shadow-blue-500/30">
        <span class="font-bold text-sm">SSH</span>
      </div>
      <div>
        <h1 class="text-xl font-bold bg-gradient-to-r from-blue-400 to-blue-500 bg-clip-text text-transparent">
          SSH Remote Manager
        </h1>
      </div>
    </div>
    <div class="flex items-center gap-4">
      <!-- Settings button, etc. -->
    </div>
  </header>

  <!-- Main Content -->
  <div class="flex-1 flex overflow-hidden">
    <!-- Sidebar -->
    <aside class="w-72 bg-slate-800/50 backdrop-blur-sm border-r border-slate-700/50 overflow-y-auto">
      <!-- Connection list will go here -->
      <div class="p-6">
        <h2 class="text-lg font-semibold mb-4 text-slate-200">Connections</h2>
        <button
          class="w-full bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-400 text-white py-2.5 px-4 rounded-xl mb-4 font-medium shadow-lg shadow-blue-500/25 transition-all duration-200 hover:shadow-xl hover:shadow-blue-500/40"
          on:click={handleNewConnection}
        >
          + New Connection
        </button>
        <div class="space-y-2">
          <!-- Connection items will be dynamically added here -->
          <div class="p-3 bg-slate-700/50 rounded-xl cursor-pointer hover:bg-slate-700 border border-slate-600/50 transition-all duration-200">
            <div class="font-medium text-slate-100">Example Connection</div>
            <div class="text-sm text-slate-400 mt-1">user@example.com:22</div>
          </div>
        </div>
      </div>
    </aside>

    <!-- Main Content Area -->
    <main class="flex-1 overflow-hidden flex flex-col">
      <!-- Tabs for terminal, file transfer, etc. -->
      <div class="flex border-b border-slate-700/50 bg-slate-800/30">
        <button class="px-6 py-3 bg-slate-800/80 border-r border-slate-700/50 font-medium text-blue-400 border-b-2 border-blue-500">
          Terminal
        </button>
        <button class="px-6 py-3 font-medium text-slate-400 hover:text-slate-200 hover:bg-slate-800/50 transition-colors duration-200">
          File Transfer
        </button>
      </div>

      <!-- Content based on selected tab -->
      <div class="flex-1 overflow-auto p-8">
        {#if showConnectionForm}
          <div class="max-w-2xl mx-auto bg-slate-800/50 backdrop-blur-sm rounded-2xl p-8 border border-slate-700/50 shadow-xl">
            <h2 class="text-2xl font-bold mb-6 bg-gradient-to-r from-blue-400 to-blue-500 bg-clip-text text-transparent">
              New Connection
            </h2>
            <form on:submit|preventDefault={handleSaveConnection}>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-5 mb-6">
                <div class="md:col-span-2">
                  <label for="name" class="block text-sm font-medium text-slate-300 mb-2"
                    >Connection Name</label
                  >
                  <input
                    type="text"
                    id="name"
                    bind:value={newConnection.name}
                    class="w-full bg-slate-700/50 border border-slate-600/50 rounded-xl px-4 py-2.5 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all duration-200"
                    placeholder="Enter connection name"
                    required
                  />
                </div>

                <div>
                  <label for="host" class="block text-sm font-medium text-slate-300 mb-2">Host</label
                  >
                  <input
                    type="text"
                    id="host"
                    bind:value={newConnection.host}
                    class="w-full bg-slate-700/50 border border-slate-600/50 rounded-xl px-4 py-2.5 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all duration-200"
                    placeholder="e.g., example.com or 192.168.1.1"
                    required
                  />
                </div>

                <div>
                  <label for="port" class="block text-sm font-medium text-slate-300 mb-2">Port</label
                  >
                  <input
                    type="number"
                    id="port"
                    bind:value={newConnection.port}
                    class="w-full bg-slate-700/50 border border-slate-600/50 rounded-xl px-4 py-2.5 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all duration-200"
                    min="1"
                    max="65535"
                    required
                  />
                </div>

                <div>
                  <label for="username" class="block text-sm font-medium text-slate-300 mb-2"
                    >Username</label
                  >
                  <input
                    type="text"
                    id="username"
                    bind:value={newConnection.username}
                    class="w-full bg-slate-700/50 border border-slate-600/50 rounded-xl px-4 py-2.5 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all duration-200"
                    placeholder="Enter username"
                    required
                  />
                </div>

                <div class="md:col-span-2">
                  <label for="password" class="block text-sm font-medium text-slate-300 mb-2"
                    >Password</label
                  >
                  <input
                    type="password"
                    id="password"
                    bind:value={newConnection.password}
                    class="w-full bg-slate-700/50 border border-slate-600/50 rounded-xl px-4 py-2.5 text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-transparent transition-all duration-200"
                    placeholder="Enter password"
                    required
                  />
                </div>

                <div class="md:col-span-2 flex items-center bg-slate-700/30 rounded-xl px-4 py-2.5">
                  <input
                    type="checkbox"
                    id="savePassword"
                    bind:checked={newConnection.savePassword}
                    class="mr-3 h-4 w-4 text-blue-600 bg-slate-600 border-slate-500 rounded focus:ring-blue-500 focus:ring-offset-0"
                  />
                  <label for="savePassword" class="text-sm text-slate-300 cursor-pointer">Save password</label>
                </div>
              </div>

              <div class="flex justify-end gap-3 pt-4 border-t border-slate-700/50">
                <button
                  type="button"
                  on:click={handleCancel}
                  class="px-6 py-2.5 bg-slate-700/50 hover:bg-slate-700 text-slate-200 rounded-xl font-medium focus:outline-none focus:ring-2 focus:ring-slate-500 transition-all duration-200"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  class="px-6 py-2.5 bg-gradient-to-r from-blue-600 to-blue-500 hover:from-blue-500 hover:to-blue-400 text-white rounded-xl font-medium shadow-lg shadow-blue-500/25 focus:outline-none focus:ring-2 focus:ring-blue-500 transition-all duration-200 hover:shadow-xl hover:shadow-blue-500/40"
                >
                  Save Connection
                </button>
              </div>
            </form>
          </div>
        {:else}
          <div class="flex flex-col items-center justify-center h-full text-center">
            <div class="w-24 h-24 bg-gradient-to-br from-blue-500/20 to-blue-600/20 rounded-full flex items-center justify-center mb-6 shadow-lg shadow-blue-500/10">
              <svg class="w-12 h-12 text-blue-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8.228 9c.549-1.165 2.03-2 3.772-2 2.21 0 4 1.343 4 3 0 1.4-1.278 2.575-3.006 2.907-.542.104-.994.54-.994 1.093m0 3h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
              </svg>
            </div>
            <h2 class="text-3xl font-bold mb-3 bg-gradient-to-r from-blue-400 to-blue-500 bg-clip-text text-transparent">
              Welcome to SSH Remote Manager
            </h2>
            <p class="text-slate-400 text-lg max-w-md">
              Select a connection from the sidebar or create a new one to get started.
            </p>
          </div>
        {/if}
      </div>
    </main>
  </div>
</div>
