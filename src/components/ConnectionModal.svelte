<script lang="ts">
  import { showConnectionForm, editingConnection, connections, connectionHistory, type Connection, showSuccessMessage } from '../lib/store';
  import { saveConnection, createBackendConfig } from '../lib/connectionService';
  import { connectAndOpen, invokeWithTimeout } from '../lib/terminalService';
  import {
    getHostKeyPromptHint,
    getHostKeyPromptTitle,
    parseHostKeyPrompt,
    saveHostKeyPrompt,
    type HostKeyPrompt,
  } from '../lib/hostKeyPrompt';
  import XIcon from './icons/XIcon.svelte';
  import PlusIcon from './icons/PlusIcon.svelte';
  import ActivityIcon from './icons/ActivityIcon.svelte';
  import ServerIcon from './icons/ServerIcon.svelte';
  import FolderIcon from './icons/FolderIcon.svelte';
  import ClockIcon from './icons/ClockIcon.svelte';
  import { slide, fade } from 'svelte/transition';

  function createDefaultFormData() {
    return {
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
      proxyHasStoredPassword: false,
      proxyPasswordDirty: false,
      clearStoredProxyPassword: false,
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
      autoReconnect: false,
    };
  }

  // Form state
  let formData = createDefaultFormData();
  let previousEditingConnectionId: string | null = null;
  let previousFormVisible = false;

  /*
   * Maintain explicit reset semantics: opening "new connection" from edit mode
   * must not keep any stale fields from the previous edit session.
   */
  function resetFormData() {
    formData = createDefaultFormData();
    activeTab = 'basic';
    lastProtocol = formData.protocol;
    tagInput = '';
    newLocalForward = { local_host: 'localhost', local_port: 0, remote_host: 'localhost', remote_port: 0 };
    newRemoteForward = { remote_host: 'localhost', remote_port: 0, local_host: 'localhost', local_port: 0 };
    hostKeyVerification = null;
  }

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

  function asRecord(value: unknown): Record<string, unknown> | null {
    if (!value || typeof value !== 'object') return null;
    return value as Record<string, unknown>;
  }

  function asString(value: unknown, fallback = ''): string {
    return typeof value === 'string' ? value : fallback;
  }

  function asNumber(value: unknown, fallback: number): number {
    return typeof value === 'number' && Number.isFinite(value) ? value : fallback;
  }

  function hydrateFromConnection(connection: Connection) {
    formData.id = connection.id;
    formData.name = connection.name ?? '';
    formData.protocol = connection.protocol ?? 'Ssh';
    formData.host = connection.host ?? '';
    formData.port = Number(connection.port ?? 22);
    formData.username = connection.username ?? '';
    formData.description = connection.description ?? '';
    formData.tags = normalizeTagsValue(connection.tags).join(',');
    formData.autoReconnect = connection.auto_reconnect ?? false;

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

    const socksPort = connection.socks_proxy_port;
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
    if (authMethod.Password) {
      formData.authMethod = 'password';
      formData.password = authMethod.Password.password ?? '';
      formData.savePassword = Boolean(authMethod.Password.save_password);
    } else if (authMethod.KeyboardInteractive) {
      formData.authMethod = 'keyboardInteractive';
    } else if (authMethod.PrivateKey) {
      formData.authMethod = 'privateKey';
      formData.keyPath = authMethod.PrivateKey.key_path ?? '';
      formData.passphrase = authMethod.PrivateKey.passphrase ?? '';
      formData.savePassphrase = Boolean(authMethod.PrivateKey.save_passphrase);
    } else if (authMethod.Agent) {
      formData.authMethod = 'agent';
      formData.agentPath = authMethod.Agent.agent_path ?? '';
    } else if (authMethod.Certificate) {
      formData.authMethod = 'certificate';
      formData.certificatePath = authMethod.Certificate.certificate_path ?? '';
      formData.privateKeyPath = authMethod.Certificate.private_key_path ?? '';
      formData.passphrase = authMethod.Certificate.passphrase ?? '';
      formData.savePassphrase = Boolean(authMethod.Certificate.save_passphrase);
    } else {
      formData.authMethod = 'password';
    }

    formData.proxyType = 'none';
    formData.proxyHost = '';
    formData.proxyPort = 1080;
    formData.proxyUsername = '';
    formData.proxyPassword = '';
    formData.proxyHasStoredPassword = false;
    formData.proxyPasswordDirty = false;
    formData.clearStoredProxyPassword = false;
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

    const proxyType = connection.proxy_type;
    const proxyRecord = asRecord(proxyType);
    const socksProxy = asRecord(proxyRecord?.Socks5);
    const httpProxy = asRecord(proxyRecord?.Http);
    const jumpHostProxy = asRecord(proxyRecord?.JumpHost);

    if (!proxyType || proxyType === 'None' || !proxyRecord) {
      formData.proxyType = 'none';
    } else if (socksProxy) {
      formData.proxyType = 'socks5';
      formData.proxyHost = asString(socksProxy.host);
      formData.proxyPort = asNumber(socksProxy.port, 1080);
      formData.proxyUsername = asString(socksProxy.username);
      formData.proxyPassword = asString(socksProxy.password);
      formData.proxyHasStoredPassword =
        Boolean(socksProxy.has_password) || formData.proxyPassword.length > 0;
      formData.proxyPasswordDirty = false;
      formData.clearStoredProxyPassword = false;
    } else if (httpProxy) {
      formData.proxyType = 'http';
      formData.proxyHost = asString(httpProxy.host);
      formData.proxyPort = asNumber(httpProxy.port, 8080);
      formData.proxyUsername = asString(httpProxy.username);
      formData.proxyPassword = asString(httpProxy.password);
      formData.proxyHasStoredPassword =
        Boolean(httpProxy.has_password) || formData.proxyPassword.length > 0;
      formData.proxyPasswordDirty = false;
      formData.clearStoredProxyPassword = false;
    } else if (jumpHostProxy) {
      formData.proxyType = 'jumpHost';
      formData.proxyHost = asString(jumpHostProxy.host);
      formData.proxyPort = asNumber(jumpHostProxy.port, 22);
      formData.jumpHostUsername = asString(jumpHostProxy.username);

      const jumpAuth = asRecord(jumpHostProxy.auth_method) ?? {};
      if (jumpAuth.Password) {
        formData.jumpHostAuthMethod = 'password';
        const password = asRecord(jumpAuth.Password);
        formData.jumpHostPassword = asString(password?.password);
        formData.jumpHostSavePassword = Boolean(password?.save_password);
      } else if (jumpAuth.KeyboardInteractive) {
        formData.jumpHostAuthMethod = 'keyboardInteractive';
      } else if (jumpAuth.PrivateKey) {
        formData.jumpHostAuthMethod = 'privateKey';
        const privateKey = asRecord(jumpAuth.PrivateKey);
        formData.jumpHostKeyPath = asString(privateKey?.key_path);
        formData.jumpHostPassphrase = asString(privateKey?.passphrase);
        formData.jumpHostSavePassphrase = Boolean(privateKey?.save_passphrase);
      } else if (jumpAuth.Agent) {
        formData.jumpHostAuthMethod = 'agent';
        const agent = asRecord(jumpAuth.Agent);
        formData.jumpHostAgentPath = asString(agent?.agent_path);
      } else if (jumpAuth.Certificate) {
        formData.jumpHostAuthMethod = 'certificate';
        const certificate = asRecord(jumpAuth.Certificate);
        formData.jumpHostCertificatePath = asString(certificate?.certificate_path);
        formData.jumpHostPrivateKeyPath = asString(certificate?.private_key_path);
        formData.jumpHostPassphrase = asString(certificate?.passphrase);
        formData.jumpHostSavePassphrase = Boolean(certificate?.save_passphrase);
      }
    }
  }

  function handleProxyPasswordInput(value: string) {
    formData.proxyPassword = value;
    formData.proxyPasswordDirty = true;
    if (value) {
      formData.clearStoredProxyPassword = false;
    }
  }

  function clearStoredProxyPassword() {
    formData.proxyPassword = '';
    formData.proxyPasswordDirty = true;
    formData.clearStoredProxyPassword = true;
    formData.proxyHasStoredPassword = false;
  }

  function restoreStoredProxyPassword() {
    formData.proxyPassword = '';
    formData.proxyPasswordDirty = false;
    formData.clearStoredProxyPassword = false;
    formData.proxyHasStoredPassword = true;
  }

  function sanitizeConnectConfig(config: unknown): Connection {
    const out = JSON.parse(JSON.stringify(config ?? {})) as Connection;
    if (out?.auth_method?.Password) out.auth_method.Password.password = '';
    if (out?.auth_method?.PrivateKey) delete out.auth_method.PrivateKey.passphrase;
    if (out?.auth_method?.Certificate) delete out.auth_method.Certificate.passphrase;

    const proxyRecord = asRecord(out?.proxy_type);
    const socksProxy = asRecord(proxyRecord?.Socks5);
    const httpProxy = asRecord(proxyRecord?.Http);
    const jumpHost = asRecord(proxyRecord?.JumpHost);
    const jumpAuth = asRecord(jumpHost?.auth_method);
    const jumpPassword = asRecord(jumpAuth?.Password);
    const jumpPrivateKey = asRecord(jumpAuth?.PrivateKey);
    const jumpCertificate = asRecord(jumpAuth?.Certificate);

    if (socksProxy) {
      socksProxy.password = '';
    }
    if (httpProxy) {
      httpProxy.password = '';
    }

    if (jumpPassword) {
      jumpPassword.password = '';
    }
    if (jumpPrivateKey) {
      delete jumpPrivateKey.passphrase;
    }
    if (jumpCertificate) {
      delete jumpCertificate.passphrase;
    }
    return out;
  }

  // Tabs
  let activeTab: 'basic' | 'advanced' = 'basic';
  let modalView: 'chooser' | 'form' = 'form';
  let chooserSearchTerm = '';
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
  let testConnectionFeedback: { type: 'success' | 'error'; message: string } | null = null;

  let hostKeyVerification: HostKeyPrompt | null = null;

  $: hostKeyTitle =
    hostKeyVerification ? getHostKeyPromptTitle(hostKeyVerification.type) : '未知的主机密钥';

  $: hostKeyHint =
    hostKeyVerification ? getHostKeyPromptHint(hostKeyVerification.type) : '首次连接该服务器，请确认指纹后再继续。';

  $: {
    const formVisible = $showConnectionForm;
    const editingId = $editingConnection?.id ?? null;
    const openedCreate = formVisible && !previousFormVisible && editingId === null;
    const switchedFromEditToCreate =
      formVisible && editingId === null && previousEditingConnectionId !== null;

    if (openedCreate || switchedFromEditToCreate) {
      resetFormData();
      modalView = 'chooser';
    } else if (formVisible && editingId && editingId !== previousEditingConnectionId && $editingConnection) {
      hydrateFromConnection($editingConnection);
      modalView = 'form';
    }

    previousFormVisible = formVisible;
    previousEditingConnectionId = editingId;
  }

  $: savedConnections = [...$connections].sort((a, b) => {
    const aName = (a.name ?? '').toLowerCase();
    const bName = (b.name ?? '').toLowerCase();
    return aName.localeCompare(bName, 'zh-CN');
  });

  $: recentConnectionMap = new Map($connectionHistory.map(item => [item.connection.id, item.lastConnected]));

  $: searchedSavedConnections = savedConnections.filter(connection => {
    const term = chooserSearchTerm.trim().toLowerCase();
    if (!term) return true;
    const tags = Array.isArray(connection.tags) ? connection.tags.join(' ').toLowerCase() : '';
    const address = `${connection.username ?? ''} ${connection.host ?? ''} ${connection.port ?? ''}`.toLowerCase();
    return (
      (connection.name ?? '').toLowerCase().includes(term) ||
      address.includes(term) ||
      tags.includes(term)
    );
  });

  $: chooserStats = {
    total: savedConnections.length,
    recent: savedConnections.filter(connection => recentConnectionMap.has(connection.id)).length,
    grouped: savedConnections.filter(connection => Array.isArray(connection.tags) && connection.tags.length > 0).length,
  };

  type ChooserTagNode = {
    name: string;
    path: string;
    children: Map<string, ChooserTagNode>;
    connections: Connection[];
  };

  type ChooserTreeRow =
    | { kind: 'folder'; id: string; depth: number; name: string; path: string; count: number; hasChildren: boolean }
    | { kind: 'connection'; id: string; depth: number; connection: Connection };

  let chooserExpandedPaths = new Set<string>(['未分组']);

  function splitTagPath(tag: string): string[] {
    return tag
      .split('/')
      .map(part => part.trim())
      .filter(Boolean);
  }

  function getPrimaryGroupTag(connection: Connection): string {
    const tags = normalizeTagsValue(connection.tags);
    return tags[0] ? String(tags[0]).trim() : '未分组';
  }

  function buildChooserTree(items: Connection[]): ChooserTagNode {
    const root: ChooserTagNode = { name: '', path: '', children: new Map(), connections: [] };

    for (const connection of items) {
      const groupTag = getPrimaryGroupTag(connection);
      const parts = groupTag === '未分组' ? ['未分组'] : splitTagPath(groupTag);
      if (parts.length === 0) continue;

      let node = root;
      for (const part of parts) {
        const nextPath = node.path ? `${node.path}/${part}` : part;
        const existing = node.children.get(part);
        if (existing) {
          node = existing;
        } else {
          const created: ChooserTagNode = { name: part, path: nextPath, children: new Map(), connections: [] };
          node.children.set(part, created);
          node = created;
        }
      }
      node.connections.push(connection);
    }

    if (!root.children.has('未分组')) {
      root.children.set('未分组', { name: '未分组', path: '未分组', children: new Map(), connections: [] });
    }

    return root;
  }

  function countChooserConnections(node: ChooserTagNode): number {
    let total = node.connections.length;
    for (const child of node.children.values()) {
      total += countChooserConnections(child);
    }
    return total;
  }

  function flattenChooserTree(node: ChooserTagNode, expanded: Set<string>, depth = 0): ChooserTreeRow[] {
    const rows: ChooserTreeRow[] = [];
    const children = Array.from(node.children.values()).sort((a, b) => a.name.localeCompare(b.name, 'zh-Hans-CN'));

    for (const child of children) {
      const hasChildren = child.children.size > 0;
      rows.push({
        kind: 'folder',
        id: `folder:${child.path}`,
        depth,
        name: child.name,
        path: child.path,
        count: countChooserConnections(child),
        hasChildren,
      });

      if (expanded.has(child.path)) {
        rows.push(...flattenChooserTree(child, expanded, depth + 1));
        const sortedConnections = [...child.connections].sort((a, b) =>
          String(a.name ?? '').localeCompare(String(b.name ?? ''), 'zh-Hans-CN')
        );
        for (const connection of sortedConnections) {
          rows.push({
            kind: 'connection',
            id: `connection:${child.path}:${connection.id}`,
            depth: depth + 1,
            connection,
          });
        }
      }
    }

    return rows;
  }

  function toggleChooserFolder(path: string) {
    const next = new Set(chooserExpandedPaths);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    chooserExpandedPaths = next;
  }

  $: chooserTree = buildChooserTree(searchedSavedConnections);
  $: chooserTreeRows = flattenChooserTree(chooserTree, chooserExpandedPaths);

  function formatLastConnected(timestamp: number | undefined): string {
    if (!timestamp) return '';
    const diff = Date.now() - timestamp;
    const minute = 60 * 1000;
    const hour = 60 * minute;
    const day = 24 * hour;
    if (diff < hour) return `${Math.max(1, Math.floor(diff / minute))} 分钟前`;
    if (diff < day) return `${Math.floor(diff / hour)} 小时前`;
    return `${Math.floor(diff / day)} 天前`;
  }

  async function handleQuickConnect(connection: Connection) {
    await connectAndOpen(connection);
    handleClose();
  }

  function openCreateForm() {
    resetFormData();
    modalView = 'form';
  }

  function goBackToChooser() {
    testConnectionFeedback = null;
    modalView = 'chooser';
  }

  function setTestConnectionFeedback(type: 'success' | 'error', message: string) {
    testConnectionFeedback = { type, message };
  }

  async function acceptHostKey() {
    if (!hostKeyVerification) return;

    try {
      await saveHostKeyPrompt(hostKeyVerification);

      hostKeyVerification = null;
      showSuccessMessage('主机密钥已保存到应用信任库', 3000);

      // Retry connection test
      handleTestConnection();
    } catch (error) {
      console.error('Failed to save host key:', error);
      setTestConnectionFeedback('error', `保存主机密钥失败: ${error}`);
    }
  }

  async function handleTestConnection() {
    commitPendingTag();
    trimHost();
    testConnectionFeedback = null;
    if (!formData.host || !formData.port) {
      setTestConnectionFeedback('error', '请填写主机地址和端口');
      return;
    }

    isTesting = true;
    try {
      const config = await createBackendConfig({
        ...formData,
        local_forwards: formData.localForwards,
        remote_forwards: formData.remoteForwards,
      });

      await invokeWithTimeout('test_connection', { config }, 45000);
      setTestConnectionFeedback('success', '连接测试成功！');
    } catch (error: unknown) {
      const parsedHostKey = parseHostKeyPrompt(error);
      if (parsedHostKey) {
        hostKeyVerification = parsedHostKey;
        return;
      }

      console.error('Connection test failed:', error);
      setTestConnectionFeedback('error', `连接测试失败: ${error}`);
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
        connectAndOpen(safeConnection, result.connectConfig);
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
    testConnectionFeedback = null;
    editingConnection.set(null);
    showConnectionForm.set(false);
  }
</script>

<svelte:window on:keydown={(e) => e.key === 'Escape' && handleClose()} />

<div class="absolute inset-0 z-30 flex flex-col bg-app-bg">
  <div class="relative flex-1 flex flex-col overflow-hidden">
    <!-- Host Key Verification Overlay -->
    {#if hostKeyVerification}
      <div class="absolute inset-0 z-50 bg-app-surface flex flex-col items-center justify-center p-8 text-center" transition:fade={{ duration: 200 }}>
        <div class="w-16 h-16 bg-yellow-100 dark:bg-yellow-900/30 text-yellow-600 dark:text-yellow-400 rounded-full flex items-center justify-center mb-4">
          <svg class="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"></path></svg>
        </div>
        <h3 class="text-xl font-semibold text-app-text mb-2">{hostKeyTitle}</h3>
          <p class="text-sm text-app-text-secondary mb-6 max-w-md">
          服务器 <strong>{hostKeyVerification.payload.host}:{hostKeyVerification.payload.port}</strong> 的身份验证需要确认。
          <br>
          {hostKeyHint}
          <br>
          {#if hostKeyVerification.payload.reason}
            原因: {hostKeyVerification.payload.reason}
            <br>
          {/if}
          指纹: <code class="bg-app-bg px-1 py-0.5 rounded text-xs font-mono select-all mt-2 inline-block">{hostKeyVerification.payload.fingerprint}</code>
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
    <div class="flex-none border-b border-app-border bg-app-bg px-6 py-4">
      <div class="mx-auto w-full max-w-3xl flex items-center justify-between gap-4">
        <div class="flex items-center gap-4">
          <div class="flex items-center gap-3">
            {#if !$editingConnection && modalView === 'form'}
              <button
                type="button"
                class="inline-flex items-center justify-center rounded-md p-1.5 text-app-text-secondary transition-colors hover:bg-app-bg-hover hover:text-app-text"
                on:click={goBackToChooser}
                title="返回新建页"
              >
                <svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
                </svg>
              </button>
            {/if}
            <h2 class="text-lg font-semibold text-app-text">
              {$editingConnection ? '编辑连接' : modalView === 'chooser' ? '新建页' : '新建连接'}
            </h2>
          </div>
          {#if modalView === 'form'}
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
          {/if}
        </div>
        <button
          class="text-app-text-secondary hover:text-app-text transition-colors p-1 rounded-md hover:bg-app-bg-hover"
          on:click={handleClose}
        >
          <XIcon class="w-5 h-5" />
        </button>
      </div>
    </div>

    <!-- Scrollable Content -->
    <div class="flex-1 overflow-y-auto p-6 custom-scrollbar">
      <div class="mx-auto w-full max-w-3xl">
      {#if !$editingConnection && modalView === 'chooser'}
        <div class="space-y-6" in:fade={{ duration: 150 }}>
          <div class="border-b border-app-border pb-5">
            <div class="flex gap-2">
              <button
                type="button"
                class="flex items-center gap-2 rounded-lg bg-primary-600 px-4 py-2 text-sm font-medium text-white shadow-md transition-all hover:bg-primary-500 hover:shadow-primary-900/30 active:scale-95"
                on:click={openCreateForm}
              >
                <PlusIcon class="h-4 w-4" />
                <span>新建连接</span>
              </button>
            </div>
            <div class="mt-3 text-sm text-app-text-secondary">
              已保存 <span class="text-app-text">{chooserStats.total}</span> 个连接，最近使用 <span class="text-app-text">{chooserStats.recent}</span> 个。
            </div>
          </div>

          <div class="space-y-3">
            <div class="px-2 py-1.5 flex justify-between items-center text-xs font-semibold text-app-text-secondary uppercase tracking-wider whitespace-nowrap">
              <span>已保存连接</span>
              <span>{searchedSavedConnections.length} 项</span>
            </div>

            <div class="px-2">
              <div class="relative">
                <input
                  type="text"
                  bind:value={chooserSearchTerm}
                  placeholder="搜索连接..."
                  class="w-full bg-app-surface border border-app-border rounded-lg py-1.5 px-3 pl-9 text-sm text-app-text placeholder-app-text-secondary focus:outline-none focus:border-primary-500/50 focus:ring-1 focus:ring-primary-500/50 transition-all"
                />
                <svg class="absolute left-3 top-2 w-4 h-4 text-app-text-secondary" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"></path>
                </svg>
              </div>
            </div>

            {#if chooserTreeRows.length > 0}
              <div class="space-y-0.5">
                {#each chooserTreeRows as row (row.id)}
                  {#if row.kind === 'folder'}
                    <button
                      type="button"
                      class="w-full text-left flex items-center gap-2 rounded-lg p-2 transition-colors hover:bg-app-surface"
                      style={`padding-left: ${0.5 + row.depth * 0.75}rem;`}
                      on:click={() => toggleChooserFolder(row.path)}
                    >
                      <span class="text-app-text-secondary w-4 inline-flex justify-center">
                        {#if chooserExpandedPaths.has(row.path)}
                          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path>
                          </svg>
                        {:else}
                          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"></path>
                          </svg>
                        {/if}
                      </span>
                      <span class="text-app-text-secondary shrink-0">
                        <FolderIcon class="w-4 h-4" />
                      </span>
                      <span class="flex-1 min-w-0">
                        <span class="font-medium text-app-text truncate">{row.name}</span>
                      </span>
                      <span class="text-[10px] text-app-text-secondary bg-app-surface px-1.5 py-0.5 rounded-full">
                        {row.count}
                      </span>
                    </button>
                  {:else}
                    <button
                      type="button"
                      class="group w-full text-left flex items-center gap-3 rounded-lg p-3 transition-colors hover:bg-app-surface"
                      style={`padding-left: ${0.75 + row.depth * 0.75}rem;`}
                      on:click={() => handleQuickConnect(row.connection)}
                    >
                      <div class="text-app-text-secondary group-hover:text-primary-500 dark:group-hover:text-primary-400 transition-colors shrink-0">
                        <ServerIcon class="w-4 h-4" />
                      </div>
                      <div class="min-w-0 flex-1">
                        <div class="font-medium text-app-text truncate flex items-center gap-2">
                          <span class="truncate">{row.connection.name}</span>
                          <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-app-surface text-app-text-secondary shrink-0">
                            {(row.connection as any).protocol === 'Rdp' ? 'RDP' : 'SSH'}
                          </span>
                          {#if recentConnectionMap.has(row.connection.id)}
                            <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-app-surface text-app-text-secondary shrink-0 inline-flex items-center gap-1">
                              <ClockIcon className="w-3 h-3" />
                              <span>{formatLastConnected(recentConnectionMap.get(row.connection.id))}</span>
                            </span>
                          {/if}
                        </div>
                        <div class="text-xs text-app-text-secondary truncate mt-0.5 font-mono opacity-80">
                          {#if row.connection.username}{row.connection.username}@{/if}{row.connection.host}:{row.connection.port}
                        </div>
                      </div>
                    </button>
                  {/if}
                {/each}
              </div>
            {:else}
              <div class="flex flex-col items-center justify-center py-12 text-app-text-secondary">
                <div class="mb-3 flex h-12 w-12 items-center justify-center rounded-xl bg-app-surface text-app-text-secondary">
                  <ServerIcon class="h-5 w-5" />
                </div>
                <p class="text-sm font-medium text-app-text">{chooserSearchTerm.trim() ? '没有匹配的连接' : '还没有已保存连接'}</p>
                <p class="mt-1 text-sm text-app-text-secondary">{chooserSearchTerm.trim() ? '换个关键词试试，或者直接创建一个新连接。' : '先创建一个连接配置，之后就可以在这里快速进入。'}</p>
                {#if chooserSearchTerm.trim()}
                  <button
                    type="button"
                    class="mt-4 inline-flex items-center gap-2 rounded-lg border border-app-border bg-app-surface px-4 py-2 text-sm text-app-text transition-colors hover:bg-app-bg-hover"
                    on:click={() => (chooserSearchTerm = '')}
                  >
                    清空搜索
                  </button>
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {:else}
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

              <!-- Row 3: Auto Reconnect -->
              <div class="flex items-center">
                <label class="flex items-center gap-2 cursor-pointer group">
                  <div class="relative flex items-center justify-center w-4 h-4">
                    <input 
                      type="checkbox" 
                      bind:checked={formData.autoReconnect} 
                      class="peer appearance-none w-4 h-4 rounded border border-app-border checked:border-primary-500 checked:bg-primary-500 transition-all"
                    />
                    <svg class="absolute w-3 h-3 text-white scale-0 peer-checked:scale-100 transition-transform" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="3" d="M5 13l4 4L19 7" />
                    </svg>
                  </div>
                  <span class="text-xs text-app-text-secondary group-hover:text-app-text transition-colors">自动重连 (意外断开时)</span>
                </label>
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
                          <div class="space-y-2">
                            <input
                              type="password"
                              value={formData.proxyPassword}
                              on:input={(event) => handleProxyPasswordInput((event.currentTarget as HTMLInputElement).value)}
                              class="w-full bg-app-surface border border-app-border rounded-lg px-3 py-2 text-sm text-app-text focus:border-primary-500 focus:ring-1 focus:ring-primary-500 outline-none transition-all"
                              placeholder={
                                formData.proxyHasStoredPassword && !formData.proxyPasswordDirty
                                  ? '已保存密码，留空则保持不变'
                                  : '密码'
                              }
                            />
                            {#if formData.proxyHasStoredPassword && !formData.proxyPasswordDirty}
                              <div class="flex items-center justify-between gap-3 text-xs text-app-text-secondary">
                                <span>已存在保存的代理密码。</span>
                                <button
                                  type="button"
                                  class="text-red-500 hover:text-red-400 transition-colors"
                                  on:click={clearStoredProxyPassword}
                                >
                                  清除已保存密码
                                </button>
                              </div>
                            {:else if formData.clearStoredProxyPassword}
                              <div class="flex items-center justify-between gap-3 text-xs text-app-text-secondary">
                                <span>保存时会删除当前代理密码。</span>
                                <button
                                  type="button"
                                  class="text-primary-600 hover:text-primary-500 transition-colors"
                                  on:click={restoreStoredProxyPassword}
                                >
                                  保留已保存密码
                                </button>
                              </div>
                            {/if}
                          </div>
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
      {/if}
      </div>
    </div>

    <!-- Footer -->
    {#if modalView === 'form' || $editingConnection}
    <div class="flex-none px-6 py-4 border-t border-app-border bg-app-surface">
      <div class="mx-auto w-full max-w-3xl space-y-3">
      {#if testConnectionFeedback}
        <div
          class="flex items-start gap-2 rounded-lg border px-3 py-2 text-sm {testConnectionFeedback.type === 'success'
            ? 'border-green-500/20 bg-green-500/10 text-green-400'
            : 'border-red-500/20 bg-red-500/10 text-red-400'}"
        >
          {#if testConnectionFeedback.type === 'success'}
            <svg class="mt-0.5 w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path></svg>
          {:else}
            <svg class="mt-0.5 w-4 h-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
          {/if}
          <span class="leading-5 break-words">{testConnectionFeedback.message}</span>
        </div>
      {/if}

      <div class="flex items-center justify-end gap-3">
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
    {/if}
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
