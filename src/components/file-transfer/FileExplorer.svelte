<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { save } from '@tauri-apps/plugin-dialog';
  import { invoke } from '@tauri-apps/api/core';
  import { sftpService } from '../../lib/sftpService';
  import { localFsService } from '../../lib/localFsService';
  import { fileClipboard, settings } from '../../lib/store';
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
  let isCrossDragging = false;

  let editorOpen = false;
  let editorFile: FileEntry | null = null;
  let editorContent = '';
  let editorLoading = false;
  let editorSaving = false;
  let editorError: string | null = null;

  function joinPath(base: string, name: string): string {
    if (base === '/' || base === '') return `/${name}`;
    return `${base}/${name}`.replace('//', '/');
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

  // 解析快捷键为修饰键和主键
  function parseShortcut(shortcut: string): { ctrl: boolean; meta: boolean; shift: boolean; alt: boolean; key: string } | null {
    if (!shortcut) return null;
    const parts = shortcut.split('+').map(p => p.trim()).filter(Boolean);
    if (parts.length === 0) return null;

    const modifiers = { ctrl: false, meta: false, shift: false, alt: false };
    let keyPart = '';

    for (const part of parts) {
      const lower = part.toLowerCase();
      if (lower === 'ctrl' || lower === 'control') modifiers.ctrl = true;
      else if (lower === 'meta' || lower === 'cmd' || lower === 'command') modifiers.meta = true;
      else if (lower === 'shift') modifiers.shift = true;
      else if (lower === 'alt' || lower === 'option') modifiers.alt = true;
      else keyPart = part;
    }

    if (!keyPart) return null;
    return { ...modifiers, key: keyPart.toLowerCase() };
  }

  // 检查键盘事件是否匹配快捷键
  function matchesShortcut(e: KeyboardEvent, shortcut: string): boolean {
    const parsed = parseShortcut(shortcut);
    if (!parsed) return false;

    const eventKey = e.key.toLowerCase();
    const eventModifiers = {
      ctrl: e.ctrlKey,
      meta: e.metaKey,
      shift: e.shiftKey,
      alt: e.altKey
    };

    return (
      eventKey === parsed.key &&
      eventModifiers.ctrl === parsed.ctrl &&
      eventModifiers.meta === parsed.meta &&
      eventModifiers.shift === parsed.shift &&
      eventModifiers.alt === parsed.alt
    );
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
    if (!options?.force && loading && lastRequestedPath === path) return;

    lastRequestedPath = path;
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
      const cached = directoryCache.get(getCacheKey(path));
      if (cached && Date.now() - cached.ts <= CACHE_TTL_MS) {
        files = cached.files;
        currentPath = path;
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

      const nextFiles = await Promise.race([sftpService.listDirectory(sessionId, path), abortPromise]);
      if (requestId !== loadSequence) return;
      sortFiles(nextFiles);
      files = nextFiles;
      currentPath = path;
      setCache(path, nextFiles);
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
    
    const path = joinPath(getMenuTargetDirectory(), name);
    try {
      await sftpService.createDirectory(sessionId, path);
      invalidateCache(currentPath);
      loadFiles(currentPath, { force: true });
    } catch (e: any) {
      error = e.toString();
    }
  }

  async function handleCreateFile() {
    closeContextMenu();
    const name = prompt('请输入文件名:');
    if (!name) return;

    const path = joinPath(getMenuTargetDirectory(), name);
    try {
      try {
        await sftpService.writeFile(sessionId, path, new Uint8Array(0), false);
      } catch (e) {
        await sftpService.scpUpload(sessionId, path, new Uint8Array(0));
      }
      invalidateCache(currentPath);
      loadFiles(currentPath, { force: true });
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

    try {
      for (const file of selectedFiles) {
        if (file.isDirectory) {
          await sftpService.removeDirectory(sessionId, file.path);
        } else {
          await sftpService.removeFile(sessionId, file.path);
        }
      }
      invalidateCache(currentPath);
      loadFiles(currentPath, { force: true });
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

    try {
      await sftpService.rename(sessionId, file.path, newPath);
      invalidateCache(currentPath);
      loadFiles(currentPath, { force: true });
    } catch (e: any) {
      error = e.toString();
    }
  }

  function handleDragOver() {
    isDragging = true;
  }

  function handleDragLeave() {
    isDragging = false;
  }

  async function handleDrop(e: DragEvent) {
    isDragging = false;
    const payload = e.dataTransfer?.getData('application/x-starshuttle-file');
    if (payload) {
      try {
        const data = JSON.parse(payload);
        if (data?.source === 'local' && data?.path && data?.name) {
          loading = true;
          try {
            const content = await localFsService.readFile(data.path);
            const remotePath = currentPath === '/' ? `/${data.name}` : `${currentPath}/${data.name}`.replace('//', '/');
            try {
              await sftpService.writeFile(sessionId, remotePath, content, false);
            } catch (err) {
              await sftpService.scpUpload(sessionId, remotePath, content);
            }
            invalidateCache(currentPath);
            await loadFiles(currentPath, { force: true });
          } finally {
            loading = false;
          }
          return;
        }
      } catch {
        isCrossDragging = false;
      }
    }

    const items = e.dataTransfer?.files;
    if (!items || items.length === 0) return;

    await uploadFiles(Array.from(items));
  }

  async function handleFileUpload(e: Event) {
    const input = e.target as HTMLInputElement;
    if (!input.files || input.files.length === 0) return;
    
    await uploadFiles(Array.from(input.files));
    input.value = ''; // Reset input
  }

  async function uploadFiles(filesToUpload: File[]) {
    loading = true;
    try {
      for (const file of filesToUpload) {
        await uploadSingleFile(file);
      }
      invalidateCache(currentPath);
      await loadFiles(currentPath, { force: true });
    } catch (e: any) {
      error = e.toString();
    } finally {
      loading = false;
    }
  }

  async function uploadSingleFile(file: File): Promise<void> {
    const path = currentPath === '/' ? `/${file.name}` : `${currentPath}/${file.name}`.replace('//', '/');
    
    // Chunked upload
    const CHUNK_SIZE = 1024 * 1024; // 1MB
    const totalChunks = Math.ceil(file.size / CHUNK_SIZE);

    if (file.size === 0) {
      try {
        await sftpService.writeFile(sessionId, path, new Uint8Array(0), false);
      } catch (e) {
        await sftpService.scpUpload(sessionId, path, new Uint8Array(0));
      }
      return;
    }
    
    try {
      for (let i = 0; i < totalChunks; i++) {
        const start = i * CHUNK_SIZE;
        const end = Math.min(start + CHUNK_SIZE, file.size);
        const chunk = file.slice(start, end);
        
        await new Promise<void>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = async () => {
            if (!reader.result) { resolve(); return; }
            const content = new Uint8Array(reader.result as ArrayBuffer);
            try {
              // First chunk overwrites/creates, subsequent chunks append
              await sftpService.writeFile(sessionId, path, content, i > 0);
              resolve();
            } catch (e) {
              reject(e);
            }
          };
          reader.onerror = () => reject(new Error("Failed to read file chunk"));
          reader.readAsArrayBuffer(chunk);
        });
      }
    } catch (e) {
      const full = new Uint8Array(await file.arrayBuffer());
      await sftpService.scpUpload(sessionId, path, full);
    }
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
      let content: Uint8Array;
      try {
        content = await sftpService.readFile(sessionId, file.path);
      } catch (e) {
        content = await sftpService.scpDownload(sessionId, file.path);
      }

      // 使用保存对话框让用户选择保存位置
      const filePath = await save({
        filters: [{
          name: file.name.split('.').pop() || 'All Files',
          extensions: file.name.includes('.') ? [file.name.split('.').pop()!] : ['*']
        }],
        defaultPath: file.name
      });

      if (!filePath) {
        loading = false;
        return; // 用户取消了保存
      }

      // 使用 Rust 端的命令保存二进制文件到本地
      await invoke('save_file_to_local', {
        path: filePath,
        content: Array.from(content) // 将 Uint8Array 转换为普通数组，以便通过 IPC 传输
      });

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
    try {
      for (const entry of item.entries) {
        if (entry.isDirectory) continue; // Skip directories for now
        
        const destPath = joinPath(destDir, entry.name);
        
        let content: Uint8Array;
        if (item.source === 'local') {
          content = await localFsService.readFile(entry.path);
        } else {
          if (!item.sessionId) continue;
          try {
            content = await sftpService.readFile(item.sessionId, entry.path);
          } catch (e) {
            content = await sftpService.scpDownload(item.sessionId, entry.path);
          }
        }

        try {
          await sftpService.writeFile(sessionId, destPath, content, false);
        } catch (e) {
          await sftpService.scpUpload(sessionId, destPath, content);
        }
      }

      invalidateCache(currentPath);
      await loadFiles(currentPath, { force: true });
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (editorOpen) return;
    const target = e.target as HTMLElement | null;
    if (
      target &&
      (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || (target as any).isContentEditable)
    ) {
      return;
    }

    const copyShortcut = $settings.shortcuts.copy;
    const pasteShortcut = $settings.shortcuts.paste;
    const renameShortcut = $settings.shortcuts.fileBrowserRename;
    const deleteShortcut = $settings.shortcuts.fileBrowserDelete;
    const openShortcut = $settings.shortcuts.fileBrowserOpen;
    const backShortcut = $settings.shortcuts.fileBrowserBack;
    const selectAllShortcut = $settings.shortcuts.fileBrowserSelectAll;

    if (matchesShortcut(e, selectAllShortcut)) {
      e.preventDefault();
      selectedPaths.clear();
      files.forEach(f => selectedPaths.add(f.path));
      selectedPaths = selectedPaths;
      return;
    }

    if (matchesShortcut(e, copyShortcut)) {
      e.preventDefault();
      handleCopy();
      return;
    }

    if (matchesShortcut(e, pasteShortcut)) {
      e.preventDefault();
      void handlePaste();
      return;
    }

    const refreshShortcut = $settings.shortcuts.fileBrowserRefresh;
    const newFolderShortcut = $settings.shortcuts.fileBrowserNewFolder;
    const newFileShortcut = $settings.shortcuts.fileBrowserNewFile;

    if (matchesShortcut(e, refreshShortcut)) {
      e.preventDefault();
      loadFiles(currentPath, { force: true });
      return;
    }

    if (matchesShortcut(e, newFolderShortcut)) {
      e.preventDefault();
      void handleCreateFolder();
      return;
    }

    if (matchesShortcut(e, newFileShortcut)) {
      e.preventDefault();
      void handleCreateFile();
      return;
    }

    if (matchesShortcut(e, renameShortcut)) {
      e.preventDefault();
      void handleRename();
      return;
    }

    if (matchesShortcut(e, deleteShortcut)) {
      e.preventDefault();
      void handleDelete();
      return;
    }

    if (matchesShortcut(e, openShortcut)) {
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

    if (matchesShortcut(e, backShortcut)) {
      e.preventDefault();
      if (currentPath !== '/' && currentPath !== '') {
        loadFiles('..');
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
    window.addEventListener('click', closeContextMenu);
    window.addEventListener('keydown', handleKeydown);
    return () => {
      window.removeEventListener('click', closeContextMenu);
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

  function handleDragEnter(e: DragEvent) {
    const payload = e.dataTransfer?.getData('application/x-starshuttle-file');
    if (!payload) return;
    try {
      const data = JSON.parse(payload);
      if (data?.source === 'local') isCrossDragging = true;
    } catch {
      isCrossDragging = false;
    }
  }

  function handleDragLeaveCross() {
    isCrossDragging = false;
  }

  async function openEditor(file: FileEntry) {
    if (file.isDirectory) {
      handleNavigate(file.path);
      return;
    }
    editorOpen = true;
    editorFile = file;
    editorContent = '';
    editorError = null;
    editorLoading = true;
    try {
      let content: Uint8Array;
      try {
        content = await sftpService.readFile(sessionId, file.path);
      } catch (e) {
        content = await sftpService.scpDownload(sessionId, file.path);
      }
      if (content.byteLength > 2 * 1024 * 1024) {
        throw new Error('文件过大，暂不支持直接编辑（> 2MB）');
      }
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
    editorContent = '';
    editorError = null;
    editorLoading = false;
  }

  async function saveEditor() {
    if (!editorFile || editorSaving) return;
    editorSaving = true;
    editorError = null;
    try {
      const content = new TextEncoder().encode(editorContent);
      try {
        await sftpService.writeFile(sessionId, editorFile.path, content, false);
      } catch (e) {
        await sftpService.scpUpload(sessionId, editorFile.path, content);
      }
      invalidateCache(currentPath);
      await loadFiles(currentPath, { force: true });
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
  on:dragenter|preventDefault={handleDragEnter}
  on:dragleave={handleDragLeave}
  on:drop|preventDefault={handleDrop}
  on:dragleave|self={handleDragLeaveCross}
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
  {#if isCrossDragging}
    <div class="absolute inset-0 bg-primary-500/10 flex items-center justify-center z-40 pointer-events-none">
      <div class="text-lg font-semibold text-primary-600 dark:text-primary-200">拖拽到此处上传到远程</div>
    </div>
  {/if}
  <!-- Toolbar -->
  <div class="flex items-center p-2 border-b border-app-border space-x-2">
    <button 
        class="p-1 hover:bg-app-bg-hover rounded text-app-text-secondary" 
        on:click={() => loadFiles('..')} 
        title="Up"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M14.707 12.707a1 1 0 01-1.414 0L10 9.414l-3.293 3.293a1 1 0 01-1.414-1.414l4-4a1 1 0 011.414 0l4 4a1 1 0 010 1.414z" clip-rule="evenodd" />
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
        on:click={handleCreateFolder} 
        title="New Folder"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1H8a3 3 0 00-3 3v1.5a1.5 1.5 0 01-3 0V6z" clip-rule="evenodd" />
        <path d="M6 12a2 2 0 012-2h8a2 2 0 012 2v2a2 2 0 01-2 2H2h2a2 2 0 002-2v-2z" />
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
    <div
      class="fixed bg-app-bg border border-app-border rounded shadow-lg py-1 z-50 text-sm min-w-[180px]"
      style="top: {contextMenu.y}px; left: {contextMenu.x}px"
      role="menu"
      tabindex="-1"
    >
      {#if contextMenu.file}
        {@const fileIcon = getFileIcon(contextMenu.file.name, contextMenu.file.isDirectory)}
        <div class="px-4 py-2 border-b border-app-border flex items-center space-x-2">
          <FileIcon iconType={fileIcon} />
          <span class="truncate font-medium text-app-text">
            {selectedPaths.size > 1 ? `${selectedPaths.size} items selected` : contextMenu.file.name}
          </span>
        </div>
        {#if !contextMenu.file.isDirectory}
          {#if selectedPaths.size === 1}
            <button
              class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text flex items-center space-x-2"
              on:click|stopPropagation={handleDownload}
            >
              <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"/>
              </svg>
              <span>下载</span>
            </button>
          {/if}
          <button
            class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text flex items-center space-x-2"
            on:click|stopPropagation={handleCopy}
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"/>
            </svg>
            <span>复制</span>
          </button>
        {/if}
        {#if selectedPaths.size === 1}
          <button
            class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text flex items-center space-x-2"
            on:click|stopPropagation={handleRename}
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
            </svg>
            <span>重命名</span>
          </button>
        {/if}
        <button
          class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-red-600 dark:text-red-400 hover:text-red-500 dark:hover:text-red-300 flex items-center space-x-2"
          on:click|stopPropagation={handleDelete}
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
          </svg>
          <span>删除</span>
        </button>
        <div class="border-t border-app-border my-1"></div>
      {/if}
      <button
        class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text flex items-center space-x-2"
        on:click|stopPropagation={handleCreateFolder}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z"/>
        </svg>
        <span>新建文件夹</span>
      </button>
      <button
        class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text flex items-center space-x-2"
        on:click|stopPropagation={handleCreateFile}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"/>
        </svg>
        <span>新建文件</span>
      </button>
      <button
        class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text flex items-center space-x-2 disabled:opacity-60"
        on:click|stopPropagation={handlePaste}
        disabled={!$fileClipboard}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"/>
        </svg>
        <span>粘贴</span>
      </button>
      <button
        class="w-full text-left px-4 py-2 hover:bg-app-bg-hover text-app-text flex items-center space-x-2"
        on:click|stopPropagation={() => loadFiles(currentPath, { force: true })}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
        </svg>
        <span>刷新</span>
      </button>
    </div>
  {/if}
</div>
