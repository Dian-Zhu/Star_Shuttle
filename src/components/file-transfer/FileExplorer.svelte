<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sftpService } from '../../lib/sftpService';
  import { localFsService } from '../../lib/localFsService';
  import { fileClipboard, settings } from '../../lib/store';
  import { isEditableShortcutTarget, matchShortcut } from '../../lib/shortcuts';
  import ContextMenu from '../ui/ContextMenu.svelte';
  import ContextMenuItem from '../ui/ContextMenuItem.svelte';
  import ContextMenuDivider from '../ui/ContextMenuDivider.svelte';
  import FileIcon from './FileIcon.svelte';
  import type { FileEntry } from '../../types';
  
  export let sessionId: string;
  export let initialPath: string = '.'; // Default to current directory

  let currentPath = initialPath;
  let files: FileEntry[] = [];
  let loading = false;
  let error: string | null = null;
  
  // Selection State
  let selectedPaths: Set<string> = new Set();
  let lastSelectedPath: string | null = null;
  
  let contextMenu = { x: 0, y: 0, show: false, file: null as FileEntry | null };
  let fileInput: HTMLInputElement;
  let isDragging = false;

  let editorOpen = false;
  let editorFile: FileEntry | null = null;
  let editorContent = '';
  let editorLoading = false;
  let editorSaving = false;
  let editorError: string | null = null;
  let editorSessionId: string | null = null;
  const FILE_TRANSFER_CHUNK_SIZE = 1024 * 1024; // 1MB
  const MAX_IN_MEMORY_FALLBACK_BYTES = 8 * 1024 * 1024; // 8MB
  const pathWriteLocks = new Map<string, Promise<void>>();

  function joinPath(base: string, name: string): string {
    if (base === '/' || base === '') return `/${name}`;
    return `${base}/${name}`.replace('//', '/');
  }

  function normalizePath(path: string): string {
    const trimmed = path.trim();
    if (!trimmed || trimmed === '.') return '.';
    if (trimmed === '/') return '/';
    if (trimmed.startsWith('~')) return trimmed;

    const isAbsolute = trimmed.startsWith('/');
    const parts = trimmed.split('/').filter(Boolean);
    const stack: string[] = [];

    for (const part of parts) {
      if (part === '.') continue;
      if (part === '..') {
        if (stack.length > 0 && stack[stack.length - 1] !== '..') {
          stack.pop();
        } else if (!isAbsolute) {
          stack.push('..');
        }
        continue;
      }
      stack.push(part);
    }

    if (isAbsolute) {
      return stack.length > 0 ? `/${stack.join('/')}` : '/';
    }

    return stack.length > 0 ? stack.join('/') : '.';
  }

  async function withPathWriteLock<T>(
    scope: 'remote' | 'local',
    rawPath: string,
    task: () => Promise<T>
  ): Promise<T> {
    const normalizedPath = scope === 'remote' ? normalizePath(rawPath) : rawPath.replace(/\\/g, '/').trim();
    const key = `${scope}:${normalizedPath}`;

    // Serialize writes for the same target path to prevent interleaved chunks.
    for (;;) {
      const inFlight = pathWriteLocks.get(key);
      if (!inFlight) break;
      await inFlight;
    }

    let releaseLock: () => void = () => {};
    const lock = new Promise<void>((resolve) => {
      releaseLock = resolve;
    });
    pathWriteLocks.set(key, lock);

    try {
      return await task();
    } finally {
      if (pathWriteLocks.get(key) === lock) {
        pathWriteLocks.delete(key);
      }
      releaseLock();
    }
  }

  function randomId(): string {
    // Prefer stable unique IDs when available (Tauri webview supports crypto in modern runtimes).
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
      return crypto.randomUUID();
    }
    return `${Date.now()}-${Math.random().toString(16).slice(2)}`;
  }

  async function atomicReplaceRemoteFile(
    targetSessionId: string,
    finalPath: string,
    writeToTemp: (tempPath: string) => Promise<void>
  ): Promise<void> {
    const id = randomId();
    const tempPath = `${finalPath}.starshuttle-tmp-${id}`;
    const backupPath = `${finalPath}.starshuttle-backup-${id}`;
    let backedUp = false;

    // Ensure temp doesn't exist (best-effort).
    try {
      await sftpService.removeFile(targetSessionId, tempPath);
    } catch {
      // Ignore missing temp files from previous interrupted attempts.
    }

    try {
      await writeToTemp(tempPath);
    } catch (e) {
      // Don't leave partial temp files around.
      try {
        await sftpService.removeFile(targetSessionId, tempPath);
      } catch {
        // Ignore temp cleanup failures and preserve the original write error.
      }
      throw e;
    }

    // Best-effort: move the existing target aside so we can roll back if rename fails.
    try {
      await sftpService.rename(targetSessionId, finalPath, backupPath);
      backedUp = true;
    } catch {
      backedUp = false;
    }

    try {
      await sftpService.rename(targetSessionId, tempPath, finalPath);
    } catch (e) {
      // Roll back if we successfully backed up the original.
      if (backedUp) {
        try {
          await sftpService.rename(targetSessionId, backupPath, finalPath);
        } catch {
          // Ignore rollback failures; the original rename error is more important.
        }
      }
      // Also try to clean up temp if it's still there.
      try {
        await sftpService.removeFile(targetSessionId, tempPath);
      } catch {
        // Ignore temp cleanup failures and preserve the original rename error.
      }
      throw e;
    }

    // Success: try to delete the backup; if it fails, prefer leaving data around over data loss.
    if (backedUp) {
      try {
        await sftpService.removeFile(targetSessionId, backupPath);
      } catch {
        // Leaving a backup file behind is safer than risking data loss.
      }
    }
  }

  function parentPath(path: string): string {
    const normalized = normalizePath(path);
    if (normalized === '/' || normalized === '.') return normalized;
    if (normalized === '~') return '~';
    // Treat ~/... as home-relative; allow navigating upwards within that namespace.
    if (normalized.startsWith('~/')) {
      const rest = normalized.slice(2);
      const segments = rest.split('/').filter(Boolean);
      segments.pop();
      return segments.length > 0 ? `~/${segments.join('/')}` : '~';
    }
    // Preserve other ~-prefixed forms (e.g. ~user) as-is.
    if (normalized.startsWith('~')) return normalized;

    if (normalized.startsWith('/')) {
      const segments = normalized.split('/').filter(Boolean);
      segments.pop();
      return segments.length > 0 ? `/${segments.join('/')}` : '/';
    }

    const segments = normalized.split('/').filter(Boolean);
    segments.pop();
    return segments.length > 0 ? segments.join('/') : '.';
  }

  function resolveTargetPath(inputPath: string): string {
    const trimmedInput = inputPath.trim();
    if (!trimmedInput || trimmedInput === '.') {
      return normalizePath(currentPath);
    }
    if (trimmedInput === '..') {
      return parentPath(currentPath);
    }
    if (trimmedInput.startsWith('/')) {
      return normalizePath(trimmedInput);
    }
    if (trimmedInput.startsWith('~')) {
      return trimmedInput;
    }

    const normalizedCurrent = normalizePath(currentPath);
    if (normalizedCurrent === '/' || normalizedCurrent.startsWith('~')) {
      return normalizePath(joinPath(normalizedCurrent, trimmedInput));
    }
    if (normalizedCurrent === '.') {
      return normalizePath(trimmedInput);
    }
    return normalizePath(`${normalizedCurrent}/${trimmedInput}`);
  }

  // 根据文件扩展名获取图标
  function getFileIcon(fileName: string, isDirectory: boolean): string {
    if (isDirectory) return 'folder';

    const ext = fileName.split('.').pop()?.toLowerCase() || '';

    // 代码文件
    const codeFiles = ['js', 'ts', 'jsx', 'tsx', 'vue', 'svelte', 'py', 'java', 'c', 'cpp', 'h', 'hpp', 'cs', 'go', 'rs', 'swift', 'kt', 'php', 'rb', 'scala'];
    if (codeFiles.includes(ext)) return 'code';

    // 配置文件
    const configFiles = ['json', 'yaml', 'yml', 'toml', 'ini', 'conf', 'cfg', 'env', 'xml', 'properties', 'lock'];
    if (configFiles.includes(ext)) return 'settings';

    // 样式文件
    const styleFiles = ['css', 'scss', 'sass', 'less', 'styl'];
    if (styleFiles.includes(ext)) return 'style';

    // 图片文件
    const imageFiles = ['jpg', 'jpeg', 'png', 'gif', 'bmp', 'svg', 'webp', 'ico', 'tiff', 'heic'];
    if (imageFiles.includes(ext)) return 'image';

    // 视频文件
    const videoFiles = ['mp4', 'avi', 'mkv', 'mov', 'wmv', 'flv', 'webm', 'm4v', '3gp'];
    if (videoFiles.includes(ext)) return 'video';

    // 音频文件
    const audioFiles = ['mp3', 'wav', 'ogg', 'flac', 'aac', 'm4a', 'wma'];
    if (audioFiles.includes(ext)) return 'audio';

    // 压缩文件
    const archiveFiles = ['zip', 'rar', '7z', 'tar', 'gz', 'bz2', 'xz', 'tar.gz', 'tgz'];
    if (archiveFiles.includes(ext)) return 'archive';

    // 文档文件
    const docFiles = ['pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'odt', 'ods', 'odp'];
    if (docFiles.includes(ext)) return 'document';

    // Markdown 文件
    if (ext === 'md' || ext === 'markdown') return 'markdown';

    // 文本文件
    const textFiles = ['txt', 'log', 'csv', 'tsv'];
    if (textFiles.includes(ext)) return 'text';

    // 数据库文件
    const dbFiles = ['sql', 'db', 'sqlite', 'sqlite3', 'mdb'];
    if (dbFiles.includes(ext)) return 'database';

    // 证书/密钥文件
    const certFiles = ['pem', 'crt', 'cer', 'key', 'p12', 'pfx', 'csr'];
    if (certFiles.includes(ext)) return 'certificate';

    // 可执行文件
    const exeFiles = ['exe', 'msi', 'app', 'dmg', 'sh', 'bat', 'cmd', 'ps1'];
    if (exeFiles.includes(ext)) return 'executable';

    // 默认文件图标
    return 'file';
  }

  function getMenuTargetDirectory() {
    if (contextMenu.file?.isDirectory) return contextMenu.file.path;
    return currentPath;
  }

  type DirectoryCacheEntry = {
    ts: number;
    files: FileEntry[];
  };

  const directoryCache = new Map<string, DirectoryCacheEntry>();
  const CACHE_TTL_MS = 30000;
  const CACHE_MAX_ENTRIES = 50;

  let loadSequence = 0;
  let lastRequestedPath: string | null = null;
  let lastSessionId = sessionId;
  let activeLoadAbortController: AbortController | null = null;

  $: if (sessionId !== lastSessionId) {
    lastSessionId = sessionId;
    directoryCache.clear();
    loadSequence += 1;
    lastRequestedPath = null;
    activeLoadAbortController?.abort();
    activeLoadAbortController = null;
    if (editorOpen) {
      editorOpen = false;
      editorFile = null;
      editorContent = '';
      editorLoading = false;
      editorSaving = false;
      editorError = null;
      editorSessionId = null;
    }
  }

  function ensureSessionUnchanged(expectedSessionId: string) {
    if (expectedSessionId !== sessionId) {
      throw new Error('会话已切换，操作已取消，请重试');
    }
  }

  async function copyRemoteFileInChunks(
    sourceSessionId: string,
    sourcePath: string,
    targetSessionId: string,
    targetPath: string
  ): Promise<void> {
    const normalizedSource = normalizePath(sourcePath);
    const normalizedTarget = normalizePath(targetPath);
    if (sourceSessionId === targetSessionId && normalizedSource === normalizedTarget) {
      throw new Error('不能粘贴到同一路径：源文件与目标文件相同');
    }

    await withPathWriteLock('remote', targetPath, async () => {
      await atomicReplaceRemoteFile(targetSessionId, targetPath, async (tempPath) => {
        let offset = 0;
        let append = false;
        for (;;) {
          ensureSessionUnchanged(targetSessionId);
          const chunk = await sftpService.readChunk(
            sourceSessionId,
            sourcePath,
            offset,
            FILE_TRANSFER_CHUNK_SIZE
          );
          if (chunk.length === 0) {
            if (!append) {
              // Source is empty; create empty file.
              await sftpService.writeFile(targetSessionId, tempPath, new Uint8Array(0), false);
            }
            break;
          }
          await sftpService.writeFile(targetSessionId, tempPath, chunk, append);
          append = true;
          offset += chunk.length;
        }
      });
    });
  }

  async function downloadRemoteFileInChunks(
    sourceSessionId: string,
    sourcePath: string,
    localPath: string,
    accessToken: string
  ): Promise<void> {
    await withPathWriteLock('local', localPath, async () => {
      let handle: any = null;
      let offset = 0;
      try {
        // Best-effort safety: read the first chunk before truncating/creating the destination.
        // For small files, prefer buffering the entire content to avoid leaving a partial file.
        const first = await sftpService.readChunk(
          sourceSessionId,
          sourcePath,
          0,
          FILE_TRANSFER_CHUNK_SIZE
        );

        handle = await localFsService.openWriteFile(localPath, true, accessToken);
        if (first.length > 0) {
          await localFsService.writeChunk(handle, first);
          offset = first.length;
        }

        for (;;) {
          ensureSessionUnchanged(sourceSessionId);
          const chunk = await sftpService.readChunk(
            sourceSessionId,
            sourcePath,
            offset,
            FILE_TRANSFER_CHUNK_SIZE
          );
          if (chunk.length === 0) break;
          await localFsService.writeChunk(handle, chunk);
          offset += chunk.length;
        }
      } finally {
        if (handle) {
          await localFsService.closeFile(handle);
        }
      }
    });
  }

  async function downloadRemoteFileToMemoryThenWrite(
    sourceSessionId: string,
    sourcePath: string,
    localPath: string,
    accessToken: string,
    maxBytes: number
  ): Promise<void> {
    await withPathWriteLock('local', localPath, async () => {
      // Read first; only truncate/create the destination after we have the full content.
      const content = await readRemoteFileToMemoryInChunks(sourceSessionId, sourcePath, maxBytes);
      let handle: any = null;
      try {
        handle = await localFsService.openWriteFile(localPath, true, accessToken);
        await localFsService.writeChunk(handle, content);
      } finally {
        if (handle) {
          await localFsService.closeFile(handle);
        }
      }
    });
  }

  async function readRemoteFileToMemoryInChunks(
    sourceSessionId: string,
    sourcePath: string,
    maxBytes: number
  ): Promise<Uint8Array> {
    const chunks: Uint8Array[] = [];
    let offset = 0;
    let total = 0;
    for (;;) {
      ensureSessionUnchanged(sourceSessionId);
      const chunk = await sftpService.readChunk(sourceSessionId, sourcePath, offset, FILE_TRANSFER_CHUNK_SIZE);
      if (chunk.length === 0) break;
      total += chunk.length;
      if (total > maxBytes) {
        throw new Error(`文件过大，暂不支持直接编辑（> ${Math.floor(maxBytes / (1024 * 1024))} MB）`);
      }
      chunks.push(chunk);
      offset += chunk.length;
    }

    const content = new Uint8Array(total);
    let cursor = 0;
    for (const chunk of chunks) {
      content.set(chunk, cursor);
      cursor += chunk.length;
    }
    return content;
  }

  function sortFiles(list: FileEntry[]) {
    list.sort((a, b) => {
      if (a.isDirectory === b.isDirectory) return a.name.localeCompare(b.name);
      return a.isDirectory ? -1 : 1;
    });
  }

  function getCacheKey(path: string) {
    return `${sessionId}::${path}`;
  }

  function setCache(path: string, list: FileEntry[]) {
    const key = getCacheKey(path);
    directoryCache.set(key, { ts: Date.now(), files: list });
    while (directoryCache.size > CACHE_MAX_ENTRIES) {
      const firstKey = directoryCache.keys().next().value as string | undefined;
      if (!firstKey) break;
      directoryCache.delete(firstKey);
    }
  }

  function invalidateCache(path: string) {
    directoryCache.delete(getCacheKey(path));
  }

  async function loadFiles(path: string, options?: { force?: boolean }) {
    const targetPath = resolveTargetPath(path);
    if (!options?.force && loading && lastRequestedPath === targetPath) return;

    lastRequestedPath = targetPath;
    const requestId = (loadSequence += 1);
    let controller: AbortController | null = null;

    loading = true;
    error = null;
    selectedPaths.clear();
    lastSelectedPath = null;
    contextMenu.show = false;

    activeLoadAbortController?.abort();
    activeLoadAbortController = null;

    if (!options?.force) {
      const cached = directoryCache.get(getCacheKey(targetPath));
      if (cached && Date.now() - cached.ts <= CACHE_TTL_MS) {
        files = cached.files;
        currentPath = targetPath;
        loading = false;
        return;
      }
    }

    try {
      controller = new AbortController();
      const localController = controller;
      activeLoadAbortController = localController;
      const abortPromise = new Promise<never>((_, reject) => {
        if (localController.signal.aborted) {
          reject(new DOMException('Aborted', 'AbortError'));
          return;
        }
        localController.signal.addEventListener('abort', () => reject(new DOMException('Aborted', 'AbortError')), { once: true });
      });

      const nextFiles = await Promise.race([sftpService.listDirectory(sessionId, targetPath), abortPromise]);
      if (requestId !== loadSequence) return;
      sortFiles(nextFiles);
      files = nextFiles;
      currentPath = targetPath;
      setCache(targetPath, nextFiles);
    } catch (e: any) {
      if (requestId !== loadSequence) return;
      if (e?.name === 'AbortError') return;
      error = e.toString();
    } finally {
      if (controller && activeLoadAbortController === controller) {
        activeLoadAbortController = null;
      }
      const shouldStopLoading = requestId === loadSequence;
      if (shouldStopLoading) loading = false;
    }
  }

  function handleNavigate(path: string) {
    loadFiles(path);
  }

  function handleSelect(e: MouseEvent, file: FileEntry) {
    if (e.ctrlKey || e.metaKey) {
      if (selectedPaths.has(file.path)) {
        selectedPaths.delete(file.path);
      } else {
        selectedPaths.add(file.path);
        lastSelectedPath = file.path;
      }
      selectedPaths = selectedPaths; // trigger reactivity
    } else if (e.shiftKey && lastSelectedPath) {
      // Range selection
      const lastIdx = files.findIndex(f => f.path === lastSelectedPath);
      const currIdx = files.indexOf(file);
      if (lastIdx !== -1 && currIdx !== -1) {
        const start = Math.min(lastIdx, currIdx);
        const end = Math.max(lastIdx, currIdx);
        selectedPaths.clear();
        for (let i = start; i <= end; i++) {
          selectedPaths.add(files[i].path);
        }
        selectedPaths = selectedPaths;
      } else {
         // Fallback if lastSelectedPath not found in current files (e.g. after reload)
         selectedPaths.clear();
         selectedPaths.add(file.path);
         lastSelectedPath = file.path;
         selectedPaths = selectedPaths;
      }
    } else {
      // Single select
      selectedPaths.clear();
      selectedPaths.add(file.path);
      lastSelectedPath = file.path;
      selectedPaths = selectedPaths;
    }
  }

  function handleContextMenu(e: MouseEvent, file: FileEntry | null) {
    e.preventDefault();
    
    // If clicking on a file
    if (file) {
      // If the file is NOT in the current selection, select it (exclusive)
      if (!selectedPaths.has(file.path)) {
        selectedPaths.clear();
        selectedPaths.add(file.path);
        lastSelectedPath = file.path;
        selectedPaths = selectedPaths;
      }
      // If it IS in the selection, do nothing to selection (preserve multi-select for right click)
    } else {
      // Clicked on empty space
      selectedPaths.clear();
      lastSelectedPath = null;
      selectedPaths = selectedPaths;
    }

    contextMenu = {
      x: e.clientX,
      y: e.clientY,
      show: true,
      file
    };
  }

  function closeContextMenu() {
    contextMenu.show = false;
  }

  async function handleCreateFolder() {
    closeContextMenu();
    const name = prompt('请输入文件夹名称:');
    if (!name) return;

    const targetSessionId = sessionId;
    const targetPath = currentPath;
    const path = joinPath(getMenuTargetDirectory(), name);
    try {
      ensureSessionUnchanged(targetSessionId);
      await sftpService.createDirectory(targetSessionId, path);
      if (targetSessionId === sessionId) {
        invalidateCache(targetPath);
        void loadFiles(targetPath, { force: true });
      }
    } catch (e: any) {
      error = e.toString();
    }
  }

  async function handleCreateFile() {
    closeContextMenu();
    const name = prompt('请输入文件名:');
    if (!name) return;

    const targetSessionId = sessionId;
    const targetPath = currentPath;
    const path = joinPath(getMenuTargetDirectory(), name);
    try {
      ensureSessionUnchanged(targetSessionId);
      await withPathWriteLock('remote', path, async () => {
        try {
          await sftpService.writeFile(targetSessionId, path, new Uint8Array(0), false);
        } catch (e) {
          await sftpService.scpUpload(targetSessionId, path, new Uint8Array(0));
        }
      });
      if (targetSessionId === sessionId) {
        invalidateCache(targetPath);
        void loadFiles(targetPath, { force: true });
      }
    } catch (e: any) {
      error = e.toString();
    }
  }

  async function handleDelete() {
    closeContextMenu();
    if (selectedPaths.size === 0) return;
    
    // Get file names for confirmation
    const selectedFiles = files.filter(f => selectedPaths.has(f.path));
    if (selectedFiles.length === 0) return;

    const confirmMsg = selectedFiles.length === 1 
      ? `确定要删除 ${selectedFiles[0].name} 吗？` 
      : `确定要删除选中的 ${selectedFiles.length} 个项目吗？`;

    if (!confirm(confirmMsg)) return;

    const targetSessionId = sessionId;
    const targetPath = currentPath;
    try {
      for (const file of selectedFiles) {
        ensureSessionUnchanged(targetSessionId);
        if (file.isDirectory) {
          await sftpService.removeDirectory(targetSessionId, file.path);
        } else {
          await sftpService.removeFile(targetSessionId, file.path);
        }
      }
      if (targetSessionId === sessionId) {
        invalidateCache(targetPath);
        void loadFiles(targetPath, { force: true });
      }
    } catch (e: any) {
      error = e.toString();
    }
  }

  async function handleRename() {
    closeContextMenu();
    // Rename only supports single file
    if (selectedPaths.size !== 1) return;
    
    const path = Array.from(selectedPaths)[0];
    const file = files.find(f => f.path === path);
    if (!file) return;

    const newName = prompt('请输入新名称:', file.name);
    if (!newName || newName === file.name) return;

    // Construct new path.
    const parts = file.path.split('/');
    parts.pop(); // Remove old filename
    const parentPath = parts.join('/');
    const newPath = parentPath === '' ? `/${newName}` : `${parentPath}/${newName}`;

    const targetSessionId = sessionId;
    const targetPath = currentPath;
    try {
      ensureSessionUnchanged(targetSessionId);
      await sftpService.rename(targetSessionId, file.path, newPath);
      if (targetSessionId === sessionId) {
        invalidateCache(targetPath);
        void loadFiles(targetPath, { force: true });
      }
    } catch (e: any) {
      error = e.toString();
    }
  }

  function handleDragOver(e: DragEvent) {
    isDragging = true;
    if (e.dataTransfer) {
      e.dataTransfer.dropEffect = 'copy';
    }
  }

  function handleDragLeave() {
    isDragging = false;
  }

  async function handleDrop(e: DragEvent) {
    isDragging = false;
    const targetSessionId = sessionId;
    const targetPath = currentPath;

    const items = e.dataTransfer?.files;
    if (!items || items.length === 0) return;

    await uploadFiles(Array.from(items), targetSessionId, targetPath);
  }

  async function handleFileUpload(e: Event) {
    const input = e.target as HTMLInputElement;
    if (!input.files || input.files.length === 0) return;

    await uploadFiles(Array.from(input.files), sessionId, currentPath);
    input.value = ''; // Reset input
  }

  async function uploadFiles(filesToUpload: File[], targetSessionId: string = sessionId, targetPath: string = currentPath) {
    loading = true;
    try {
      for (const file of filesToUpload) {
        await uploadSingleFile(file, targetSessionId, targetPath);
      }
      if (targetSessionId === sessionId) {
        invalidateCache(targetPath);
        await loadFiles(targetPath, { force: true });
      }
    } catch (e: any) {
      error = e.toString();
    } finally {
      loading = false;
    }
  }

  async function uploadSingleFile(file: File, targetSessionId: string, targetPath: string): Promise<void> {
    const path = targetPath === '/' ? `/${file.name}` : `${targetPath}/${file.name}`.replace('//', '/');

    await withPathWriteLock('remote', path, async () => {
      await atomicReplaceRemoteFile(targetSessionId, path, async (tempPath) => {
        const totalChunks = Math.ceil(file.size / FILE_TRANSFER_CHUNK_SIZE);

        if (file.size === 0) {
          try {
            ensureSessionUnchanged(targetSessionId);
            await sftpService.writeFile(targetSessionId, tempPath, new Uint8Array(0), false);
          } catch (e) {
            await sftpService.scpUpload(targetSessionId, tempPath, new Uint8Array(0));
          }
          return;
        }

        try {
          for (let i = 0; i < totalChunks; i++) {
            const start = i * FILE_TRANSFER_CHUNK_SIZE;
            const end = Math.min(start + FILE_TRANSFER_CHUNK_SIZE, file.size);
            const chunk = file.slice(start, end);
            const content = new Uint8Array(await chunk.arrayBuffer());
            ensureSessionUnchanged(targetSessionId);
            await sftpService.writeFile(targetSessionId, tempPath, content, i > 0);
          }
        } catch (e) {
          if (file.size > MAX_IN_MEMORY_FALLBACK_BYTES) {
            throw new Error(
              `上传失败：为避免内存占用，已禁用超大文件整块回退（${formatSize(file.size)}）`
            );
          }
          const full = new Uint8Array(await file.arrayBuffer());
          ensureSessionUnchanged(targetSessionId);
          await sftpService.scpUpload(targetSessionId, tempPath, full);
        }
      });
    });
  }

  async function handleDownload() {
    closeContextMenu();
    // Only support single file download for now
    if (selectedPaths.size !== 1) return;
    
    const path = Array.from(selectedPaths)[0];
    const file = files.find(f => f.path === path);
    if (!file || file.isDirectory) return;

    loading = true;
    try {
      const targetSessionId = sessionId;
      const extension = file.name.includes('.') ? file.name.split('.').pop()! : '';
      const filters = extension ? [{ name: extension, extensions: [extension] }] : [];
      const grant = await localFsService.pickFileForWrite(file.name, filters);
      if (!grant) {
        return; // 用户取消了保存
      }

      if (file.size > 0 && file.size <= MAX_IN_MEMORY_FALLBACK_BYTES) {
        await downloadRemoteFileToMemoryThenWrite(
          targetSessionId,
          file.path,
          grant.path,
          grant.accessToken,
          MAX_IN_MEMORY_FALLBACK_BYTES
        );
      } else {
        await downloadRemoteFileInChunks(targetSessionId, file.path, grant.path, grant.accessToken);
      }

    } catch (e: any) {
      error = e.toString();
    } finally {
      loading = false;
    }
  }

  function formatSize(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  }

  function formatDate(date: Date): string {
    return date.toLocaleString();
  }

  function handleCopy() {
    closeContextMenu();
    if (selectedPaths.size === 0) return;

    const entries = files
      .filter(f => selectedPaths.has(f.path))
      .map(f => ({
        path: f.path,
        name: f.name,
        isDirectory: f.isDirectory
      }));

    fileClipboard.set({
      source: 'remote',
      sessionId,
      entries,
      operation: 'copy',
    });
  }

  async function handlePaste() {
    closeContextMenu();
    const item = get(fileClipboard);
    if (!item || !item.entries || item.entries.length === 0) return;

    // Determine destination
    let destDir = currentPath;
    if (selectedPaths.size === 1) {
      const path = Array.from(selectedPaths)[0];
      const file = files.find(f => f.path === path);
      if (file && file.isDirectory) {
        destDir = file.path;
      }
    }

    loading = true;
    error = null;
    const targetSessionId = sessionId;
    const targetPath = currentPath;
    try {
      for (const entry of item.entries) {
        if (entry.isDirectory) continue; // Skip directories for now
        
        const destPath = joinPath(destDir, entry.name);

        if (!item.sessionId) continue;
        // Prevent a destructive no-op: copying a file onto itself corrupts it when chunked.
        if (item.sessionId === targetSessionId && normalizePath(entry.path) === normalizePath(destPath)) {
          error = `已跳过：不能粘贴到同一路径（${entry.name}）`;
          continue;
        }
        await copyRemoteFileInChunks(item.sessionId, entry.path, targetSessionId, destPath);
      }

      if (targetSessionId === sessionId) {
        invalidateCache(targetPath);
        await loadFiles(targetPath, { force: true });
      }
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (editorOpen) return;
    if (isEditableShortcutTarget(e.target)) {
      return;
    }

    const copyShortcut = $settings.shortcuts.copy;
    const pasteShortcut = $settings.shortcuts.paste;
    const renameShortcut = $settings.shortcuts.fileBrowserRename;
    const deleteShortcut = $settings.shortcuts.fileBrowserDelete;
    const openShortcut = $settings.shortcuts.fileBrowserOpen;
    const backShortcut = $settings.shortcuts.fileBrowserBack;
    const selectAllShortcut = $settings.shortcuts.fileBrowserSelectAll;

    if (matchShortcut(e, selectAllShortcut)) {
      e.preventDefault();
      selectedPaths.clear();
      files.forEach(f => selectedPaths.add(f.path));
      selectedPaths = selectedPaths;
      return;
    }

    if (matchShortcut(e, copyShortcut)) {
      e.preventDefault();
      handleCopy();
      return;
    }

    if (matchShortcut(e, pasteShortcut)) {
      e.preventDefault();
      void handlePaste();
      return;
    }

    const refreshShortcut = $settings.shortcuts.fileBrowserRefresh;
    const newFolderShortcut = $settings.shortcuts.fileBrowserNewFolder;
    const newFileShortcut = $settings.shortcuts.fileBrowserNewFile;

    if (matchShortcut(e, refreshShortcut)) {
      e.preventDefault();
      loadFiles(currentPath, { force: true });
      return;
    }

    if (matchShortcut(e, newFolderShortcut)) {
      e.preventDefault();
      void handleCreateFolder();
      return;
    }

    if (matchShortcut(e, newFileShortcut)) {
      e.preventDefault();
      void handleCreateFile();
      return;
    }

    if (matchShortcut(e, renameShortcut)) {
      e.preventDefault();
      void handleRename();
      return;
    }

    if (matchShortcut(e, deleteShortcut)) {
      e.preventDefault();
      void handleDelete();
      return;
    }

    if (matchShortcut(e, openShortcut)) {
      e.preventDefault();
      if (selectedPaths.size === 1) {
        const path = Array.from(selectedPaths)[0];
        const file = files.find(f => f.path === path);
        if (file) {
          void openEditor(file);
        }
      }
      return;
    }

    if (matchShortcut(e, backShortcut)) {
      e.preventDefault();
      if (currentPath !== '/' && currentPath !== '') {
        loadFiles(parentPath(currentPath));
      }
      return;
    }

    // Keyboard Navigation
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (files.length === 0) return;
      
      let nextIndex = 0;
      // Use lastSelectedPath to find current position
      if (lastSelectedPath) {
        const idx = files.findIndex(f => f.path === lastSelectedPath);
        if (idx !== -1 && idx < files.length - 1) {
          nextIndex = idx + 1;
        } else if (idx !== -1) {
          nextIndex = idx; // stay at bottom
        }
      }
      
      const file = files[nextIndex];
      // Update selection based on modifier keys?
      // For simplicity, arrow keys = single select new item (like standard explorer)
      // unless Shift is held (not implementing shift-arrow for now to keep it simple, just single move)
      selectedPaths.clear();
      selectedPaths.add(file.path);
      lastSelectedPath = file.path;
      selectedPaths = selectedPaths;
      
      scrollToJsonFile(nextIndex);
      return;
    }

    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (files.length === 0) return;
      
      let prevIndex = files.length - 1;
      if (lastSelectedPath) {
        const idx = files.findIndex(f => f.path === lastSelectedPath);
        if (idx > 0) {
          prevIndex = idx - 1;
        } else if (idx !== -1) {
          prevIndex = 0;
        }
      }

      const file = files[prevIndex];
      selectedPaths.clear();
      selectedPaths.add(file.path);
      lastSelectedPath = file.path;
      selectedPaths = selectedPaths;

      scrollToJsonFile(prevIndex);
      return;
    }
  }

  function scrollToJsonFile(index: number) {
    const el = document.getElementById('file-row-' + index);
    if (el) {
      el.scrollIntoView({ block: 'nearest' });
    }
  }

  onMount(() => {
    loadFiles(currentPath);
    // window.addEventListener('click', closeContextMenu);
    window.addEventListener('keydown', handleKeydown);
    return () => {
      // window.removeEventListener('click', closeContextMenu);
      window.removeEventListener('keydown', handleKeydown);
    };
  });


  function handleDragStart(e: DragEvent, file: FileEntry) {
    if (file.isDirectory) return;
    e.dataTransfer?.setData(
      'application/x-starshuttle-file',
      JSON.stringify({
        source: 'remote',
        sessionId,
        path: file.path,
        name: file.name,
        size: file.size,
      })
    );
    e.dataTransfer?.setData('text/plain', file.name);
    e.dataTransfer?.setDragImage?.(document.createElement('div'), 0, 0);
  }

  async function openEditor(file: FileEntry) {
    if (file.isDirectory) {
      handleNavigate(file.path);
      return;
    }
    editorOpen = true;
    editorFile = file;
    editorSessionId = sessionId;
    editorContent = '';
    editorError = null;
    editorLoading = true;
    const targetSessionId = sessionId;
    try {
      const maxEditorBytes = 2 * 1024 * 1024;
      if (file.size > maxEditorBytes) {
        throw new Error('文件过大，暂不支持直接编辑（> 2MB）');
      }
      const content = await readRemoteFileToMemoryInChunks(targetSessionId, file.path, maxEditorBytes);
      ensureSessionUnchanged(targetSessionId);
      if (content.includes(0)) {
        throw new Error('疑似二进制文件，暂不支持直接编辑');
      }
      editorContent = new TextDecoder('utf-8').decode(content);
    } catch (e: any) {
      editorError = e?.message ?? String(e);
    } finally {
      editorLoading = false;
    }
  }

  function closeEditor() {
    if (editorSaving) return;
    editorOpen = false;
    editorFile = null;
    editorSessionId = null;
    editorContent = '';
    editorError = null;
    editorLoading = false;
  }

  async function saveEditor() {
    if (!editorFile || editorSaving) return;
    editorSaving = true;
    editorError = null;
    const targetSessionId = editorSessionId ?? sessionId;
    const targetPath = currentPath;
    const editorPath = editorFile.path;
    try {
      ensureSessionUnchanged(targetSessionId);
      const content = new TextEncoder().encode(editorContent);
      await withPathWriteLock('remote', editorPath, async () => {
        try {
          await sftpService.writeFile(targetSessionId, editorPath, content, false);
        } catch (e) {
          await sftpService.scpUpload(targetSessionId, editorPath, content);
        }
      });
      if (targetSessionId === sessionId) {
        invalidateCache(targetPath);
        await loadFiles(targetPath, { force: true });
      }
      closeEditor();
    } catch (e: any) {
      editorError = e?.message ?? String(e);
    } finally {
      editorSaving = false;
    }
  }
</script>

<div 
  class="flex flex-col h-full bg-app-bg text-app-text relative {isDragging ? 'border-2 border-primary-500 bg-app-surface' : ''}" 
  on:contextmenu|preventDefault={(e) => handleContextMenu(e, null)} 
  role="presentation"
  on:dragover|preventDefault={handleDragOver}
  on:dragleave={handleDragLeave}
  on:drop|preventDefault={handleDrop}
>
  {#if editorOpen}
    <div class="fixed inset-0 z-50 flex items-center justify-center" role="presentation">
      <button type="button" class="absolute inset-0 bg-slate-900/60 dark:bg-black/60" on:click={closeEditor} aria-label="关闭编辑器"></button>
      <div class="relative w-[min(900px,95vw)] h-[min(700px,90vh)] bg-app-bg border border-app-border rounded-lg shadow-xl flex flex-col" role="dialog" aria-modal="true">
        <div class="flex items-center justify-between px-4 py-2 border-b border-app-border gap-3">
          <div class="text-sm text-app-text truncate flex-1">{editorFile?.path}</div>
          <div class="flex items-center gap-2 flex-none">
            <button class="px-3 py-1 rounded bg-app-surface hover:bg-app-bg-hover text-app-text disabled:opacity-60" on:click={closeEditor} disabled={editorSaving}>
              关闭
            </button>
            <button class="px-3 py-1 rounded bg-primary-600 hover:bg-primary-500 text-white disabled:opacity-60" on:click={saveEditor} disabled={editorSaving || editorLoading || !editorFile}>
              {editorSaving ? '保存中…' : '保存'}
            </button>
          </div>
        </div>
        {#if editorLoading}
          <div class="flex-1 flex items-center justify-center text-app-text-secondary">加载中…</div>
        {:else}
          <textarea class="flex-1 w-full bg-app-surface text-app-text font-mono text-sm p-3 outline-none resize-none" bind:value={editorContent} disabled={editorSaving}></textarea>
        {/if}
        {#if editorError}
          <div class="px-4 py-2 border-t border-app-border text-red-600 dark:text-red-400 text-sm">{editorError}</div>
        {/if}
      </div>
    </div>
  {/if}
  {#if isDragging}
    <div class="absolute inset-0 bg-primary-500/20 flex items-center justify-center z-50 pointer-events-none">
      <div class="text-2xl font-bold text-primary-600 dark:text-primary-200">Drop files to upload</div>
    </div>
  {/if}
  <!-- Toolbar -->
  <div class="flex items-center p-2 border-b border-app-border space-x-2">
    <button 
        class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary" 
        on:click={() => loadFiles('.')} 
        title="Home"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path d="M10.707 2.293a1 1 0 00-1.414 0l-7 7a1 1 0 001.414 1.414L4 10.414V17a1 1 0 001 1h2a1 1 0 001-1v-2a1 1 0 011-1h2a1 1 0 011 1v2a1 1 0 001 1h2a1 1 0 001-1v-6.586l.293.293a1 1 0 001.414-1.414l-7-7z" />
      </svg>
    </button>
    <button 
        class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary" 
        on:click={() => loadFiles(currentPath, { force: true })} 
        title="Refresh"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v3.276a1 1 0 01-2 0V14.907a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z" clip-rule="evenodd" />
      </svg>
    </button>
    <button 
        class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary" 
        on:click={() => {
            const currentWidth = get(settings).ui.rightSidebarWidth;
            const maxWidth = window.innerWidth - 50;
            if (currentWidth >= maxWidth - 50) {
                settings.update(s => ({ ...s, ui: { ...s.ui, rightSidebarWidth: 400 } }));
            } else {
                settings.update(s => ({ ...s, ui: { ...s.ui, rightSidebarWidth: maxWidth } }));
            }
        }}
        title="Full Screen"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M3 4a1 1 0 011-1h4a1 1 0 010 2H6.414l2.293 2.293a1 1 0 11-1.414 1.414L5 6.414V8a1 1 0 01-2 0V4zm9 1a1 1 0 010-2h4a1 1 0 011 1v4a1 1 0 01-2 0V6.414l-2.293 2.293a1 1 0 11-1.414-1.414L13.586 5H12zm-9 7a1 1 0 011 1v1.586l2.293-2.293a1 1 0 111.414 1.414L5.414 15H7a1 1 0 010 2H3a1 1 0 01-1-1v-4a1 1 0 011-1zm13.414-1.414a1 1 0 011.414 0l2.293 2.293V12a1 1 0 012 0v4a1 1 0 01-1 1h-4a1 1 0 010-2h1.586l-2.293-2.293a1 1 0 010-1.414z" clip-rule="evenodd" />
      </svg>
    </button>
    <button 
        class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary" 
        on:click={() => fileInput.click()} 
        title="Upload File"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zM6.293 6.707a1 1 0 010-1.414l3-3a1 1 0 011.414 0l3 3a1 1 0 01-1.414 1.414L11 5.414V13a1 1 0 11-2 0V5.414L7.707 6.707a1 1 0 01-1.414 0z" clip-rule="evenodd" />
      </svg>
    </button>
    <input 
      type="file" 
      multiple
      bind:this={fileInput} 
      on:change={handleFileUpload} 
      style="display: none;" 
    />
    <input 
      class="flex-1 bg-app-surface border border-app-border rounded px-2 py-1 text-sm text-app-text"
      value={currentPath}
      on:change={(e) => loadFiles(e.currentTarget.value)}
    />
  </div>

  <!-- File List -->
  <div class="flex-1 overflow-auto" role="grid">
    {#if loading}
      <div class="absolute inset-0 top-10 bg-white/50 dark:bg-gray-900/50 flex items-center justify-center z-10">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary-500 dark:border-white"></div>
      </div>
    {/if}

    {#if error}
      <div class="p-4 text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20 m-2 rounded">
        Error: {error}
      </div>
    {/if}

    <table class="w-full text-sm text-left border-collapse">
      <thead class="bg-app-surface sticky top-0 text-app-text-secondary text-xs uppercase font-semibold">
        <tr>
          <th class="p-2 border-b border-app-border">Name</th>
          <th class="p-2 border-b border-app-border w-24">Size</th>
          <th class="p-2 border-b border-app-border w-40">Modified</th>
          <th class="p-2 border-b border-app-border w-20">Perms</th>
        </tr>
      </thead>
      <tbody>
        {#each files as file, i}
          {@const iconType = getFileIcon(file.name, file.isDirectory)}
          <tr
            id={'file-row-' + i}
            class="cursor-pointer border-b border-app-border transition-colors duration-75 {selectedPaths.has(file.path) ? 'bg-primary-100 dark:bg-primary-900/30' : 'hover:bg-app-bg-hover'}"
            on:click|stopPropagation={(e) => handleSelect(e, file)}
            on:dblclick={() => openEditor(file)}
            on:contextmenu|preventDefault|stopPropagation={(e) => handleContextMenu(e, file)}
            draggable={!file.isDirectory}
            on:dragstart={(e) => handleDragStart(e, file)}
          >
            <td class="p-2 flex items-center space-x-2">
              <FileIcon iconType={iconType} />
              <span class={file.isDirectory ? 'font-medium text-app-text' : 'text-app-text'}>{file.name}</span>
            </td>
            <td class="p-2 text-app-text-secondary font-mono text-xs">{file.isDirectory ? '-' : formatSize(file.size)}</td>
            <td class="p-2 text-app-text-secondary text-xs">{formatDate(file.modified)}</td>
            <td class="p-2 text-app-text-secondary font-mono text-xs">{file.permissions}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if contextMenu.show}
    <ContextMenu 
      x={contextMenu.x} 
      y={contextMenu.y} 
      on:close={closeContextMenu}
    >
      {#if contextMenu.file}
        {@const fileIcon = getFileIcon(contextMenu.file.name, contextMenu.file.isDirectory)}
        <div class="px-3 py-1.5 border-b border-app-border/50 flex items-center space-x-2 text-sm text-app-text-secondary">
          <FileIcon iconType={fileIcon} />
          <span class="truncate font-medium">
            {selectedPaths.size > 1 ? `${selectedPaths.size} items selected` : contextMenu.file.name}
          </span>
        </div>
        {#if !contextMenu.file.isDirectory}
          {#if selectedPaths.size === 1}
            <ContextMenuItem on:click={handleDownload} label="下载">
              <svg slot="icon" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"/>
              </svg>
            </ContextMenuItem>
          {/if}
          <ContextMenuItem on:click={handleCopy} label="复制">
            <svg slot="icon" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"/>
            </svg>
          </ContextMenuItem>
        {/if}
        {#if selectedPaths.size === 1}
          <ContextMenuItem on:click={handleRename} label="重命名">
            <svg slot="icon" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
            </svg>
          </ContextMenuItem>
        {/if}
        <ContextMenuItem on:click={handleDelete} label="删除" danger>
          <svg slot="icon" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
          </svg>
        </ContextMenuItem>
        <ContextMenuDivider />
      {/if}
      <ContextMenuItem on:click={handleCreateFolder} label="新建文件夹">
        <svg slot="icon" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z"/>
        </svg>
      </ContextMenuItem>
      <ContextMenuItem on:click={handleCreateFile} label="新建文件">
        <svg slot="icon" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"/>
        </svg>
      </ContextMenuItem>
      <ContextMenuItem on:click={handlePaste} label="粘贴" disabled={!$fileClipboard}>
        <svg slot="icon" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"/>
        </svg>
      </ContextMenuItem>
      <ContextMenuItem on:click={() => loadFiles(currentPath, { force: true })} label="刷新">
        <svg slot="icon" class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
        </svg>
      </ContextMenuItem>
    </ContextMenu>
  {/if}
</div>
