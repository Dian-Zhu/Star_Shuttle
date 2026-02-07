<script lang="ts">
  import { showConnectionForm, editingConnection, connections, type Connection, errorMessage, successMessage } from '../lib/store';
  import { saveConnection, createBackendConfig } from '../lib/connectionService';
  import { connectAndOpen } from '../lib/terminalService';
  import { invoke } from '@tauri-apps/api/core';
  import XIcon from './icons/XIcon.svelte';
  import PlusIcon from './icons/PlusIcon.svelte';
  import ActivityIcon from './icons/ActivityIcon.svelte';
  import { slide, fade } from 'svelte/transition';

  // Form state
  let formData = {
    id: '',
    name: '',
    protocol: 'Ssh' as 'Ssh' | 'Rdp' | 'Telnet',
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

  function normalizeTagsValue(value: unknown): string[] {
    if (Array.isArray(value)) {
      return value.map(v => String(v).trim()).filter(Boolean);
    }
    if (typeof value === 'string') {
      return value
        .split(/[,\uFF0C;\uFF1B\n]+/g)
        .map(tag => tag.trim())
        .filter(Boolean);
    }
    return [];
  }

  function hydrateFromConnection(connection: Connection) {
    formData.id = connection.id;
    formData.name = connection.name ?? '';
    formData.protocol = ((connection as any).protocol ?? 'Ssh') as any;
    formData.host = connection.host ?? '';
    formData.port = Number(connection.port ?? 22);
    formData.username = connection.username ?? '';
    formData.description = connection.description ?? '';
    formData.tags = normalizeTagsValue((connection as any).tags).join(',');

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
  let lastProtocol: 'Ssh' | 'Rdp' | 'Telnet' = formData.protocol;

  $: if (formData.protocol !== lastProtocol) {
    if (formData.protocol === 'Rdp' && (formData.port === 22 || formData.port === 23)) formData.port = 3389;
    else if (formData.protocol === 'Telnet' && (formData.port === 22 || formData.port === 3389)) formData.port = 23;
    else if (formData.protocol === 'Ssh' && (formData.port === 23 || formData.port === 3389)) formData.port = 22;
    lastProtocol = formData.protocol;
  }

  $: if (formData.protocol !== 'Ssh' && activeTab === 'advanced') {
    activeTab = 'basic';
  }

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

  function commitPendingTag() {
    if (tagInput.trim()) {
      addTag(tagInput);
    }
  }

  function trimHost() {
    formData.host = (formData.host ?? '').trim();
  }

  // Temporary variables for adding new forwards
  let newLocalForward = { local_host: 'localhost', local_port: 0, remote_host: 'localhost', remote_port: 0 };
  let newRemoteForward = { remote_host: 'localhost', remote_port: 0, local_host: 'localhost', local_port: 0 };

  let isSaving = false;
  let isTesting = false;
  
  let hostKeyVerification: { 
    host: string; 
    port: number; 
    keyType: string; 
    keyBase64: string; 
    fingerprint: string; 
  } | null = null;

  $: if ($editingConnection) {
    hydrateFromConnection($editingConnection);
  }

  async function acceptHostKey() {
    if (!hostKeyVerification) return;
    
    try {
      await invoke('known_hosts_save_host_key', {
        host: hostKeyVerification.host,
        port: hostKeyVerification.port,
        keyType: hostKeyVerification.keyType,
        keyBase64: hostKeyVerification.keyBase64,
        replace: false
      });
      
      hostKeyVerification = null;
      successMessage.set('主机密钥已保存');
      setTimeout(() => successMessage.set(null), 3000);
      
      // Retry connection test
      handleTestConnection();
    } catch (error) {
      console.error('Failed to save host key:', error);
      errorMessage.set(`保存主机密钥失败: ${error}`);
      setTimeout(() => errorMessage.set(null), 5000);
    }
  }

  async function handleTestConnection() {
    commitPendingTag();
    trimHost();
    if (!formData.host || !formData.port) {
      errorMessage.set('请填写主机地址和端口');
      setTimeout(() => errorMessage.set(null), 3000);
      return;
    }
    
    isTesting = true;
    try {
      const config = await createBackendConfig({
        ...formData,
        local_forwards: formData.localForwards,
        remote_forwards: formData.remoteForwards,
      });
      
      await invoke('test_connection', { config });
      successMessage.set('连接测试成功！');
      setTimeout(() => successMessage.set(null), 3000);
    } catch (error: any) {
      const errStr = String(error);
      if (errStr.includes('HOST_KEY_UNKNOWN|')) {
        try {
          const jsonStr = errStr.split('HOST_KEY_UNKNOWN|')[1];
          const keyData = JSON.parse(jsonStr);
          hostKeyVerification = {
            host: keyData.host,
            port: keyData.port,
            keyType: keyData.key_type,
            keyBase64: keyData.key_base64,
            fingerprint: keyData.fingerprint
          };
          return;
        } catch (e) {
          console.error('Failed to parse host key data', e);
        }
      }

      console.error('Connection test failed:', error);
      errorMessage.set(`连接测试失败: ${error}`);
      setTimeout(() => errorMessage.set(null), 5000);
    } finally {
      isTesting = false;
    }
  }

  async function handleSubmit() {
    commitPendingTag();
    trimHost();
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

<div class="fixed inset-0 z-50 flex items-center justify-center bg-app-backdrop backdrop-blur-sm p-4" role="button" tabindex="0" on:click|self={handleClose} on:keydown={(e) => e.key === 'Escape' && handleClose()}>
  <div class="relative bg-app-surface border border-app-border rounded-xl shadow-2xl w-full max-w-2xl max-h-[90vh] flex flex-col overflow-hidden">
    <!-- Host Key Verification Overlay -->
    {#if hostKeyVerification}
      <div class="absolute inset-0 z-50 bg-app-surface flex flex-col items-center justify-center p-8 text-center" transition:fade={{ duration: 200 }}>
        <div class="w-16 h-16 bg-yellow-100 dark:bg-yellow-900/30 text-yellow-600 dark:text-yellow-400 rounded-full flex items-center justify-center mb-4">
          <svg class="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"></path></svg>
        </div>
        <h3 class="text-xl font-semibold text-app-text mb-2">未知的主机密钥</h3>
        <p class="text-sm text-app-text-secondary mb-6 max-w-md">
          服务器 <strong>{hostKeyVerification.host}:{hostKeyVerification.port}</strong> 的身份无法验证。
          <br>
          指纹: <code class="bg-app-bg px-1 py-0.5 rounded text-xs font-mono select-all mt-2 inline-block">{hostKeyVerification.fingerprint}</code>
        </p>
        <div class="flex items-center gap-3">
          <button
            type="button"
            class="px-4 py-2 text-sm font-medium text-app-text-secondary hover:text-app-text bg-app-bg hover:bg-app-bg-hover rounded-lg transition-colors"
            on:click={() => hostKeyVerification = null}
          >
            取消
          </button>
          <button
            type="button"
            class="px-4 py-2 text-sm font-medium text-white bg-primary-600 hover:bg-primary-500 rounded-lg transition-colors shadow-lg shadow-primary-900/20"
            on:click={acceptHostKey}
          >
            接受并保存
          </button>
        </div>
      </div>
    {/if}

    <!-- Header -->
    <div class="flex items-center justify-between px-6 py-4 border-b border-app-border bg-app-bg">
      <div class="flex items-center gap-4">
        <h2 class="text-lg font-semibold text-app-text">{$editingConnection ? '编辑连接' : '新建连接'}</h2>
        <!-- Tabs -->
        <div class="flex bg-app-bg rounded-lg p-1 border border-app-border">
          <button 
            class="px-3 py-1 text-xs font-medium rounded-md transition-all {activeTab === 'basic' ? 'bg-app-surface text-primary-600 dark:text-primary-400 shadow-sm' : 'text-app-text-secondary hover:text-app-text'}"
            on:click={() => activeTab = 'basic'}
          >
            基本信息
          </button>
          <button 
            class="px-3 py-1 text-xs font-medium rounded-md transition-all {formData.protocol !== 'Ssh' ? 'opacity-40 cursor-not-allowed' : ''} {activeTab === 'advanced' ? 'bg-app-surface text-primary-600 dark:text-primary-400 shadow-sm' : 'text-app-text-secondary hover:text-app-text'}"
            disabled={formData.protocol !== 'Ssh'}
            on:click={() => activeTab = 'advanced'}
          >
            高级 & 隧道
          </button>
        </div>
      </div>
      <button 
        class="text-app-text-secondary hover:text-app-text transition-colors p-1 rounded-md hover:bg-app-bg-hover"
        on:click={handleClose}
      >
        <XIcon class="w-5 h-5" />
      </button>
    </div>

    <!-- Scrollable Content -->
    <div class="flex-1 overflow-y-auto p-6 custom-scrollbar">
      <form id="connection-form" on:submit|preventDefault={handleSubmit} class="space-y-6">
        {#if activeTab === 'basic'}
          <div in:slide={{ duration: 200 }} class="space-y-5 p-1">
            
            <!-- Connection Details (Compact) -->
            <div class="space-y-4">
              <!-- Row 0: Protocol (Radio Buttons) -->
              <div class="mb-4">
                <span class="block text-xs font-medium text-app-text-secondary mb-2">协议类型</span>
                <div class="flex items-center gap-4">
                  {#each ['Ssh', 'Rdp', 'Telnet'] as proto}
                    <label class="flex items-center gap-2 cursor-pointer group">
                      <div class="relative flex items-center justify-center w-4 h-4">
                        <input 
                          type="radio" 
                          name="protocol" 
                          value={proto} 
                          bind:group={formData.protocol} 
                          class="peer appearance-none w-4 h-4 rounded-full border border-app-border checked:border-primary-500 checked:bg-primary-500 transition-all"
                        />
                        <div class="absolute w-1.5 h-1.5 rounded-full bg-white scale-0 peer-checked:scale-100 transition-transform"></div>
                      </div>
                      <span class="text-sm text-app-text group-hover:text-primary-500 transition-colors">{proto.toUpperCase()}</span>
                    </label>
                  {/each}
                </div>
              </div>

              <!-- Row 1: Name & Tags -->
              <div class="grid grid-cols-12 gap-4">
                <div class="col-span-6">
                  <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="name">连接名称 <span class="text-red-500">*</span></label>
                  <input
                    type="text"
                    id="name"
                    bind:value={formData.name}
                    class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                    placeholder="例如: 生产环境服务器"
                    required
                  />
                </div>
                <div class="col-span-6">
                   <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="tags">标签</label>
                   <div class="w-full bg-app-bg border border-app-border rounded-lg px-2 py-1.5 flex flex-wrap gap-2 min-h-[38px] focus-within:border-primary-500 focus-within:ring-1 focus-within:ring-primary-500 transition-all">
                    {#each currentTags as tag}
                      <span class="bg-primary-100 dark:bg-primary-900/30 text-primary-700 dark:text-primary-300 text-xs px-2 py-0.5 rounded-md flex items-center gap-1">
                        {tag}
                        <button type="button" on:click={() => removeTag(tag)} class="hover:text-primary-900 dark:hover:text-primary-100">
                          <XIcon class="w-3 h-3" />
                        </button>
                      </span>
                    {/each}
                    <input
                      type="text"
                      bind:value={tagInput}
                      on:keydown={handleTagKeydown}
                      on:blur={commitPendingTag}
                      class="bg-transparent border-none outline-none text-sm min-w-[60px] flex-1 text-app-text placeholder-app-text-secondary/50 p-0"
                      placeholder={currentTags.length === 0 ? "标签..." : ""}
                    />
                  </div>
                  <!-- Suggested Tags -->
                  {#if availableTags.length > 0}
                    <div class="flex flex-wrap gap-2 mt-2">
                      {#each availableTags as tag}
                         <button 
                           type="button" 
                           class="text-[10px] px-1.5 py-0.5 rounded-md bg-app-surface border border-app-border text-app-text-secondary hover:bg-app-bg-hover transition-colors"
                           on:click={() => addTag(tag)}
                         >
                           {tag}
                         </button>
                      {/each}
                    </div>
                  {/if}
                </div>
              </div>

              <!-- Row 2: Host, Port, Username -->
              <div class="grid grid-cols-12 gap-4">
                <div class="col-span-6">
                  <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="host">主机地址 <span class="text-red-500">*</span></label>
                  <div class="relative">
                    <input
                      type="text"
                      id="host"
                      bind:value={formData.host}
                      on:blur={trimHost}
                      class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all font-mono"
                      placeholder="192.168.1.1"
                      required
                    />
                  </div>
                </div>
                <div class="col-span-2">
                  <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="port">端口 <span class="text-red-500">*</span></label>
                  <input
                    type="number"
                    id="port"
                    bind:value={formData.port}
                    class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all font-mono"
                    min="1"
                    max="65535"
                    required
                  />
                </div>
                <div class="col-span-4">
                  <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="username">
                    {formData.protocol === 'Ssh' ? '用户名' : '用户名 (可选)'}
                  </label>
                  <input
                    type="text"
                    id="username"
                    bind:value={formData.username}
                    class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all font-mono"
                    placeholder={formData.protocol === 'Ssh' ? 'root' : 'Administrator'}
                  />
                </div>
              </div>

              <!-- Tags moved up -->
            </div>

            <div class="border-t border-app-border"></div>

            <!-- Authentication -->
            {#if formData.protocol === 'Ssh'}
            <div class="space-y-3">
              <div class="flex items-center justify-between">
                 <h3 class="text-xs font-medium text-app-text-secondary uppercase tracking-wider">认证方式</h3>
              </div>
              
              <div class="flex p-1 bg-app-bg rounded-lg border border-app-border overflow-x-auto">
                {#each [
                  { id: 'password', label: '密码' },
                  { id: 'privateKey', label: '私钥' },
                  { id: 'keyboardInteractive', label: 'MFA' },
                  { id: 'agent', label: 'Agent' },
                  { id: 'certificate', label: '证书' }
                ] as method}
                  <button
                    type="button"
                    class="flex-1 whitespace-nowrap px-3 py-1.5 text-xs rounded-md transition-all {formData.authMethod === method.id ? 'bg-app-surface text-primary-600 shadow-sm font-medium' : 'text-app-text-secondary hover:text-app-text'}"
                    on:click={() => formData.authMethod = method.id as any}
                  >
                    {method.label}
                  </button>
                {/each}
              </div>

              <div class="pt-1">
                {#if formData.authMethod === 'password'}
                  <div>
                    <input
                      type="password"
                      id="password"
                      bind:value={formData.password}
                      class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                      placeholder="请输入密码"
                    />
                    <label class="flex items-center mt-2 cursor-pointer">
                      <input type="checkbox" bind:checked={formData.savePassword} class="rounded border-app-border bg-app-surface text-primary-600 focus:ring-primary-600 ring-offset-app-surface">
                      <span class="ml-2 text-xs text-app-text-secondary">保存密码</span>
                    </label>
                  </div>
                {:else if formData.authMethod === 'keyboardInteractive'}
                  <div class="text-sm text-app-text-secondary flex items-center gap-2 p-2 bg-app-bg rounded-lg border border-app-border border-dashed">
                    <ActivityIcon class="w-4 h-4" />
                    连接时会弹出交互式认证提示（MFA），输入内容不会被保存。
                  </div>
                {:else if formData.authMethod === 'privateKey'}
                  <div class="space-y-3">
                    <div class="grid grid-cols-12 gap-3">
                      <div class="col-span-12">
                         <input
                          type="text"
                          bind:value={formData.keyPath}
                          class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all font-mono"
                          placeholder="私钥路径 (~/.ssh/id_rsa)"
                        />
                      </div>
                      <div class="col-span-12">
                        <input
                          type="password"
                          bind:value={formData.passphrase}
                          class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                          placeholder="密码短语 (可选)"
                        />
                        <label class="flex items-center mt-2 cursor-pointer">
                          <input type="checkbox" bind:checked={formData.savePassphrase} class="rounded border-app-border bg-app-surface text-primary-600 focus:ring-primary-600 ring-offset-app-surface">
                          <span class="ml-2 text-xs text-app-text-secondary">保存密码短语</span>
                        </label>
                      </div>
                    </div>
                  </div>
                {:else if formData.authMethod === 'agent'}
                  <div>
                    <input
                      type="text"
                      bind:value={formData.agentPath}
                      class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all font-mono"
                      placeholder="Agent 路径 (可选，默认系统 SSH_AUTH_SOCK)"
                    />
                  </div>
                {:else if formData.authMethod === 'certificate'}
                  <div class="space-y-3">
                    <input
                        type="text"
                        bind:value={formData.certificatePath}
                        class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all font-mono"
                        placeholder="证书路径 (~/.ssh/id_rsa-cert.pub)"
                    />
                    <input
                        type="text"
                        bind:value={formData.privateKeyPath}
                        class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all font-mono"
                        placeholder="私钥路径 (~/.ssh/id_rsa)"
                    />
                    <div>
                        <input
                          type="password"
                          bind:value={formData.passphrase}
                          class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                          placeholder="密码短语 (可选)"
                        />
                        <label class="flex items-center mt-2 cursor-pointer">
                          <input type="checkbox" bind:checked={formData.savePassphrase} class="rounded border-app-border bg-app-surface text-primary-600 focus:ring-primary-600 ring-offset-app-surface">
                          <span class="ml-2 text-xs text-app-text-secondary">保存密码短语</span>
                        </label>
                    </div>
                  </div>
                {/if}
              </div>
            </div>
            {/if}

            <div class="border-t border-app-border"></div>

            <!-- Description -->
            <div>
               <textarea
                 bind:value={formData.description}
                 rows="2"
                 class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all resize-none"
                 placeholder="备注信息 (可选)..."
               ></textarea>
            </div>
            
          </div>
        {:else if activeTab === 'advanced'}
          <div in:slide={{ duration: 200 }} class="space-y-5">
            <!-- Port Forwarding -->
            <div>
              <h3 class="text-xs font-medium text-app-text-secondary uppercase tracking-wider mb-3">端口转发 (SSH Tunnel)</h3>
              
              <div class="space-y-4">
                <!-- Local Forwarding -->
                <div>
                  <div class="flex items-center justify-between mb-2">
                    <span class="text-xs font-medium text-app-text-secondary">本地转发 (Local) - 将本地端口转发到远程服务器</span>
                  </div>
                  
                  {#if formData.localForwards.length > 0}
                    <div class="space-y-2 mb-3">
                      {#each formData.localForwards as forward, i}
                        <div class="flex items-center gap-2 bg-app-bg p-2 rounded-lg border border-app-border group/item hover:border-primary-500/50 transition-colors">
                          <div class="flex-1 text-xs font-mono text-app-text-secondary flex items-center gap-2">
                            <span class="bg-green-500/10 text-green-600 dark:text-green-400 px-1.5 py-0.5 rounded">Local:{forward.local_port}</span>
                            <span class="text-app-text-secondary opacity-50">→</span>
                            <span class="bg-primary-500/10 text-primary-600 dark:text-primary-400 px-1.5 py-0.5 rounded">{forward.remote_host}:{forward.remote_port}</span>
                          </div>
                          <button type="button" on:click={() => removeLocalForward(i)} class="text-app-text-secondary hover:text-red-500 p-1 rounded-md hover:bg-app-surface transition-colors opacity-0 group-hover/item:opacity-100">
                            <XIcon class="w-3.5 h-3.5" />
                          </button>
                        </div>
                      {/each}
                    </div>
                  {/if}

                  <div class="grid grid-cols-12 gap-2">
                    <div class="col-span-3">
                      <input
                        type="number"
                        bind:value={newLocalForward.local_port}
                        placeholder="本地端口"
                        class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-xs text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                      />
                    </div>
                    <div class="col-span-4">
                      <input
                        type="text"
                        bind:value={newLocalForward.remote_host}
                        placeholder="目标主机"
                        class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-xs text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                      />
                    </div>
                    <div class="col-span-3">
                      <input
                        type="number"
                        bind:value={newLocalForward.remote_port}
                        placeholder="目标端口"
                        class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-xs text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                      />
                    </div>
                    <div class="col-span-2">
                      <button
                        type="button"
                        on:click={addLocalForward}
                        class="w-full h-full flex items-center justify-center bg-primary-600 hover:bg-primary-500 text-white rounded-lg text-xs font-medium transition-colors shadow-sm"
                      >
                        <PlusIcon class="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                </div>

                <!-- Remote Forwarding -->
                <div>
                  <div class="flex items-center justify-between mb-2">
                    <span class="text-xs font-medium text-app-text-secondary">远程转发 (Remote) - 将远程端口转发到本地</span>
                  </div>

                  {#if formData.remoteForwards.length > 0}
                    <div class="space-y-2 mb-3">
                      {#each formData.remoteForwards as forward, i}
                        <div class="flex items-center gap-2 bg-app-bg p-2 rounded-lg border border-app-border group/item hover:border-primary-500/50 transition-colors">
                          <div class="flex-1 text-xs font-mono text-app-text flex items-center gap-2">
                            <span class="bg-primary-500/10 text-primary-600 dark:text-primary-400 px-1.5 py-0.5 rounded">Remote:{forward.remote_port}</span>
                            <span class="text-app-text-secondary opacity-50">→</span>
                            <span class="bg-green-500/10 text-green-600 dark:text-green-400 px-1.5 py-0.5 rounded">{forward.local_host}:{forward.local_port}</span>
                          </div>
                          <button type="button" on:click={() => removeRemoteForward(i)} class="text-app-text-secondary hover:text-red-500 p-1 rounded-md hover:bg-app-surface transition-colors opacity-0 group-hover/item:opacity-100">
                            <XIcon class="w-3.5 h-3.5" />
                          </button>
                        </div>
                      {/each}
                    </div>
                  {/if}

                  <div class="grid grid-cols-12 gap-2">
                    <div class="col-span-3">
                      <input
                        type="number"
                        bind:value={newRemoteForward.remote_port}
                        placeholder="远程端口"
                        class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-xs text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                      />
                    </div>
                    <div class="col-span-4">
                      <input
                        type="text"
                        bind:value={newRemoteForward.local_host}
                        placeholder="本地主机"
                        class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-xs text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                      />
                    </div>
                    <div class="col-span-3">
                      <input
                        type="number"
                        bind:value={newRemoteForward.local_port}
                        placeholder="本地端口"
                        class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-xs text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                      />
                    </div>
                    <div class="col-span-2">
                      <button
                        type="button"
                        on:click={addRemoteForward}
                        class="w-full h-full flex items-center justify-center bg-primary-600 hover:bg-primary-500 text-white rounded-lg text-xs font-medium transition-colors shadow-sm"
                      >
                         <PlusIcon class="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                </div>
              </div>
            </div>
            
            <div class="border-t border-app-border"></div>

            <!-- Proxy Configuration -->
            <div>
              <h3 class="text-xs font-medium text-app-text-secondary uppercase tracking-wider mb-3">代理跳板配置</h3>
              
              <div class="space-y-4">
                <!-- Proxy Type Selection -->
                <div>
                  <span class="block text-xs font-medium text-app-text-secondary mb-1.5">代理类型</span>
                  <div class="flex p-1 bg-app-bg rounded-lg border border-app-border overflow-x-auto">
                    {#each [
                      { id: 'none', label: '无代理' },
                      { id: 'socks5', label: 'SOCKS5' },
                      { id: 'http', label: 'HTTP' },
                      { id: 'jumpHost', label: 'Jump Host' }
                    ] as type}
                      <button
                        type="button"
                        class="flex-1 whitespace-nowrap px-3 py-1.5 text-xs rounded-md transition-all {formData.proxyType === type.id ? 'bg-app-surface text-primary-600 shadow-sm font-medium' : 'text-app-text-secondary hover:text-app-text'}"
                        on:click={() => formData.proxyType = type.id as any}
                      >
                        {type.label}
                      </button>
                    {/each}
                  </div>
                </div>
                
                <!-- Proxy Details -->
                {#if formData.proxyType !== 'none'}
                  <div in:slide={{ duration: 200 }} class="space-y-4 pt-2">
                    {#if formData.proxyType === 'socks5' || formData.proxyType === 'http'}
                      <!-- SOCKS5/HTTP Proxy Configuration -->
                      <div class="grid grid-cols-12 gap-3">
                        <div class="col-span-8">
                          <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="proxyHost">代理主机</label>
                          <input
                            type="text"
                            id="proxyHost"
                            bind:value={formData.proxyHost}
                            class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                            placeholder="proxy.example.com"
                          />
                        </div>
                        <div class="col-span-4">
                          <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="proxyPort">代理端口</label>
                          <input
                            type="number"
                            id="proxyPort"
                            bind:value={formData.proxyPort}
                            class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                            placeholder="1080"
                            min="1"
                            max="65535"
                          />
                        </div>
                      </div>
                      
                      <!-- Proxy Authentication -->
                      <div class="bg-app-bg/50 rounded-lg p-3 border border-app-border border-dashed">
                        <span class="block text-xs font-medium text-app-text-secondary mb-2">代理认证 (可选)</span>
                        <div class="grid grid-cols-2 gap-3">
                          <input
                            type="text"
                            bind:value={formData.proxyUsername}
                            class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                            placeholder="用户名"
                          />
                          <input
                            type="password"
                            bind:value={formData.proxyPassword}
                            class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                            placeholder="密码"
                          />
                        </div>
                      </div>
                      
                      <!-- SOCKS Proxy Port (Dynamic Port Forwarding) -->
                      {#if formData.proxyType === 'socks5'}
                        <div class="bg-app-bg/50 rounded-lg p-3 border border-app-border border-dashed">
                          <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="socksProxyPort">动态端口转发 (SOCKS 代理端口)</label>
                          <div class="flex items-center gap-2">
                            <input
                              type="number"
                              id="socksProxyPort"
                              bind:value={formData.socksProxyPort}
                              class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                              placeholder="1080 (留空则不启用)"
                              min="1024"
                              max="65535"
                            />
                            <div class="text-xs text-app-text-secondary whitespace-nowrap">
                              SSH -D 本地 SOCKS5
                            </div>
                          </div>
                        </div>
                      {/if}
                    {:else if formData.proxyType === 'jumpHost'}
                      <!-- Jump Host Configuration -->
                      <div class="space-y-4">
                        <div class="grid grid-cols-12 gap-3">
                          <div class="col-span-8">
                            <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpHost">跳板主机</label>
                            <input
                              type="text"
                              id="jumpHost"
                              bind:value={formData.proxyHost}
                              class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                              placeholder="bastion.example.com"
                            />
                          </div>
                          <div class="col-span-4">
                            <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpPort">跳板端口</label>
                            <input
                              type="number"
                              id="jumpPort"
                              bind:value={formData.proxyPort}
                              class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                              placeholder="22"
                              min="1"
                              max="65535"
                            />
                          </div>
                        </div>
                        
                        <div class="grid grid-cols-2 gap-3">
                            <div>
                                <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpHostUsername">跳板用户名</label>
                                <input
                                type="text"
                                id="jumpHostUsername"
                                bind:value={formData.jumpHostUsername}
                                class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                                placeholder="用户名"
                                />
                            </div>
                            
                            <!-- Jump Host Authentication Method -->
                            <div>
                                <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpHostAuthMethod">跳板认证方式</label>
                                <div class="relative">
                                    <select
                                    id="jumpHostAuthMethod"
                                    bind:value={formData.jumpHostAuthMethod}
                                    class="w-full bg-app-bg border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all appearance-none"
                                    >
                                    <option value="password">密码认证</option>
                                    <option value="keyboardInteractive">MFA (键盘交互)</option>
                                    <option value="privateKey">私钥认证</option>
                                    <option value="agent">SSH Agent</option>
                                    <option value="certificate">证书认证</option>
                                    </select>
                                    <div class="absolute inset-y-0 right-0 flex items-center px-2 pointer-events-none text-app-text-secondary">
                                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path></svg>
                                    </div>
                                </div>
                            </div>
                        </div>

                        <div class="bg-app-bg/50 rounded-lg p-4 border border-app-border border-dashed">
                          {#if formData.jumpHostAuthMethod === 'password'}
                            <div>
                              <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpHostPassword">跳板密码</label>
                              <input
                                type="password"
                                id="jumpHostPassword"
                                bind:value={formData.jumpHostPassword}
                                class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                                placeholder="密码"
                              />
                              <label class="flex items-center mt-2 cursor-pointer">
                                <input type="checkbox" bind:checked={formData.jumpHostSavePassword} class="rounded border-app-border bg-app-surface text-primary-600 focus:ring-primary-600 ring-offset-app-surface">
                                <span class="ml-2 text-xs text-app-text-secondary">保存密码</span>
                              </label>
                            </div>
                          {:else if formData.jumpHostAuthMethod === 'keyboardInteractive'}
                            <div class="text-xs text-app-text-secondary">
                              连接跳板时会弹出交互式认证提示（MFA），输入内容不会被保存。
                            </div>
                          {:else if formData.jumpHostAuthMethod === 'privateKey'}
                            <div class="space-y-3">
                              <div>
                                <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpHostKeyPath">跳板私钥路径</label>
                                <input
                                  type="text"
                                  id="jumpHostKeyPath"
                                  bind:value={formData.jumpHostKeyPath}
                                  class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 outline-none font-mono"
                                  placeholder="~/.ssh/id_rsa"
                                />
                              </div>
                              <div>
                                <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpHostPassphrase">密码短语 (可选)</label>
                                <input
                                  type="password"
                                  id="jumpHostPassphrase"
                                  bind:value={formData.jumpHostPassphrase}
                                  class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 outline-none"
                                />
                                <label class="flex items-center mt-2 cursor-pointer">
                                  <input type="checkbox" bind:checked={formData.jumpHostSavePassphrase} class="rounded border-app-border bg-app-surface text-primary-600 focus:ring-primary-600 ring-offset-app-surface">
                                  <span class="ml-2 text-xs text-app-text-secondary">保存密码短语</span>
                                </label>
                              </div>
                            </div>
                          {:else if formData.jumpHostAuthMethod === 'agent'}
                            <div>
                              <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpHostAgentPath">Agent 路径 (可选)</label>
                              <input
                                type="text"
                                id="jumpHostAgentPath"
                                bind:value={formData.jumpHostAgentPath}
                                class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 outline-none font-mono"
                                placeholder="默认使用系统 SSH_AUTH_SOCK"
                              />
                            </div>
                          {:else if formData.jumpHostAuthMethod === 'certificate'}
                            <div class="space-y-3">
                              <div>
                                <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpHostCertificatePath">跳板证书路径</label>
                                <input
                                  type="text"
                                  id="jumpHostCertificatePath"
                                  bind:value={formData.jumpHostCertificatePath}
                                  class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 outline-none font-mono"
                                  placeholder="~/.ssh/id_rsa-cert.pub"
                                />
                              </div>
                              <div>
                                <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpHostPrivateKeyPath">跳板私钥路径</label>
                                <input
                                  type="text"
                                  id="jumpHostPrivateKeyPath"
                                  bind:value={formData.jumpHostPrivateKeyPath}
                                  class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 outline-none font-mono"
                                  placeholder="~/.ssh/id_rsa"
                                />
                              </div>
                              <div>
                                <label class="block text-xs font-medium text-app-text-secondary mb-1.5" for="jumpHostPassphraseCert">密码短语 (可选)</label>
                                <input
                                  type="password"
                                  id="jumpHostPassphraseCert"
                                  bind:value={formData.jumpHostPassphrase}
                                  class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 outline-none"
                                />
                                <label class="flex items-center mt-2 cursor-pointer">
                                  <input type="checkbox" bind:checked={formData.jumpHostSavePassphrase} class="rounded border-app-border bg-app-surface text-primary-600 focus:ring-primary-600 ring-offset-app-surface">
                                  <span class="ml-2 text-xs text-app-text-secondary">保存密码短语</span>
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
          </div>
        {/if}
      </form>
    </div>

    <!-- Footer -->
    <div class="px-6 py-4 border-t border-app-border bg-app-surface flex items-center justify-end gap-3">
      <button
        type="button"
        class="mr-auto px-4 py-2 text-sm font-medium text-app-text-secondary hover:text-primary-500 bg-app-bg hover:bg-app-bg-hover rounded-lg transition-colors flex items-center gap-2"
        disabled={isTesting}
        on:click={handleTestConnection}
      >
        {#if isTesting}
          <div class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"></div>
          <span>测试中...</span>
        {:else}
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"></path></svg>
          <span>测试连接</span>
        {/if}
      </button>

      <button
        type="button"
        class="px-4 py-2 text-sm font-medium text-app-text-secondary hover:text-app-text bg-app-bg hover:bg-app-bg-hover rounded-lg transition-colors"
        on:click={handleClose}
      >
        取消
      </button>
      <button
        type="submit"
        form="connection-form"
        disabled={isSaving}
        class="px-4 py-2 text-sm font-medium text-white bg-primary-600 hover:bg-primary-500 active:bg-primary-700 rounded-lg transition-colors shadow-lg shadow-primary-900/20 disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
      >
        {#if isSaving}
          <div class="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin"></div>
          <span>处理中...</span>
        {:else}
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"></path></svg>
          <span>{$editingConnection ? '保存' : '保存并连接'}</span>
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
    background: var(--scrollbar-track);
  }
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: var(--scrollbar-thumb);
    border-radius: 3px;
    transition: background-color 0.2s ease;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: var(--scrollbar-thumb-hover);
  }
</style>
