<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';
  
  const appWindow = getCurrentWindow();
  
  function handleMinimize() {
    console.log('Minimize button clicked');
    appWindow.minimize();
  }
  
  function handleMaximize() {
    console.log('Maximize button clicked');
    appWindow.toggleMaximize();
  }
  
  function handleClose() {
    console.log('Close button clicked');
    appWindow.close();
  }
</script>

<div class="titlebar">
  <div class="titlebar-app-section">
    <div class="titlebar-icon">
      <svg class="w-5 h-5" viewBox="0 0 1024 1024" xmlns="http://www.w3.org/2000/svg">
        <path d="M981.5 215.2c-0.1-55.3-30.9-85.3-86.4-85.4-255.6-0.1-511.3-0.1-766.9 0-55.1 0-85.6 30.4-85.7 86.1-0.3 197.2-0.2 394.3 0 591.5 0.1 57 30.5 86.6 87.8 86.7 126.5 0.1 253 0 379.5 0 128.3 0 256.5 0.1 384.8 0 55.9 0 86.9-29.7 87-84.8 0.2-198 0.2-396-0.1-594.1z m-65.4 586.9c0.1 21.3-6.6 26.6-26.9 26.5-125.7-0.8-251.5-0.4-377.2-0.4-124.9 0-249.7-0.5-374.6 0.5-22.7 0.2-29.7-5.7-29.6-29.2 0.9-192.1 0.9-384.2 0-576.2-0.1-22.3 6.9-27.8 28.2-27.8 250.6 0.6 501.2 0.6 751.7 0 21.3-0.1 28.3 5.5 28.2 27.8-0.7 192.9-0.6 385.8 0.2 578.8z" fill="currentColor"></path>
        <path d="M249.5 349.1c11.3-0.3 20 5.5 28 12.4 46.9 40.2 94.1 80.2 140.5 121 23.3 20.4 23.3 38.8 0.2 59.1-47.2 41.3-94.8 82-142.7 122.5-18.5 15.6-37.9 15-50.2-1.3-13.8-18.2-7.9-34.4 7.8-48 34.9-30.2 69.6-60.6 105.2-89.9 11.7-9.6 12.7-15 0.4-25.1-36.3-29.9-71.5-61-107.2-91.6-12.7-10.9-17.2-24.3-10.6-39.7 5.1-12.2 15.1-18.9 28.6-19.4zM635.1 653.7c43.6 0 87.3-0.2 130.9 0.1 26.5 0.2 41.3 12.7 40 33.9-1.5 24.4-17.8 32-39.5 32-89 0-178 0.2-267-0.1-27-0.1-41.4-12-41.5-32.8-0.1-21 14.3-32.9 41.1-33 45.3-0.3 90.6-0.1 136-0.1z" fill="currentColor"></path>
      </svg>
    </div>
    <div class="titlebar-app-name">Star Shuttle</div>
  </div>
  
  <div class="titlebar-drag-region" data-tauri-drag-region></div>
  
  <div class="titlebar-controls">
    <button on:click={handleMinimize} title="最小化" class="minimize-btn">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path fill="currentColor" d="M19 13H5v-2h14z" />
      </svg>
    </button>
    <button on:click={handleMaximize} title="最大化/还原" class="maximize-btn">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path fill="currentColor" d="M4 4h16v16H4zm2 4v10h12V8z" />
      </svg>
    </button>
    <button on:click={handleClose} title="关闭" class="close-btn">
      <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path fill="currentColor" d="M13.46 12L19 17.54V19h-1.46L12 13.46L6.46 19H5v-1.46L10.54 12L5 6.46V5h1.46L12 10.54L17.54 5H19v1.46z" />
      </svg>
    </button>
  </div>
</div>

<style>
  .titlebar {
    height: 30px;
    background: var(--color-surface);
    user-select: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    z-index: 1000;
    border-bottom: 1px solid var(--color-border);
    color: var(--color-text-secondary);
    -webkit-app-region: drag;
    pointer-events: none;
  }
  
  .titlebar-app-section {
    display: flex;
    align-items: center;
    gap: 8px;
    padding-left: 8px;
    height: 100%;
    -webkit-app-region: drag;
    pointer-events: auto;
  }
  
  .titlebar-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-secondary);
  }
  
  .titlebar-app-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--color-text);
    white-space: nowrap;
  }
  
  .titlebar-drag-region {
    flex: 1;
    height: 100%;
    -webkit-app-region: drag;
    cursor: move;
    pointer-events: auto;
  }
  
  .titlebar-controls {
    display: flex;
    height: 100%;
    -webkit-app-region: no-drag;
  }
  
  .titlebar-controls button {
    appearance: none;
    padding: 0;
    margin: 0;
    border: none;
    display: inline-flex;
    justify-content: center;
    align-items: center;
    width: 30px;
    background-color: transparent;
    color: var(--color-text-secondary);
    transition: all 0.15s ease;
    cursor: pointer;
    pointer-events: auto;
  }
  
  .titlebar-controls button:hover {
    background: var(--color-surface-light);
    color: var(--color-text);
  }
  
  .close-btn:hover {
    background: #ef4444 !important;
    color: white !important;
  }
</style>
