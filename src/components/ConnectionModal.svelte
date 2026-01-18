<script lang="ts">
  import { showConnectionForm, editingConnection, connections, connectionGroups, type Connection } from '../lib/store';
  import { saveConnection } from '../lib/connectionService';
  import { connectAndOpen } from '../lib/terminalService';
  import XIcon from './icons/XIcon.svelte';
  import { slide } from 'svelte/transition';
  import { get } from 'svelte/store';
  import { onMount } from 'svelte';

  // Form state
  let formData = {
    id: '',
    name: '',
    host: '',
    port: 22,
    username: '',
    authMethod: 'password' as 'password' | 'keyboardInteractive' | 'privateKey' | 'agent' | 'certificate',
    password: '',
    savePassword: false,
    keyPath: '',
    passphrase: '',
    savePassphrase: false,
    agentPath: '',
    certificatePath: '',
    privateKeyPath: '',
    description: '',
    tags: '', // Comma separated string for backend compatibility
    groupId: '',
    localForwards: [] as { local_host: string; local_port: number; remote_host: string; remote_port: number }[],
    remoteForwards: [] as { remote_host: string; remote_port: number; local_host: string; local_port: number }[],
    proxyType: 'none' as 'none' | 'socks5' | 'http' | 'jumpHost',
    proxyHost: '',
    proxyPort: 1080,
    proxyUsername: '',
    proxyPassword: '',
    jumpHostUsername: '',
    jumpHostAuthMethod: 'password' as 'password' | 'keyboardInteractive' | 'privateKey' | 'agent' | 'certificate',
    jumpHostPassword: '',
    jumpHostSavePassword: false,
    jumpHostKeyPath: '',
    jumpHostPassphrase: '',
    jumpHostSavePassphrase: false,
    jumpHostAgentPath: '',
    jumpHostCertificatePath: '',
    jumpHostPrivateKeyPath: '',
    socksProxyPort: undefined as number | undefined,
  };

  function hydrateFromConnection(connection: Connection) {
    formData.id = connection.id;
    formData.name = connection.name ?? '';
    formData.host = connection.host ?? '';
    formData.port = Number(connection.port ?? 22);
    formData.username = connection.username ?? '';
    formData.description = connection.description ?? '';
    formData.tags = (connection.tags ?? []).join(',');
    formData.groupId = connection.group_id ?? '';

    formData.localForwards = (connection.local_forwards ?? []).map(f => ({
      local_host: f.local_host,
      local_port: Number(f.local_port),
      remote_host: f.remote_host,
      remote_port: Number(f.remote_port),
    }));
    formData.remoteForwards = (connection.remote_forwards ?? []).map(f => ({
      remote_host: f.remote_host,
      remote_port: Number(f.remote_port),
      local_host: f.local_host,
      local_port: Number(f.local_port),
    }));

    const socksPort = (connection as any).socks_proxy_port;
    formData.socksProxyPort = typeof socksPort === 'number' ? socksPort : undefined;

    formData.password = '';
    formData.savePassword = false;
    formData.keyPath = '';
    formData.passphrase = '';
    formData.savePassphrase = false;
    formData.agentPath = '';
    formData.certificatePath = '';
    formData.privateKeyPath = '';

    const authMethod = connection.auth_method ?? {};
    if ((authMethod as any).Password) {
      formData.authMethod = 'password';
      formData.password = (authMethod as any).Password.password ?? '';
      formData.savePassword = Boolean((authMethod as any).Password.save_password);
    } else if ((authMethod as any).KeyboardInteractive) {
      formData.authMethod = 'keyboardInteractive';
    } else if ((authMethod as any).PrivateKey) {
      formData.authMethod = 'privateKey';
      formData.keyPath = (authMethod as any).PrivateKey.key_path ?? '';
      formData.passphrase = (authMethod as any).PrivateKey.passphrase ?? '';
      formData.savePassphrase = Boolean((authMethod as any).PrivateKey.save_passphrase);
    } else if ((authMethod as any).Agent) {
      formData.authMethod = 'agent';
      formData.agentPath = (authMethod as any).Agent.agent_path ?? '';
    } else if ((authMethod as any).Certificate) {
      formData.authMethod = 'certificate';
      formData.certificatePath = (authMethod as any).Certificate.certificate_path ?? '';
      formData.privateKeyPath = (authMethod as any).Certificate.private_key_path ?? '';
      formData.passphrase = (authMethod as any).Certificate.passphrase ?? '';
      formData.savePassphrase = Boolean((authMethod as any).Certificate.save_passphrase);
    } else {
      formData.authMethod = 'password';
    }

    formData.proxyType = 'none';
    formData.proxyHost = '';
    formData.proxyPort = 1080;
    formData.proxyUsername = '';
    formData.proxyPassword = '';
    formData.jumpHostUsername = '';
    formData.jumpHostAuthMethod = 'password';
    formData.jumpHostPassword = '';
    formData.jumpHostSavePassword = false;
    formData.jumpHostKeyPath = '';
    formData.jumpHostPassphrase = '';
    formData.jumpHostSavePassphrase = false;
    formData.jumpHostAgentPath = '';
    formData.jumpHostCertificatePath = '';
    formData.jumpHostPrivateKeyPath = '';

    const proxyType = (connection as any).proxy_type;
    if (!proxyType || proxyType === 'None') {
      formData.proxyType = 'none';
    } else if (proxyType.Socks5) {
      formData.proxyType = 'socks5';
      formData.proxyHost = proxyType.Socks5.host ?? '';
      formData.proxyPort = Number(proxyType.Socks5.port ?? 1080);
      formData.proxyUsername = proxyType.Socks5.username ?? '';
      formData.proxyPassword = proxyType.Socks5.password ?? '';
    } else if (proxyType.Http) {
      formData.proxyType = 'http';
      formData.proxyHost = proxyType.Http.host ?? '';
      formData.proxyPort = Number(proxyType.Http.port ?? 8080);
      formData.proxyUsername = proxyType.Http.username ?? '';
      formData.proxyPassword = proxyType.Http.password ?? '';
    } else if (proxyType.JumpHost) {
      formData.proxyType = 'jumpHost';
      formData.proxyHost = proxyType.JumpHost.host ?? '';
      formData.proxyPort = Number(proxyType.JumpHost.port ?? 22);
      formData.jumpHostUsername = proxyType.JumpHost.username ?? '';

      const jumpAuth = proxyType.JumpHost.auth_method ?? {};
      if (jumpAuth.Password) {
        formData.jumpHostAuthMethod = 'password';
        formData.jumpHostPassword = jumpAuth.Password.password ?? '';
        formData.jumpHostSavePassword = Boolean(jumpAuth.Password.save_password);
      } else if (jumpAuth.KeyboardInteractive) {
        formData.jumpHostAuthMethod = 'keyboardInteractive';
      } else if (jumpAuth.PrivateKey) {
        formData.jumpHostAuthMethod = 'privateKey';
        formData.jumpHostKeyPath = jumpAuth.PrivateKey.key_path ?? '';
        formData.jumpHostPassphrase = jumpAuth.PrivateKey.passphrase ?? '';
        formData.jumpHostSavePassphrase = Boolean(jumpAuth.PrivateKey.save_passphrase);
      } else if (jumpAuth.Agent) {
        formData.jumpHostAuthMethod = 'agent';
        formData.jumpHostAgentPath = jumpAuth.Agent.agent_path ?? '';
      } else if (jumpAuth.Certificate) {
        formData.jumpHostAuthMethod = 'certificate';
        formData.jumpHostCertificatePath = jumpAuth.Certificate.certificate_path ?? '';
        formData.jumpHostPrivateKeyPath = jumpAuth.Certificate.private_key_path ?? '';
        formData.jumpHostPassphrase = jumpAuth.Certificate.passphrase ?? '';
        formData.jumpHostSavePassphrase = Boolean(jumpAuth.Certificate.save_passphrase);
      }
    }
  }

  function sanitizeConnectConfig(config: any) {
    const out = JSON.parse(JSON.stringify(config));
    if (out?.auth_method?.Password) out.auth_method.Password.password = '';
    if (out?.auth_method?.PrivateKey) delete out.auth_method.PrivateKey.passphrase;
    if (out?.auth_method?.Certificate) delete out.auth_method.Certificate.passphrase;

    if (out?.proxy_type?.JumpHost?.auth_method?.Password) {
      out.proxy_type.JumpHost.auth_method.Password.password = '';
    }
    if (out?.proxy_type?.JumpHost?.auth_method?.PrivateKey) {
      delete out.proxy_type.JumpHost.auth_method.PrivateKey.passphrase;
    }
    if (out?.proxy_type?.JumpHost?.auth_method?.Certificate) {
      delete out.proxy_type.JumpHost.auth_method.Certificate.passphrase;
    }
    return out;
  }

  // Tabs
  let activeTab: 'basic' | 'advanced' = 'basic';

  // Tag Management
  let tagInput = '';
  $: currentTags = formData.tags ? formData.tags.split(',').map(t => t.trim()).filter(Boolean) : [];
  
  // Get all unique existing tags for suggestions
  $: availableTags = Array.from(new Set($connections.flatMap(c => c.tags || []))).filter(t => !currentTags.includes(t));

  function addTag(tag: string) {
    const trimmed = tag.trim();
    if (trimmed && !currentTags.includes(trimmed)) {
      const newTags = [...currentTags, trimmed];
      formData.tags = newTags.join(',');
    }
    tagInput = '';
  }

  function removeTag(tag: string) {
    const newTags = currentTags.filter(t => t !== tag);
    formData.tags = newTags.join(',');
  }

  function handleTagKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault();
      addTag(tagInput);
    }
  }

  // Temporary variables for adding new forwards
  let newLocalForward = { local_host: 'localhost', local_port: 0, remote_host: 'localhost', remote_port: 0 };
  let newRemoteForward = { remote_host: 'localhost', remote_port: 0, local_host: 'localhost', local_port: 0 };

  let isSaving = false;

  onMount(() => {
    const c = get(editingConnection);
    if (c) {
      hydrateFromConnection(c);
    }
  });

  async function handleSubmit() {
    const isEditing = Boolean(formData.id);
    isSaving = true;
    const result = await saveConnection({
      ...formData,
      local_forwards: formData.localForwards,
      remote_forwards: formData.remoteForwards,
    });
    isSaving = false;
    
    if (result) {
      if (!isEditing) {
        const connection = $connections.find(c => c.id === result.connectionId);
        const safeConnection = connection ?? sanitizeConnectConfig(result.connectConfig);
        connectAndOpen(safeConnection as any, result.connectConfig);
      }
      handleClose();
    }
  }

  function addLocalForward() {
    if (newLocalForward.local_port > 0 && newLocalForward.remote_port > 0) {
      formData.localForwards = [...formData.localForwards, { ...newLocalForward }];
      newLocalForward = { local_host: 'localhost', local_port: 0, remote_host: 'localhost', remote_port: 0 };
    }
  }

  function removeLocalForward(index: number) {
    formData.localForwards = formData.localForwards.filter((_, i) => i !== index);
  }

  function addRemoteForward() {
    if (newRemoteForward.remote_port > 0 && newRemoteForward.local_port > 0) {
      formData.remoteForwards = [...formData.remoteForwards, { ...newRemoteForward }];
      newRemoteForward = { remote_host: 'localhost', remote_port: 0, local_host: 'localhost', local_port: 0 };
    }
  }

  function removeRemoteForward(index: number) {
    formData.remoteForwards = formData.remoteForwards.filter((_, i) => i !== index);
  }

  function handleClose() {
    editingConnection.set(null);
    showConnectionForm.set(false);
  }
</script>

<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 backdrop-blur-sm p-4" role="button" tabindex="0" on:click|self={handleClose} on:keydown={(e) => e.key === 'Escape' && handleClose()}>
  <div class="bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 rounded-xl shadow-2xl w-full max-w-2xl max-h-[90vh] flex flex-col overflow-hidden">
    <!-- Header -->
    <div class="flex items-center justify-between px-6 py-4 border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900">
      <div class="flex items-center gap-4">
        <h2 class="text-lg font-semibold text-slate-900 dark:text-slate-100">{$editingConnection ? '编辑连接' : '新建连接'}</h2>
        <!-- Tabs -->
        <div class="flex bg-slate-100 dark:bg-slate-800 rounded-lg p-1">
          <button 
            class="px-3 py-1 text-xs font-medium rounded-md transition-all {activeTab === 'basic' ? 'bg-white dark:bg-slate-700 text-blue-600 dark:text-blue-400 shadow-sm' : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'}"
            on:click={() => activeTab = 'basic'}
          >
            基本信息
          </button>
          <button 
            class="px-3 py-1 text-xs font-medium rounded-md transition-all {activeTab === 'advanced' ? 'bg-white dark:bg-slate-700 text-blue-600 dark:text-blue-400 shadow-sm' : 'text-slate-500 hover:text-slate-700 dark:hover:text-slate-300'}"
            on:click={() => activeTab = 'advanced'}
          >
            高级 & 隧道
          </button>
        </div>
      </div>
      <button 
        class="text-slate-500 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white transition-colors p-1 rounded-md hover:bg-slate-100 dark:hover:bg-slate-800"
        on:click={handleClose}
      >
        <XIcon class="w-5 h-5" />
      </button>
    </div>

    <!-- Scrollable Content -->
    <div class="flex-1 overflow-y-auto p-6 custom-scrollbar">
      <form id="connection-form" on:submit|preventDefault={handleSubmit} class="space-y-6">
        {#if activeTab === 'basic'}
          <div in:slide={{ duration: 200 }} class="space-y-6">
            <!-- Basic Info -->
            <div class="grid grid-cols-1 md:grid-cols-2 gap-5">
              <div class="md:col-span-2">
                <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="name">连接名称</label>
                <input
                  type="text"
                  id="name"
                  bind:value={formData.name}
                  class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all"
                  placeholder="例如: 生产环境服务器"
                  required
                />
              </div>

              <div class="md:col-span-2 grid grid-cols-12 gap-4">
                <div class="col-span-8">
                  <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="host">主机地址</label>
                  <input
                    type="text"
                    id="host"
                    bind:value={formData.host}
                    class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono"
                    placeholder="192.168.1.1 或 example.com"
                    required
                  />
                </div>
                <div class="col-span-4">
                  <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="port">端口</label>
                  <input
                    type="number"
                    id="port"
                    bind:value={formData.port}
                    class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono"
                    min="1"
                    max="65535"
                    required
                  />
                </div>
              </div>

              <div class="md:col-span-2">
                <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="username">用户名</label>
                <input
                  type="text"
                  id="username"
                  bind:value={formData.username}
                  class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono"
                  placeholder="root"
                  required
                />
              </div>
            </div>

            <!-- Authentication -->
            <div class="border-t border-slate-200 dark:border-slate-800 pt-5">
              <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">认证方式</span>
              
              <div class="flex space-x-4 mb-4">
                <label class="flex items-center cursor-pointer">
                  <input type="radio" bind:group={formData.authMethod} value="password" class="w-4 h-4 text-blue-600 bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                  <span class="ml-2 text-sm text-slate-700 dark:text-slate-300">密码</span>
                </label>
                <label class="flex items-center cursor-pointer">
                  <input type="radio" bind:group={formData.authMethod} value="keyboardInteractive" class="w-4 h-4 text-blue-600 bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                  <span class="ml-2 text-sm text-slate-700 dark:text-slate-300">MFA</span>
                </label>
                <label class="flex items-center cursor-pointer">
                  <input type="radio" bind:group={formData.authMethod} value="privateKey" class="w-4 h-4 text-blue-600 bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                  <span class="ml-2 text-sm text-slate-700 dark:text-slate-300">私钥</span>
                </label>
                <label class="flex items-center cursor-pointer">
                  <input type="radio" bind:group={formData.authMethod} value="agent" class="w-4 h-4 text-blue-600 bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                  <span class="ml-2 text-sm text-slate-700 dark:text-slate-300">Agent</span>
                </label>
                <label class="flex items-center cursor-pointer">
                  <input type="radio" bind:group={formData.authMethod} value="certificate" class="w-4 h-4 text-blue-600 bg-slate-100 dark:bg-slate-800 border-slate-300 dark:border-slate-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                  <span class="ml-2 text-sm text-slate-700 dark:text-slate-300">证书</span>
                </label>
              </div>

              <div class="bg-slate-50/50 dark:bg-slate-950/50 rounded-lg p-4 border border-slate-200/50 dark:border-slate-800/50">
                {#if formData.authMethod === 'password'}
                  <div>
                    <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="password">密码</label>
                    <input
                      type="password"
                      id="password"
                      bind:value={formData.password}
                      class="w-full bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all"
                    />
                    <label class="flex items-center mt-2 cursor-pointer">
                      <input type="checkbox" bind:checked={formData.savePassword} class="rounded border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-blue-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                      <span class="ml-2 text-xs text-slate-500 dark:text-slate-400">保存密码</span>
                    </label>
                  </div>
                {:else if formData.authMethod === 'keyboardInteractive'}
                  <div class="text-sm text-slate-600 dark:text-slate-400">
                    连接时会弹出交互式认证提示（MFA），输入内容不会被保存。
                  </div>
                {:else if formData.authMethod === 'privateKey'}
                  <div class="space-y-3">
                    <div>
                      <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="keyPath">私钥路径</label>
                      <input
                        type="text"
                        id="keyPath"
                        bind:value={formData.keyPath}
                        class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono text-sm"
                        placeholder="~/.ssh/id_rsa"
                      />
                    </div>
                    <div>
                      <label class="block text-sm font-medium text-slate-400 mb-1.5" for="passphrase">密码短语 (可选)</label>
                      <input
                        type="password"
                        id="passphrase"
                        bind:value={formData.passphrase}
                        class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all"
                      />
                      <label class="flex items-center mt-2 cursor-pointer">
                        <input type="checkbox" bind:checked={formData.savePassphrase} class="rounded border-slate-700 bg-slate-900 text-blue-600 focus:ring-blue-600 ring-offset-slate-900">
                        <span class="ml-2 text-xs text-slate-400">保存密码短语</span>
                      </label>
                    </div>
                  </div>
                {:else if formData.authMethod === 'agent'}
                  <div>
                    <label class="block text-sm font-medium text-slate-400 mb-1.5" for="agentPath">Agent 路径 (可选)</label>
                    <input
                      type="text"
                      id="agentPath"
                      bind:value={formData.agentPath}
                      class="w-full bg-slate-900 border border-slate-700 rounded-lg px-3 py-2 text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono text-sm"
                      placeholder="默认使用系统 SSH_AUTH_SOCK"
                    />
                  </div>
                {:else if formData.authMethod === 'certificate'}
                  <div class="space-y-3">
                    <div>
                      <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="certificatePath">证书路径</label>
                      <input
                        type="text"
                        id="certificatePath"
                        bind:value={formData.certificatePath}
                        class="w-full bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono text-sm"
                        placeholder="~/.ssh/id_rsa-cert.pub"
                      />
                    </div>
                    <div>
                      <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="privateKeyPath">私钥路径</label>
                      <input
                        type="text"
                        id="privateKeyPath"
                        bind:value={formData.privateKeyPath}
                        class="w-full bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all font-mono text-sm"
                        placeholder="~/.ssh/id_rsa"
                      />
                    </div>
                    <div>
                      <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="passphrase">密码短语 (可选)</label>
                      <input
                        type="password"
                        id="passphrase"
                        bind:value={formData.passphrase}
                        class="w-full bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all"
                      />
                      <label class="flex items-center mt-2 cursor-pointer">
                        <input type="checkbox" bind:checked={formData.savePassphrase} class="rounded border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-blue-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                        <span class="ml-2 text-xs text-slate-500 dark:text-slate-400">保存密码短语</span>
                      </label>
                    </div>
                  </div>
                {/if}
              </div>
            </div>

            <!-- Tags & Description -->
            <div class="border-t border-slate-200 dark:border-slate-800 pt-5 space-y-4">
               <div>
                  <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="groupId">分组</label>
                  <select
                    id="groupId"
                    bind:value={formData.groupId}
                    class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all"
                  >
                    <option value="">未分组</option>
                    {#each $connectionGroups as group}
                      <option value={group.id}>{group.name}</option>
                    {/each}
                  </select>
               </div>
               <!-- Tags -->
               <div>
                  <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="tags">标签</label>
                  <div class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg p-2 flex flex-wrap gap-2 min-h-[42px] focus-within:border-blue-500 focus-within:ring-1 focus-within:ring-blue-500 transition-all">
                    {#each currentTags as tag}
                      <span class="bg-blue-100 dark:bg-blue-900/30 text-blue-700 dark:text-blue-300 text-xs px-2 py-1 rounded-md flex items-center gap-1">
                        {tag}
                        <button type="button" on:click={() => removeTag(tag)} class="hover:text-blue-900 dark:hover:text-blue-100">
                          <XIcon class="w-3 h-3" />
                        </button>
                      </span>
                    {/each}
                    <input
                      type="text"
                      bind:value={tagInput}
                      on:keydown={handleTagKeydown}
                      class="bg-transparent border-none outline-none text-sm min-w-[80px] flex-1 text-slate-900 dark:text-slate-200 placeholder-slate-400"
                      placeholder={currentTags.length === 0 ? "输入标签并回车..." : ""}
                    />
                  </div>
                  <!-- Suggestions -->
                  {#if availableTags.length > 0}
                    <div class="flex flex-wrap gap-2 mt-2">
                      <span class="text-xs text-slate-500 py-1">推荐:</span>
                      {#each availableTags as tag}
                         <button 
                           type="button" 
                           class="text-xs px-2 py-1 rounded-md bg-slate-100 dark:bg-slate-800 text-slate-600 dark:text-slate-400 hover:bg-slate-200 dark:hover:bg-slate-700 transition-colors"
                           on:click={() => addTag(tag)}
                         >
                           {tag}
                         </button>
                      {/each}
                    </div>
                  {/if}
               </div>

               <!-- Description -->
               <div>
                  <label class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-1.5" for="description">描述</label>
                  <textarea
                    id="description"
                    bind:value={formData.description}
                    rows="3"
                    class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded-lg px-3 py-2 text-slate-900 dark:text-slate-200 focus:border-blue-500 focus:ring-1 focus:ring-blue-500 outline-none transition-all resize-none"
                    placeholder="关于此服务器的备注信息..."
                  ></textarea>
               </div>
            </div>
            
            <!-- Proxy Configuration -->
            <div>
              <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">代理跳板配置</span>
              
              <!-- Proxy Type Selection -->
              <div class="mb-4">
                <span class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-2">代理类型</span>
                <div class="grid grid-cols-4 gap-2">
                  <button
                    type="button"
                    class={`px-3 py-2 text-xs font-medium rounded-md transition-all ${formData.proxyType === 'none' ? 'bg-blue-600 text-white' : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'}`}
                    on:click={() => formData.proxyType = 'none'}
                  >
                    无代理
                  </button>
                  <button
                    type="button"
                    class={`px-3 py-2 text-xs font-medium rounded-md transition-all ${formData.proxyType === 'socks5' ? 'bg-blue-600 text-white' : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'}`}
                    on:click={() => formData.proxyType = 'socks5'}
                  >
                    SOCKS5
                  </button>
                  <button
                    type="button"
                    class={`px-3 py-2 text-xs font-medium rounded-md transition-all ${formData.proxyType === 'http' ? 'bg-blue-600 text-white' : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'}`}
                    on:click={() => formData.proxyType = 'http'}
                  >
                    HTTP
                  </button>
                  <button
                    type="button"
                    class={`px-3 py-2 text-xs font-medium rounded-md transition-all ${formData.proxyType === 'jumpHost' ? 'bg-blue-600 text-white' : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'}`}
                    on:click={() => formData.proxyType = 'jumpHost'}
                  >
                    Jump Host
                  </button>
                </div>
              </div>
              
              <!-- Proxy Details -->
              {#if formData.proxyType !== 'none'}
                <div class="space-y-4 p-4 bg-slate-50 dark:bg-slate-900/50 rounded-lg border border-slate-200 dark:border-slate-800">
                  {#if formData.proxyType === 'socks5' || formData.proxyType === 'http'}
                    <!-- SOCKS5/HTTP Proxy Configuration -->
                    <div class="grid grid-cols-12 gap-4">
                      <div class="col-span-8">
                        <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="proxyHost">代理主机</label>
                        <input
                          type="text"
                          id="proxyHost"
                          bind:value={formData.proxyHost}
                          class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                          placeholder="proxy.example.com"
                        />
                      </div>
                      <div class="col-span-4">
                        <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="proxyPort">代理端口</label>
                        <input
                          type="number"
                          id="proxyPort"
                          bind:value={formData.proxyPort}
                          class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                          placeholder="1080"
                          min="1"
                          max="65535"
                        />
                      </div>
                    </div>
                    
                    <!-- Proxy Authentication -->
                    <div>
                      <span class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-2">代理认证 (可选)</span>
                      <div class="grid grid-cols-12 gap-4">
                        <div class="col-span-6">
                          <input
                            type="text"
                            bind:value={formData.proxyUsername}
                            class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                            placeholder="用户名"
                          />
                        </div>
                        <div class="col-span-6">
                          <input
                            type="password"
                            bind:value={formData.proxyPassword}
                            class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                            placeholder="密码"
                          />
                        </div>
                      </div>
                    </div>
                    
                    <!-- SOCKS Proxy Port (Dynamic Port Forwarding) -->
                    {#if formData.proxyType === 'socks5'}
                      <div>
                        <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="socksProxyPort">动态端口转发 (SOCKS 代理端口)</label>
                        <div class="flex items-center gap-2">
                          <input
                            type="number"
                            id="socksProxyPort"
                            bind:value={formData.socksProxyPort}
                            class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                            placeholder="1080 (留空则不启用)"
                            min="1024"
                            max="65535"
                          />
                          <div class="text-xs text-slate-500">
                            使用 SSH -D 选项创建本地 SOCKS5 代理
                          </div>
                        </div>
                      </div>
                    {/if}
                  {:else if formData.proxyType === 'jumpHost'}
                    <!-- Jump Host Configuration -->
                    <div class="space-y-4">
                      <div class="grid grid-cols-12 gap-4">
                        <div class="col-span-8">
                          <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="proxyHost">跳板主机</label>
                          <input
                            type="text"
                            id="proxyHost"
                            bind:value={formData.proxyHost}
                            class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                            placeholder="bastion.example.com"
                          />
                        </div>
                        <div class="col-span-4">
                          <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="proxyPort">跳板端口</label>
                          <input
                            type="number"
                            id="proxyPort"
                            bind:value={formData.proxyPort}
                            class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                            placeholder="22"
                            min="1"
                            max="65535"
                          />
                        </div>
                      </div>
                      
                      <div>
                        <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="jumpHostUsername">跳板用户名</label>
                        <input
                          type="text"
                          id="jumpHostUsername"
                          bind:value={formData.jumpHostUsername}
                          class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                          placeholder="用户名"
                        />
                      </div>
                      
                      <!-- Jump Host Authentication Method -->
                      <div>
                        <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-2" for="jumpHostAuthMethod">跳板认证方式</label>
                        <select
                          id="jumpHostAuthMethod"
                          bind:value={formData.jumpHostAuthMethod}
                          class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                        >
                          <option value="password">密码认证</option>
                          <option value="keyboardInteractive">MFA (键盘交互)</option>
                          <option value="privateKey">私钥认证</option>
                          <option value="agent">SSH Agent</option>
                          <option value="certificate">证书认证</option>
                        </select>
                      </div>

                      <div class="bg-white/70 dark:bg-slate-950/40 rounded-lg p-3 border border-slate-200 dark:border-slate-800">
                        {#if formData.jumpHostAuthMethod === 'password'}
                          <div>
                            <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="jumpHostPassword">跳板密码</label>
                            <input
                              type="password"
                              id="jumpHostPassword"
                              bind:value={formData.jumpHostPassword}
                              class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                              placeholder="密码"
                            />
                            <label class="flex items-center mt-2 cursor-pointer">
                              <input type="checkbox" bind:checked={formData.jumpHostSavePassword} class="rounded border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-blue-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                              <span class="ml-2 text-xs text-slate-500 dark:text-slate-400">保存密码</span>
                            </label>
                          </div>
                        {:else if formData.jumpHostAuthMethod === 'keyboardInteractive'}
                          <div class="text-xs text-slate-500 dark:text-slate-400">
                            连接跳板时会弹出交互式认证提示（MFA），输入内容不会被保存。
                          </div>
                        {:else if formData.jumpHostAuthMethod === 'privateKey'}
                          <div class="space-y-3">
                            <div>
                              <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="jumpHostKeyPath">跳板私钥路径</label>
                              <input
                                type="text"
                                id="jumpHostKeyPath"
                                bind:value={formData.jumpHostKeyPath}
                                class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none font-mono"
                                placeholder="~/.ssh/id_rsa"
                              />
                            </div>
                            <div>
                              <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="jumpHostPassphrase">密码短语 (可选)</label>
                              <input
                                type="password"
                                id="jumpHostPassphrase"
                                bind:value={formData.jumpHostPassphrase}
                                class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                              />
                              <label class="flex items-center mt-2 cursor-pointer">
                                <input type="checkbox" bind:checked={formData.jumpHostSavePassphrase} class="rounded border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-blue-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                                <span class="ml-2 text-xs text-slate-500 dark:text-slate-400">保存密码短语</span>
                              </label>
                            </div>
                          </div>
                        {:else if formData.jumpHostAuthMethod === 'agent'}
                          <div>
                            <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="jumpHostAgentPath">Agent 路径 (可选)</label>
                            <input
                              type="text"
                              id="jumpHostAgentPath"
                              bind:value={formData.jumpHostAgentPath}
                              class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none font-mono"
                              placeholder="默认使用系统 SSH_AUTH_SOCK"
                            />
                          </div>
                        {:else if formData.jumpHostAuthMethod === 'certificate'}
                          <div class="space-y-3">
                            <div>
                              <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="jumpHostCertificatePath">跳板证书路径</label>
                              <input
                                type="text"
                                id="jumpHostCertificatePath"
                                bind:value={formData.jumpHostCertificatePath}
                                class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none font-mono"
                                placeholder="~/.ssh/id_rsa-cert.pub"
                              />
                            </div>
                            <div>
                              <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="jumpHostPrivateKeyPath">跳板私钥路径</label>
                              <input
                                type="text"
                                id="jumpHostPrivateKeyPath"
                                bind:value={formData.jumpHostPrivateKeyPath}
                                class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none font-mono"
                                placeholder="~/.ssh/id_rsa"
                              />
                            </div>
                            <div>
                              <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="jumpHostPassphraseCert">密码短语 (可选)</label>
                              <input
                                type="password"
                                id="jumpHostPassphraseCert"
                                bind:value={formData.jumpHostPassphrase}
                                class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                              />
                              <label class="flex items-center mt-2 cursor-pointer">
                                <input type="checkbox" bind:checked={formData.jumpHostSavePassphrase} class="rounded border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-blue-600 focus:ring-blue-600 ring-offset-white dark:ring-offset-slate-900">
                                <span class="ml-2 text-xs text-slate-500 dark:text-slate-400">保存密码短语</span>
                              </label>
                            </div>
                          </div>
                        {/if}
                      </div>
                    </div>
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        {:else if activeTab === 'advanced'}
          <div in:slide={{ duration: 200 }} class="space-y-6">
            <!-- Port Forwarding -->
            <div>
              <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">端口转发 (SSH Tunnel)</span>
              
              <!-- Local Forwarding -->
              <div class="mb-6">
                <div class="flex items-center justify-between mb-2">
                  <span class="text-xs font-medium text-slate-500 dark:text-slate-400">本地转发 (Local) - 将本地端口转发到远程服务器</span>
                </div>
                
                <div class="space-y-2 mb-3">
                  {#each formData.localForwards as forward, i}
                    <div class="flex items-center gap-2 bg-slate-50 dark:bg-slate-900/50 p-2 rounded border border-slate-200 dark:border-slate-800">
                      <div class="flex-1 text-xs font-mono text-slate-600 dark:text-slate-300">
                        <span class="text-green-500">Local:{forward.local_port}</span> <span class="text-slate-400">-></span> <span class="text-blue-500">{forward.remote_host}:{forward.remote_port}</span>
                      </div>
                      <button type="button" on:click={() => removeLocalForward(i)} class="text-slate-400 hover:text-red-500">
                        <XIcon class="w-3 h-3" />
                      </button>
                    </div>
                  {/each}
                </div>

                <div class="grid grid-cols-12 gap-2">
                  <div class="col-span-3">
                    <input
                      type="number"
                      bind:value={newLocalForward.local_port}
                      placeholder="本地端口"
                      class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-xs text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                    />
                  </div>
                  <div class="col-span-4">
                    <input
                      type="text"
                      bind:value={newLocalForward.remote_host}
                      placeholder="目标主机 (如: localhost)"
                      class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-xs text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                    />
                  </div>
                  <div class="col-span-3">
                    <input
                      type="number"
                      bind:value={newLocalForward.remote_port}
                      placeholder="目标端口"
                      class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-xs text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                    />
                  </div>
                  <div class="col-span-2">
                    <button
                      type="button"
                      on:click={addLocalForward}
                      class="w-full h-full flex items-center justify-center bg-blue-600 hover:bg-blue-500 text-white rounded text-xs transition-colors"
                    >
                      添加
                    </button>
                  </div>
                </div>
              </div>

              <!-- Remote Forwarding -->
              <div>
                <div class="flex items-center justify-between mb-2">
                  <span class="text-xs font-medium text-slate-500 dark:text-slate-400">远程转发 (Remote) - 将远程端口转发到本地</span>
                </div>

                <div class="space-y-2 mb-3">
                  {#each formData.remoteForwards as forward, i}
                    <div class="flex items-center gap-2 bg-slate-50 dark:bg-slate-900/50 p-2 rounded border border-slate-200 dark:border-slate-800">
                      <div class="flex-1 text-xs font-mono text-slate-600 dark:text-slate-300">
                        <span class="text-blue-500">Remote:{forward.remote_port}</span> <span class="text-slate-400">-></span> <span class="text-green-500">{forward.local_host}:{forward.local_port}</span>
                      </div>
                      <button type="button" on:click={() => removeRemoteForward(i)} class="text-slate-400 hover:text-red-500">
                        <XIcon class="w-3 h-3" />
                      </button>
                    </div>
                  {/each}
                </div>

                <div class="grid grid-cols-12 gap-2">
                  <div class="col-span-3">
                    <input
                      type="number"
                      bind:value={newRemoteForward.remote_port}
                      placeholder="远程端口"
                      class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-xs text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                    />
                  </div>
                  <div class="col-span-4">
                    <input
                      type="text"
                      bind:value={newRemoteForward.local_host}
                      placeholder="本地主机 (如: localhost)"
                      class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-xs text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                    />
                  </div>
                  <div class="col-span-3">
                    <input
                      type="number"
                      bind:value={newRemoteForward.local_port}
                      placeholder="本地端口"
                      class="w-full bg-slate-50 dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-2 py-1.5 text-xs text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                    />
                  </div>
                  <div class="col-span-2">
                    <button
                      type="button"
                      on:click={addRemoteForward}
                      class="w-full h-full flex items-center justify-center bg-blue-600 hover:bg-blue-500 text-white rounded text-xs transition-colors"
                    >
                      添加
                    </button>
                  </div>
                </div>
              </div>
            </div>
            
            <!-- Proxy Configuration -->
            <div>
              <span class="block text-sm font-medium text-slate-600 dark:text-slate-400 mb-3">代理跳板配置</span>
              
              <!-- Proxy Type Selection -->
              <div class="mb-4">
                <span class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-2">代理类型</span>
                <div class="grid grid-cols-4 gap-2">
                  <button
                    type="button"
                    class={`px-3 py-2 text-xs font-medium rounded-md transition-all ${formData.proxyType === 'none' ? 'bg-blue-600 text-white' : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'}`}
                    on:click={() => formData.proxyType = 'none'}
                  >
                    无代理
                  </button>
                  <button
                    type="button"
                    class={`px-3 py-2 text-xs font-medium rounded-md transition-all ${formData.proxyType === 'socks5' ? 'bg-blue-600 text-white' : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'}`}
                    on:click={() => formData.proxyType = 'socks5'}
                  >
                    SOCKS5
                  </button>
                  <button
                    type="button"
                    class={`px-3 py-2 text-xs font-medium rounded-md transition-all ${formData.proxyType === 'http' ? 'bg-blue-600 text-white' : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'}`}
                    on:click={() => formData.proxyType = 'http'}
                  >
                    HTTP
                  </button>
                  <button
                    type="button"
                    class={`px-3 py-2 text-xs font-medium rounded-md transition-all ${formData.proxyType === 'jumpHost' ? 'bg-blue-600 text-white' : 'bg-slate-100 dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-200 dark:hover:bg-slate-700'}`}
                    on:click={() => formData.proxyType = 'jumpHost'}
                  >
                    Jump Host
                  </button>
                </div>
              </div>
              
              <!-- Proxy Details -->
              {#if formData.proxyType !== 'none'}
                <div class="space-y-4 p-4 bg-slate-50 dark:bg-slate-900/50 rounded-lg border border-slate-200 dark:border-slate-800">
                  {#if formData.proxyType === 'socks5' || formData.proxyType === 'http'}
                    <!-- SOCKS5/HTTP Proxy Configuration -->
                    <div class="grid grid-cols-12 gap-4">
                      <div class="col-span-8">
                        <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="proxyHost">代理主机</label>
                        <input
                          type="text"
                          id="proxyHost"
                          bind:value={formData.proxyHost}
                          class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                          placeholder="proxy.example.com"
                        />
                      </div>
                      <div class="col-span-4">
                        <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="proxyPort">代理端口</label>
                        <input
                          type="number"
                          id="proxyPort"
                          bind:value={formData.proxyPort}
                          class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                          placeholder="1080"
                          min="1"
                          max="65535"
                        />
                      </div>
                    </div>
                    
                    <!-- Proxy Authentication -->
                    <div>
                      <span class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-2">代理认证 (可选)</span>
                      <div class="grid grid-cols-12 gap-4">
                        <div class="col-span-6">
                          <input
                            type="text"
                            bind:value={formData.proxyUsername}
                            class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                            placeholder="用户名"
                          />
                        </div>
                        <div class="col-span-6">
                          <input
                            type="password"
                            bind:value={formData.proxyPassword}
                            class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                            placeholder="密码"
                          />
                        </div>
                      </div>
                    </div>
                    
                    <!-- SOCKS Proxy Port (Dynamic Port Forwarding) -->
                    {#if formData.proxyType === 'socks5'}
                      <div>
                        <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="socksProxyPort">动态端口转发 (SOCKS 代理端口)</label>
                        <div class="flex items-center gap-2">
                          <input
                            type="number"
                            id="socksProxyPort"
                            bind:value={formData.socksProxyPort}
                            class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                            placeholder="1080 (留空则不启用)"
                            min="1024"
                            max="65535"
                          />
                          <div class="text-xs text-slate-500">
                            使用 SSH -D 选项创建本地 SOCKS5 代理
                          </div>
                        </div>
                      </div>
                    {/if}
                  {:else if formData.proxyType === 'jumpHost'}
                    <!-- Jump Host Configuration -->
                    <div class="space-y-4">
                      <div class="grid grid-cols-12 gap-4">
                        <div class="col-span-8">
                          <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="proxyHost">跳板主机</label>
                          <input
                            type="text"
                            id="proxyHost"
                            bind:value={formData.proxyHost}
                            class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                            placeholder="bastion.example.com"
                          />
                        </div>
                        <div class="col-span-4">
                          <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="proxyPort">跳板端口</label>
                          <input
                            type="number"
                            id="proxyPort"
                            bind:value={formData.proxyPort}
                            class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                            placeholder="22"
                            min="1"
                            max="65535"
                          />
                        </div>
                      </div>
                      
                      <div>
                        <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-1.5" for="jumpHostUsername">跳板用户名</label>
                        <input
                          type="text"
                          id="jumpHostUsername"
                          bind:value={formData.jumpHostUsername}
                          class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                          placeholder="用户名"
                        />
                      </div>
                      
                      <!-- Jump Host Authentication Method -->
                      <div>
                        <label class="block text-xs font-medium text-slate-500 dark:text-slate-400 mb-2" for="jumpHostAuthMethod">跳板认证方式</label>
                        <select
                          id="jumpHostAuthMethod"
                          bind:value={formData.jumpHostAuthMethod}
                          class="w-full bg-white dark:bg-slate-950 border border-slate-300 dark:border-slate-700 rounded px-3 py-2 text-sm text-slate-900 dark:text-slate-200 focus:border-blue-500 outline-none"
                        >
                          <option value="password">密码认证</option>
                          <option value="privateKey">私钥认证</option>
                          <option value="agent">SSH Agent</option>
                          <option value="certificate">证书认证</option>
                        </select>
                        <div class="text-xs text-slate-500 mt-1">
                          注意：跳板主机的认证需要单独配置
                        </div>
                      </div>
                    </div>
                  {/if}
                </div>
              {/if}
            </div>
          </div>
        {/if}
      </form>
    </div>

    <!-- Footer -->
    <div class="px-6 py-4 border-t border-slate-800 bg-slate-900 flex justify-end gap-3">
      <button
        type="button"
        class="px-4 py-2 text-sm font-medium text-slate-300 hover:text-white bg-slate-800 hover:bg-slate-700 rounded-lg transition-colors"
        on:click={handleClose}
      >
        取消
      </button>
      <button
        type="submit"
        form="connection-form"
        disabled={isSaving}
        class="px-4 py-2 text-sm font-medium text-white bg-blue-600 hover:bg-blue-500 active:bg-blue-700 rounded-lg transition-colors shadow-lg shadow-blue-900/20 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
      >
        {#if isSaving}
          <div class="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
          <span>处理中...</span>
        {:else}
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"></path></svg>
          <span>保存并连接</span>
        {/if}
      </button>
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
