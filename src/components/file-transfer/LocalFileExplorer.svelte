<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { localFsService } from '../../lib/localFsService';
  import { sftpService } from '../../lib/sftpService';
  import { fileClipboard } from '../../lib/store';
  import type { FileEntry } from '../../types';

  export let initialPath: string = '.'; // Default to current directory

  let currentPath = initialPath;
  let files: FileEntry[] = [];
  let loading = false;
  let error: string | null = null;
  let selectedFile: FileEntry | null = null;
  let contextMenu = { x: 0, y: 0, show: false, file: null as FileEntry | null };
  let fileInput: HTMLInputElement;
  let isDragging = false;
  let isCrossDragging = false;

  function normalizePath(path: string): string {
    return path.replace(/\\/g, '/');
  }

  async function resolvePath(path: string): Promise<string> {
    if (path === '~') return normalizePath(await localFsService.getHomeDir());
    if (path === '' || path === '.') return '.';
    return normalizePath(path);
  }

  function dirname(path: string): string {
    const normalized = normalizePath(path).replace(/\/+$/, '');
    const matchDrive = normalized.match(/^[A-Za-z]:$/);
    if (matchDrive) return `${normalized}/`;
    const lastSlash = normalized.lastIndexOf('/');
    if (lastSlash < 0) return '';
    if (lastSlash === 0) return '/';
    const parent = normalized.slice(0, lastSlash);
    if (/^[A-Za-z]:$/.test(parent)) return `${parent}/`;
    return parent;
  }

  function joinPath(base: string, name: string): string {
    const baseNormalized = normalizePath(base);
    const baseTrimmed = baseNormalized.replace(/\/+$/, '');
    if (baseTrimmed === '') return `/${name}`;
    return `${baseTrimmed}/${name}`;
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

  // 获取图标的 SVG
  function getIconSvg(iconType: string): string {
    const icons: Record<string, string> = {
      folder: `<svg class="w-4 h-4 text-blue-500 dark:text-blue-400" fill="currentColor" viewBox="0 0 20 20">
        <path d="M2 6a2 2 0 012-2h5l2 2h5a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z"/>
      </svg>`,

      folder_open: `<svg class="w-4 h-4 text-blue-500 dark:text-blue-400" fill="currentColor" viewBox="0 0 20 20">
        <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1H8a3 3 0 00-3 3v1.5a1.5 1.5 0 01-3 0V6z" clip-rule="evenodd"/>
        <path d="M6 12a2 2 0 012-2h8a2 2 0 012 2v2a2 2 0 01-2 2H2h2a2 2 0 002-2v-2z"/>
      </svg>`,

      code: `<svg class="w-4 h-4 text-purple-500 dark:text-purple-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"/>
      </svg>`,

      settings: `<svg class="w-4 h-4 text-orange-500 dark:text-orange-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"/>
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"/>
      </svg>`,

      style: `<svg class="w-4 h-4 text-pink-500 dark:text-pink-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01"/>
      </svg>`,

      image: `<svg class="w-4 h-4 text-green-500 dark:text-green-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z"/>
      </svg>`,

      video: `<svg class="w-4 h-4 text-red-500 dark:text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 10l4.553-2.276A1 1 0 0121 8.618v6.764a1 1 0 01-1.447.894L15 14M5 18h8a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z"/>
      </svg>`,

      audio: `<svg class="w-4 h-4 text-yellow-500 dark:text-yellow-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 19V6l12-3v13M9 19c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zm12-3c0 1.105-1.343 2-3 2s-3-.895-3-2 1.343-2 3-2 3 .895 3 2zM9 10l12-3"/>
      </svg>`,

      archive: `<svg class="w-4 h-4 text-amber-600 dark:text-amber-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4"/>
      </svg>`,

      document: `<svg class="w-4 h-4 text-blue-600 dark:text-blue-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
      </svg>`,

      markdown: `<svg class="w-4 h-4 text-slate-600 dark:text-slate-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 20l4-16m4 4l4 4-4 4M6 16l-4-4 4-4"/>
      </svg>`,

      text: `<svg class="w-4 h-4 text-gray-500 dark:text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
      </svg>`,

      database: `<svg class="w-4 h-4 text-cyan-500 dark:text-cyan-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 7v10c0 2.21 3.582 4 8 4s8-1.79 8-4V7M4 7c0 2.21 3.582 4 8 4s8-1.79 8-4M4 7c0-2.21 3.582-4 8-4s8 1.79 8 4m0 5c0 2.21-3.582 4-8 4s-8-1.79-8-4"/>
      </svg>`,

      certificate: `<svg class="w-4 h-4 text-emerald-500 dark:text-emerald-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"/>
      </svg>`,

      executable: `<svg class="w-4 h-4 text-red-600 dark:text-red-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.752 11.168l-3.197-2.132A1 1 0 0010 9.87v4.263a1 1 0 001.555.832l3.197-2.132a1 1 0 000-1.664z"/>
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 11-18 0 9 9 0 0118 0z"/>
      </svg>`,

      file: `<svg class="w-4 h-4 text-gray-400 dark:text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"/>
      </svg>`,
    };

    return icons[iconType] || icons.file;
  }

  async function loadFiles(path: string) {
    loading = true;
    error = null;
    selectedFile = null;
    contextMenu.show = false;
    try {
      const resolvedPath = await resolvePath(path);
      files = await localFsService.listDirectory(resolvedPath);
      currentPath = resolvedPath;
      
      // Sort: Directories first, then files
      files.sort((a, b) => {
        if (a.isDirectory === b.isDirectory) {
          return a.name.localeCompare(b.name);
        }
        return a.isDirectory ? -1 : 1;
      });
    } catch (e: any) {
      error = e.toString();
    } finally {
      loading = false;
    }
  }

  function handleNavigate(path: string) {
    loadFiles(path);
  }

  function handleUp() {
    const normalized = normalizePath(currentPath);
    if (normalized === '' || normalized === '/') return;
    if (normalized === '.') {
      loadFiles('..');
      return;
    }
    if (/^[A-Za-z]:\/?$/.test(normalized)) return;
    const parent = dirname(normalized);
    loadFiles(parent === '' ? '..' : parent);
  }

  function handleContextMenu(e: MouseEvent, file: FileEntry | null) {
    e.preventDefault();
    selectedFile = file;
    contextMenu = {
      x: e.clientX,
      y: e.clientY,
      show: true,
      file
    };
  }

  function getMenuTargetDirectory() {
    if (contextMenu.file?.isDirectory) return contextMenu.file.path;
    return currentPath;
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
      await localFsService.createDirectory(path);
      loadFiles(currentPath);
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
      await localFsService.writeFile(path, new Uint8Array(0), false);
      loadFiles(currentPath);
    } catch (e: any) {
      error = e.toString();
    }
  }

  async function handleDelete() {
    closeContextMenu();
    if (!selectedFile) return;
    
    if (!confirm(`确定要删除 ${selectedFile.name} 吗？`)) return;

    try {
      if (selectedFile.isDirectory) {
        await localFsService.removeDirectory(selectedFile.path);
      } else {
        await localFsService.removeFile(selectedFile.path);
      }
      loadFiles(currentPath);
    } catch (e: any) {
      error = e.toString();
    }
  }

  async function handleRename() {
    closeContextMenu();
    if (!selectedFile) return;

    const newName = prompt('请输入新名称:', selectedFile.name);
    if (!newName || newName === selectedFile.name) return;

    // Construct new path. 
    // Assumption: selectedFile.path is the full path. We need to replace the filename.
    const parentPath = dirname(selectedFile.path);
    const newPath = parentPath === '' ? newName : joinPath(parentPath, newName);

    try {
      await localFsService.rename(selectedFile.path, newPath);
      loadFiles(currentPath);
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
        if (data?.source === 'remote' && data?.sessionId && data?.path && data?.name) {
          loading = true;
          try {
            let content: Uint8Array;
            try {
              content = await sftpService.readFile(data.sessionId, data.path);
            } catch (err) {
              content = await sftpService.scpDownload(data.sessionId, data.path);
            }
            const localPath = joinPath(currentPath, data.name);
            await localFsService.writeFile(localPath, content, false);
            await loadFiles(currentPath);
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
      loadFiles(currentPath);
    } catch (e: any) {
      error = e.toString();
    } finally {
      loading = false;
    }
  }

  async function uploadSingleFile(file: File): Promise<void> {
    const path = joinPath(currentPath, file.name);
    
    // Read file as ArrayBuffer
    const arrayBuffer = await file.arrayBuffer();
    const content = new Uint8Array(arrayBuffer);
    
    await localFsService.writeFile(path, content, false);
  }

  async function handleDownload() {
    closeContextMenu();
    if (!selectedFile || selectedFile.isDirectory) return;
    
    loading = true;
    try {
      const content = await localFsService.readFile(selectedFile.path);
      const blob = new Blob([content as any]);
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = selectedFile.name;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
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
    if (!selectedFile) return;
    fileClipboard.set({
      source: 'local',
      path: selectedFile.path,
      name: selectedFile.name,
      isDirectory: selectedFile.isDirectory,
      operation: 'copy',
    });
  }

  async function handlePaste() {
    closeContextMenu();
    const item = get(fileClipboard);
    if (!item) return;
    if (item.isDirectory) return;

    const destDir = selectedFile?.isDirectory ? selectedFile.path : currentPath;
    const destPath = joinPath(destDir, item.name);

    loading = true;
    error = null;
    try {
      if (item.source === 'local') {
        const content = await localFsService.readFile(item.path);
        await localFsService.writeFile(destPath, content, false);
      } else {
        if (!item.sessionId) return;
        let content: Uint8Array;
        try {
          content = await sftpService.readFile(item.sessionId, item.path);
        } catch (e) {
          content = await sftpService.scpDownload(item.sessionId, item.path);
        }
        await localFsService.writeFile(destPath, content, false);
      }
      await loadFiles(currentPath);
    } catch (e: any) {
      error = e?.message ?? String(e);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    if (
      target &&
      (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || (target as any).isContentEditable)
    ) {
      return;
    }
    const isModifier = e.ctrlKey || e.metaKey;
    if (!isModifier) return;
    const key = String(e.key).toLowerCase();
    if (key === 'c') {
      e.preventDefault();
      if (selectedFile) {
        fileClipboard.set({
          source: 'local',
          path: selectedFile.path,
          name: selectedFile.name,
          isDirectory: selectedFile.isDirectory,
          operation: 'copy',
        });
      }
      return;
    }
    if (key === 'v') {
      e.preventDefault();
      void handlePaste();
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
        source: 'local',
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
      if (data?.source === 'remote') isCrossDragging = true;
    } catch {
      isCrossDragging = false;
    }
  }

  function handleDragLeaveCross() {
    isCrossDragging = false;
  }
</script>

<div 
  class="flex flex-col h-full bg-white dark:bg-gray-900 text-slate-900 dark:text-white relative {isDragging ? 'border-2 border-blue-500 bg-slate-50 dark:bg-gray-800' : ''}" 
  on:contextmenu|preventDefault={(e) => handleContextMenu(e, null)} 
  role="presentation"
  on:dragover|preventDefault={handleDragOver}
  on:dragenter|preventDefault={handleDragEnter}
  on:dragleave={handleDragLeave}
  on:drop|preventDefault={handleDrop}
  on:dragleave|self={handleDragLeaveCross}
>
  {#if isDragging}
    <div class="absolute inset-0 bg-blue-500/20 flex items-center justify-center z-50 pointer-events-none">
      <div class="text-2xl font-bold text-blue-600 dark:text-blue-200">Drop files to upload</div>
    </div>
  {/if}
  {#if isCrossDragging}
    <div class="absolute inset-0 bg-blue-500/10 flex items-center justify-center z-40 pointer-events-none">
      <div class="text-lg font-semibold text-blue-600 dark:text-blue-200">拖拽到此处下载到本地</div>
    </div>
  {/if}
  <!-- Toolbar -->
  <div class="flex items-center p-2 border-b border-slate-200 dark:border-gray-700 space-x-2">
    <button 
        class="p-1 hover:bg-slate-200 dark:hover:bg-gray-700 rounded text-slate-600 dark:text-gray-300" 
        on:click={handleUp} 
        title="Up"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M14.707 12.707a1 1 0 01-1.414 0L10 9.414l-3.293 3.293a1 1 0 01-1.414-1.414l4-4a1 1 0 011.414 0l4 4a1 1 0 010 1.414z" clip-rule="evenodd" />
      </svg>
    </button>
    <button 
        class="p-1 hover:bg-slate-200 dark:hover:bg-gray-700 rounded text-slate-600 dark:text-gray-300" 
        on:click={() => loadFiles(currentPath)} 
        title="Refresh"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v3.276a1 1 0 01-2 0V14.907a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z" clip-rule="evenodd" />
      </svg>
    </button>
    <button 
        class="p-1 hover:bg-slate-200 dark:hover:bg-gray-700 rounded text-slate-600 dark:text-gray-300" 
        on:click={handleCreateFolder} 
        title="New Folder"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1H8a3 3 0 00-3 3v1.5a1.5 1.5 0 01-3 0V6z" clip-rule="evenodd" />
        <path d="M6 12a2 2 0 012-2h8a2 2 0 012 2v2a2 2 0 01-2 2H2h2a2 2 0 002-2v-2z" />
      </svg>
    </button>
    <button 
        class="p-1 hover:bg-slate-200 dark:hover:bg-gray-700 rounded text-slate-600 dark:text-gray-300" 
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
      class="flex-1 bg-white dark:bg-gray-800 border border-slate-300 dark:border-gray-600 rounded px-2 py-1 text-sm text-slate-900 dark:text-gray-200"
      value={currentPath}
      on:change={(e) => loadFiles(e.currentTarget.value)}
    />
  </div>

  <!-- File List -->
  <div class="flex-1 overflow-auto" role="grid">
    {#if loading}
      <div class="absolute inset-0 top-10 bg-white/50 dark:bg-gray-900/50 flex items-center justify-center z-10">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-blue-500 dark:border-white"></div>
      </div>
    {/if}

    {#if error}
      <div class="p-4 text-red-600 dark:text-red-400 bg-red-50 dark:bg-red-900/20 m-2 rounded">
        Error: {error}
      </div>
    {/if}

    <table class="w-full text-sm text-left border-collapse">
      <thead class="bg-slate-100 dark:bg-gray-800 sticky top-0 text-slate-500 dark:text-gray-400 text-xs uppercase font-semibold">
        <tr>
          <th class="p-2 border-b border-slate-200 dark:border-gray-700">Name</th>
          <th class="p-2 border-b border-slate-200 dark:border-gray-700 w-24">Size</th>
          <th class="p-2 border-b border-slate-200 dark:border-gray-700 w-40">Modified</th>
          <th class="p-2 border-b border-slate-200 dark:border-gray-700 w-20">Perms</th>
        </tr>
      </thead>
      <tbody>
        {#each files as file}
          {@const iconType = getFileIcon(file.name, file.isDirectory)}
          <tr
            class="cursor-pointer border-b border-slate-100 dark:border-gray-800 transition-colors duration-75 {selectedFile === file ? 'bg-blue-100 dark:bg-blue-900/30' : 'hover:bg-slate-50 dark:hover:bg-gray-800'}"
            on:click|stopPropagation={() => selectedFile = file}
            on:dblclick={() => file.isDirectory && handleNavigate(file.path)}
            on:contextmenu|preventDefault|stopPropagation={(e) => handleContextMenu(e, file)}
            draggable={!file.isDirectory}
            on:dragstart={(e) => handleDragStart(e, file)}
          >
            <td class="p-2 flex items-center space-x-2">
              {@html getIconSvg(iconType)}
              <span class={file.isDirectory ? 'font-medium text-slate-900 dark:text-white' : 'text-slate-700 dark:text-gray-300'}>{file.name}</span>
            </td>
            <td class="p-2 text-slate-500 dark:text-gray-400 font-mono text-xs">{file.isDirectory ? '-' : formatSize(file.size)}</td>
            <td class="p-2 text-slate-500 dark:text-gray-400 text-xs">{formatDate(file.modified)}</td>
            <td class="p-2 text-slate-400 dark:text-gray-500 font-mono text-xs">{file.permissions}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if contextMenu.show}
    <div
      class="fixed bg-white dark:bg-gray-800 border border-slate-200 dark:border-gray-700 rounded shadow-lg py-1 z-50 text-sm min-w-[180px]"
      style="top: {contextMenu.y}px; left: {contextMenu.x}px"
      role="menu"
      tabindex="-1"
    >
      {#if contextMenu.file}
        {@const fileIcon = getFileIcon(contextMenu.file.name, contextMenu.file.isDirectory)}
        <div class="px-4 py-2 border-b border-slate-200 dark:border-gray-700 flex items-center space-x-2">
          {@html getIconSvg(fileIcon)}
          <span class="truncate font-medium text-slate-900 dark:text-white">{contextMenu.file.name}</span>
        </div>
        {#if !contextMenu.file.isDirectory}
          <button
            class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 flex items-center space-x-2"
            on:click|stopPropagation={handleDownload}
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4"/>
            </svg>
            <span>下载</span>
          </button>
          <button
            class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 flex items-center space-x-2"
            on:click|stopPropagation={handleCopy}
          >
            <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z"/>
            </svg>
            <span>复制</span>
          </button>
        {/if}
        <button
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 flex items-center space-x-2"
          on:click|stopPropagation={handleRename}
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z"/>
          </svg>
          <span>重命名</span>
        </button>
        <button
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-red-600 dark:text-red-400 hover:text-red-500 dark:hover:text-red-300 flex items-center space-x-2"
          on:click|stopPropagation={handleDelete}
        >
          <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/>
          </svg>
          <span>删除</span>
        </button>
        <div class="border-t border-slate-200 dark:border-gray-700 my-1"></div>
      {/if}
      <button
        class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 flex items-center space-x-2"
        on:click|stopPropagation={handleCreateFolder}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 13h6m-3-3v6m-9 1V7a2 2 0 012-2h6l2 2h6a2 2 0 012 2v8a2 2 0 01-2 2H5a2 2 0 01-2-2z"/>
        </svg>
        <span>新建文件夹</span>
      </button>
      <button
        class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 flex items-center space-x-2"
        on:click|stopPropagation={handleCreateFile}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z"/>
        </svg>
        <span>新建文件</span>
      </button>
      <button
        class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 flex items-center space-x-2 disabled:opacity-60"
        on:click|stopPropagation={handlePaste}
        disabled={!$fileClipboard}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5H7a2 2 0 00-2 2v12a2 2 0 002 2h10a2 2 0 002-2V7a2 2 0 00-2-2h-2M9 5a2 2 0 002 2h2a2 2 0 002-2M9 5a2 2 0 012-2h2a2 2 0 012 2"/>
        </svg>
        <span>粘贴</span>
      </button>
      <button
        class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 flex items-center space-x-2"
        on:click|stopPropagation={() => loadFiles(currentPath)}
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"/>
        </svg>
        <span>刷新</span>
      </button>
    </div>
  {/if}
</div>
