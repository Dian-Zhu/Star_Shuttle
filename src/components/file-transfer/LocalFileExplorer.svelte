<script lang="ts">
  import { onMount } from 'svelte';
  import { localFsService } from '../../lib/localFsService';
  import { sftpService } from '../../lib/sftpService';
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

  function closeContextMenu() {
    contextMenu.show = false;
  }

  async function handleCreateFolder() {
    closeContextMenu();
    const name = prompt('Enter folder name:');
    if (!name) return;
    
    const path = joinPath(currentPath, name);
    try {
      await localFsService.createDirectory(path);
      loadFiles(currentPath);
    } catch (e: any) {
      error = e.toString();
    }
  }

  async function handleDelete() {
    closeContextMenu();
    if (!selectedFile) return;
    
    if (!confirm(`Are you sure you want to delete ${selectedFile.name}?`)) return;

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

    const newName = prompt('Enter new name:', selectedFile.name);
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

  onMount(() => {
    loadFiles(currentPath);
    window.addEventListener('click', closeContextMenu);
    return () => window.removeEventListener('click', closeContextMenu);
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
  class="flex flex-col h-full bg-gray-900 text-white relative {isDragging ? 'border-2 border-blue-500 bg-gray-800' : ''}" 
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
      <div class="text-2xl font-bold text-blue-200">Drop files to upload</div>
    </div>
  {/if}
  {#if isCrossDragging}
    <div class="absolute inset-0 bg-blue-500/10 flex items-center justify-center z-40 pointer-events-none">
      <div class="text-lg font-semibold text-blue-200">拖拽到此处下载到本地</div>
    </div>
  {/if}
  <!-- Toolbar -->
  <div class="flex items-center p-2 border-b border-gray-700 space-x-2">
    <button 
        class="p-1 hover:bg-gray-700 rounded text-gray-300" 
        on:click={handleUp} 
        title="Up"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M14.707 12.707a1 1 0 01-1.414 0L10 9.414l-3.293 3.293a1 1 0 01-1.414-1.414l4-4a1 1 0 011.414 0l4 4a1 1 0 010 1.414z" clip-rule="evenodd" />
      </svg>
    </button>
    <button 
        class="p-1 hover:bg-gray-700 rounded text-gray-300" 
        on:click={() => loadFiles(currentPath)} 
        title="Refresh"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v3.276a1 1 0 01-2 0V14.907a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z" clip-rule="evenodd" />
      </svg>
    </button>
    <button 
        class="p-1 hover:bg-gray-700 rounded text-gray-300" 
        on:click={handleCreateFolder} 
        title="New Folder"
    >
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
        <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1H8a3 3 0 00-3 3v1.5a1.5 1.5 0 01-3 0V6z" clip-rule="evenodd" />
        <path d="M6 12a2 2 0 012-2h8a2 2 0 012 2v2a2 2 0 01-2 2H2h2a2 2 0 002-2v-2z" />
      </svg>
    </button>
    <button 
        class="p-1 hover:bg-gray-700 rounded text-gray-300" 
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
      class="flex-1 bg-gray-800 border border-gray-600 rounded px-2 py-1 text-sm text-gray-200"
      value={currentPath}
      on:change={(e) => loadFiles(e.currentTarget.value)}
    />
  </div>

  <!-- File List -->
  <div class="flex-1 overflow-auto" role="grid">
    {#if loading}
      <div class="absolute inset-0 top-10 bg-gray-900/50 flex items-center justify-center z-10">
        <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-white"></div>
      </div>
    {/if}

    {#if error}
      <div class="p-4 text-red-400 bg-red-900/20 m-2 rounded">
        Error: {error}
      </div>
    {/if}

    <table class="w-full text-sm text-left border-collapse">
      <thead class="bg-gray-800 sticky top-0 text-gray-400 text-xs uppercase font-semibold">
        <tr>
          <th class="p-2 border-b border-gray-700">Name</th>
          <th class="p-2 border-b border-gray-700 w-24">Size</th>
          <th class="p-2 border-b border-gray-700 w-40">Modified</th>
          <th class="p-2 border-b border-gray-700 w-20">Perms</th>
        </tr>
      </thead>
      <tbody>
        {#each files as file}
          <tr 
            class="cursor-pointer border-b border-gray-800 transition-colors duration-75 {selectedFile === file ? 'bg-blue-900/30' : 'hover:bg-gray-800'}"
            on:click|stopPropagation={() => selectedFile = file}
            on:dblclick={() => file.isDirectory && handleNavigate(file.path)}
            on:contextmenu|preventDefault|stopPropagation={(e) => handleContextMenu(e, file)}
            draggable={!file.isDirectory}
            on:dragstart={(e) => handleDragStart(e, file)}
          >
            <td class="p-2 flex items-center space-x-2">
              <span class="text-yellow-500">{file.isDirectory ? '📁' : '📄'}</span>
              <span class={file.isDirectory ? 'font-medium text-white' : 'text-gray-300'}>{file.name}</span>
            </td>
            <td class="p-2 text-gray-400 font-mono text-xs">{file.isDirectory ? '-' : formatSize(file.size)}</td>
            <td class="p-2 text-gray-400 text-xs">{formatDate(file.modified)}</td>
            <td class="p-2 text-gray-500 font-mono text-xs">{file.permissions}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if contextMenu.show}
    <div 
      class="fixed bg-gray-800 border border-gray-700 rounded shadow-lg py-1 z-50 text-sm min-w-[150px]"
      style="top: {contextMenu.y}px; left: {contextMenu.x}px"
      role="menu"
      tabindex="-1"
    >
      {#if contextMenu.file}
        {#if !contextMenu.file.isDirectory}
          <button 
            class="w-full text-left px-4 py-2 hover:bg-gray-700 text-gray-200"
            on:click|stopPropagation={handleDownload}
          >
            Download
          </button>
        {/if}
        <button 
          class="w-full text-left px-4 py-2 hover:bg-gray-700 text-gray-200"
          on:click|stopPropagation={handleRename}
        >
          Rename
        </button>
        <button 
          class="w-full text-left px-4 py-2 hover:bg-gray-700 text-red-400 hover:text-red-300"
          on:click|stopPropagation={handleDelete}
        >
          Delete
        </button>
        <div class="border-t border-gray-700 my-1"></div>
      {/if}
      <button 
        class="w-full text-left px-4 py-2 hover:bg-gray-700 text-gray-200"
        on:click|stopPropagation={handleCreateFolder}
      >
        New Folder
      </button>
      <button 
        class="w-full text-left px-4 py-2 hover:bg-gray-700 text-gray-200"
        on:click|stopPropagation={() => loadFiles(currentPath)}
      >
        Refresh
      </button>
    </div>
  {/if}
</div>
