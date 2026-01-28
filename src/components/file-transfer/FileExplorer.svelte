<script lang="ts">
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { sftpService } from '../../lib/sftpService';
  import { localFsService } from '../../lib/localFsService';
  import { fileClipboard } from '../../lib/store';
  import type { FileEntry } from '../../types';
  
  export let sessionId: string;
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

  function getMenuTargetDirectory() {
    if (contextMenu.file?.isDirectory) return contextMenu.file.path;
    return currentPath;
  }

  async function loadFiles(path: string) {
    console.log('[FileExplorer] loadFiles called', { sessionId, path });
    loading = true;
    error = null;
    selectedFile = null;
    contextMenu.show = false;
    try {
      console.log('[FileExplorer] Calling sftpService.listDirectory...');
      files = await sftpService.listDirectory(sessionId, path);
      console.log('[FileExplorer] listDirectory success', files.length);
      currentPath = path;
      
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
    if (currentPath === '/' || currentPath === '') return;
    if (currentPath === '.') {
        loadFiles('..');
        return;
    }
    const parts = currentPath.split('/').filter(p => p);
    if (parts.length > 0) {
        parts.pop();
        const parent = parts.length === 0 ? '/' : parts.join('/');
        loadFiles(parent === '' ? '/' : parent); 
    } else {
        loadFiles('..');
    }
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
      try {
        await sftpService.writeFile(sessionId, path, new Uint8Array(0), false);
      } catch (e) {
        await sftpService.scpUpload(sessionId, path, new Uint8Array(0));
      }
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
        await sftpService.removeDirectory(sessionId, selectedFile.path);
      } else {
        await sftpService.removeFile(sessionId, selectedFile.path);
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
    const parts = selectedFile.path.split('/');
    parts.pop(); // Remove old filename
    const parentPath = parts.join('/');
    const newPath = parentPath === '' ? `/${newName}` : `${parentPath}/${newName}`;

    try {
      await sftpService.rename(sessionId, selectedFile.path, newPath);
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
    if (!selectedFile || selectedFile.isDirectory) return;
    
    loading = true;
    try {
      let content: Uint8Array;
      try {
        content = await sftpService.readFile(sessionId, selectedFile.path);
      } catch (e) {
        content = await sftpService.scpDownload(sessionId, selectedFile.path);
      }
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
      source: 'remote',
      sessionId,
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
        try {
          await sftpService.writeFile(sessionId, destPath, content, false);
        } catch (e) {
          await sftpService.scpUpload(sessionId, destPath, content);
        }
      } else {
        if (!item.sessionId) return;
        let content: Uint8Array;
        try {
          content = await sftpService.readFile(item.sessionId, item.path);
        } catch (e) {
          content = await sftpService.scpDownload(item.sessionId, item.path);
        }
        try {
          await sftpService.writeFile(sessionId, destPath, content, false);
        } catch (e) {
          await sftpService.scpUpload(sessionId, destPath, content);
        }
      }

      await loadFiles(currentPath);
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
    const isModifier = e.ctrlKey || e.metaKey;
    if (!isModifier) return;
    const key = String(e.key).toLowerCase();
    if (key === 'c') {
      e.preventDefault();
      if (selectedFile) {
        fileClipboard.set({
          source: 'remote',
          sessionId,
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
      await loadFiles(currentPath);
      closeEditor();
    } catch (e: any) {
      editorError = e?.message ?? String(e);
    } finally {
      editorSaving = false;
    }
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
  {#if editorOpen}
    <div class="fixed inset-0 z-50 flex items-center justify-center" role="presentation">
      <button type="button" class="absolute inset-0 bg-slate-900/60 dark:bg-black/60" on:click={closeEditor} aria-label="关闭编辑器"></button>
      <div class="relative w-[min(900px,95vw)] h-[min(700px,90vh)] bg-white dark:bg-gray-900 border border-slate-200 dark:border-gray-700 rounded-lg shadow-xl flex flex-col" role="dialog" aria-modal="true">
        <div class="flex items-center justify-between px-4 py-2 border-b border-slate-200 dark:border-gray-700 gap-3">
          <div class="text-sm text-slate-700 dark:text-gray-200 truncate flex-1">{editorFile?.path}</div>
          <div class="flex items-center gap-2 flex-none">
            <button class="px-3 py-1 rounded bg-slate-100 dark:bg-gray-800 hover:bg-slate-200 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 disabled:opacity-60" on:click={closeEditor} disabled={editorSaving}>
              关闭
            </button>
            <button class="px-3 py-1 rounded bg-blue-600 hover:bg-blue-500 text-white disabled:opacity-60" on:click={saveEditor} disabled={editorSaving || editorLoading || !editorFile}>
              {editorSaving ? '保存中…' : '保存'}
            </button>
          </div>
        </div>
        {#if editorLoading}
          <div class="flex-1 flex items-center justify-center text-slate-500 dark:text-gray-300">加载中…</div>
        {:else}
          <textarea class="flex-1 w-full bg-slate-50 dark:bg-gray-950 text-slate-900 dark:text-gray-100 font-mono text-sm p-3 outline-none resize-none" bind:value={editorContent} disabled={editorSaving}></textarea>
        {/if}
        {#if editorError}
          <div class="px-4 py-2 border-t border-slate-200 dark:border-gray-700 text-red-600 dark:text-red-400 text-sm">{editorError}</div>
        {/if}
      </div>
    </div>
  {/if}
  {#if isDragging}
    <div class="absolute inset-0 bg-blue-500/20 flex items-center justify-center z-50 pointer-events-none">
      <div class="text-2xl font-bold text-blue-600 dark:text-blue-200">Drop files to upload</div>
    </div>
  {/if}
  {#if isCrossDragging}
    <div class="absolute inset-0 bg-blue-500/10 flex items-center justify-center z-40 pointer-events-none">
      <div class="text-lg font-semibold text-blue-600 dark:text-blue-200">拖拽到此处上传到远程</div>
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
          <tr 
            class="cursor-pointer border-b border-slate-100 dark:border-gray-800 transition-colors duration-75 {selectedFile === file ? 'bg-blue-100 dark:bg-blue-900/30' : 'hover:bg-slate-50 dark:hover:bg-gray-800'}"
            on:click|stopPropagation={() => selectedFile = file}
            on:dblclick={() => openEditor(file)}
            on:contextmenu|preventDefault|stopPropagation={(e) => handleContextMenu(e, file)}
            draggable={!file.isDirectory}
            on:dragstart={(e) => handleDragStart(e, file)}
          >
            <td class="p-2 flex items-center space-x-2">
              <span class="text-yellow-500">{file.isDirectory ? '📁' : '📄'}</span>
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
      class="fixed bg-white dark:bg-gray-800 border border-slate-200 dark:border-gray-700 rounded shadow-lg py-1 z-50 text-sm min-w-[150px]"
      style="top: {contextMenu.y}px; left: {contextMenu.x}px"
      role="menu"
      tabindex="-1"
    >
      {#if contextMenu.file}
        {#if !contextMenu.file.isDirectory}
          <button 
            class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
            on:click|stopPropagation={handleDownload}
          >
            下载
          </button>
          <button 
            class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
            on:click|stopPropagation={handleCopy}
          >
            复制
          </button>
        {/if}
        <button 
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
          on:click|stopPropagation={handleRename}
        >
          重命名
        </button>
        <button 
          class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-red-600 dark:text-red-400 hover:text-red-500 dark:hover:text-red-300"
          on:click|stopPropagation={handleDelete}
        >
          删除
        </button>
        <div class="border-t border-slate-200 dark:border-gray-700 my-1"></div>
      {/if}
      <button 
        class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
        on:click|stopPropagation={handleCreateFolder}
      >
        新建文件夹
      </button>
      <button 
        class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
        on:click|stopPropagation={handleCreateFile}
      >
        新建文件
      </button>
      <button 
        class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200 disabled:opacity-60"
        on:click|stopPropagation={handlePaste}
        disabled={!$fileClipboard}
      >
        粘贴
      </button>
      <button 
        class="w-full text-left px-4 py-2 hover:bg-slate-100 dark:hover:bg-gray-700 text-slate-700 dark:text-gray-200"
        on:click|stopPropagation={() => loadFiles(currentPath)}
      >
        刷新
      </button>
    </div>
  {/if}
</div>
