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

<div class="h-screen w-screen flex flex-col bg-gray-900 text-white">
  <!-- Header -->
  <header class="h-16 bg-gray-800 border-b border-gray-700 flex items-center justify-between px-4">
    <div class="flex items-center gap-2">
      <div class="w-8 h-8 bg-blue-600 rounded flex items-center justify-center">
        <span class="font-bold">SSH</span>
      </div>
      <h1 class="text-xl font-semibold">SSH Remote Manager</h1>
    </div>
    <div class="flex items-center gap-4">
      <!-- Settings button, etc. -->
    </div>
  </header>

  <!-- Main Content -->
  <div class="flex-1 flex overflow-hidden">
    <!-- Sidebar -->
    <aside class="w-64 bg-gray-800 border-r border-gray-700 overflow-y-auto">
      <!-- Connection list will go here -->
      <div class="p-4">
        <h2 class="text-lg font-semibold mb-4">Connections</h2>
        <button
          class="w-full bg-blue-600 hover:bg-blue-700 text-white py-2 px-4 rounded mb-4"
          on:click={handleNewConnection}
        >
          New Connection
        </button>
        <div class="space-y-2">
          <!-- Connection items will be dynamically added here -->
          <div class="p-2 bg-gray-700 rounded cursor-pointer hover:bg-gray-600">
            <div class="font-medium">Example Connection</div>
            <div class="text-sm text-gray-400">user@example.com:22</div>
          </div>
        </div>
      </div>
    </aside>

    <!-- Main Content Area -->
    <main class="flex-1 overflow-hidden flex flex-col">
      <!-- Tabs for terminal, file transfer, etc. -->
      <div class="flex border-b border-gray-700">
        <button class="px-4 py-2 bg-gray-800 border-r border-gray-700 font-medium">
          Terminal
        </button>
        <button class="px-4 py-2 bg-gray-900 font-medium text-gray-400 hover:text-white">
          File Transfer
        </button>
      </div>

      <!-- Content based on selected tab -->
      <div class="flex-1 overflow-auto p-4">
        {#if showConnectionForm}
          <div class="max-w-2xl mx-auto bg-gray-800 rounded-lg p-6 border border-gray-700">
            <h2 class="text-xl font-semibold mb-4">New Connection</h2>
            <form on:submit|preventDefault={handleSaveConnection}>
              <div class="grid grid-cols-1 md:grid-cols-2 gap-4 mb-4">
                <div class="md:col-span-2">
                  <label for="name" class="block text-sm font-medium text-gray-300 mb-1"
                    >Connection Name</label
                  >
                  <input
                    type="text"
                    id="name"
                    bind:value={newConnection.name}
                    class="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="Enter connection name"
                    required
                  />
                </div>

                <div>
                  <label for="host" class="block text-sm font-medium text-gray-300 mb-1">Host</label
                  >
                  <input
                    type="text"
                    id="host"
                    bind:value={newConnection.host}
                    class="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="e.g., example.com or 192.168.1.1"
                    required
                  />
                </div>

                <div>
                  <label for="port" class="block text-sm font-medium text-gray-300 mb-1">Port</label
                  >
                  <input
                    type="number"
                    id="port"
                    bind:value={newConnection.port}
                    class="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    min="1"
                    max="65535"
                    required
                  />
                </div>

                <div>
                  <label for="username" class="block text-sm font-medium text-gray-300 mb-1"
                    >Username</label
                  >
                  <input
                    type="text"
                    id="username"
                    bind:value={newConnection.username}
                    class="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="Enter username"
                    required
                  />
                </div>

                <div class="md:col-span-2">
                  <label for="password" class="block text-sm font-medium text-gray-300 mb-1"
                    >Password</label
                  >
                  <input
                    type="password"
                    id="password"
                    bind:value={newConnection.password}
                    class="w-full bg-gray-700 border border-gray-600 rounded px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-blue-500"
                    placeholder="Enter password"
                    required
                  />
                </div>

                <div class="md:col-span-2 flex items-center">
                  <input
                    type="checkbox"
                    id="savePassword"
                    bind:checked={newConnection.savePassword}
                    class="mr-2 h-4 w-4 text-blue-600 bg-gray-700 border-gray-600 rounded focus:ring-blue-500"
                  />
                  <label for="savePassword" class="text-sm text-gray-300">Save password</label>
                </div>
              </div>

              <div class="flex justify-end gap-3">
                <button
                  type="button"
                  on:click={handleCancel}
                  class="px-4 py-2 bg-gray-700 hover:bg-gray-600 text-white rounded focus:outline-none focus:ring-2 focus:ring-gray-500"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  class="px-4 py-2 bg-blue-600 hover:bg-blue-700 text-white rounded focus:outline-none focus:ring-2 focus:ring-blue-500"
                >
                  Save Connection
                </button>
              </div>
            </form>
          </div>
        {:else}
          <div class="text-center text-gray-500">
            <h2 class="text-2xl font-semibold mb-2">Welcome to SSH Remote Manager</h2>
            <p>Select a connection from the sidebar or create a new one to get started.</p>
          </div>
        {/if}
      </div>
    </main>
  </div>
</div>
