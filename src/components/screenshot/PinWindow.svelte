<script lang="ts">
  import { onMount } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { getPinImage, copyPin, closePin } from '../../lib/screenshotService';

  // The pin id equals this window's label (e.g. "pin-3").
  const label = getCurrentWindow().label;

  let imageUrl = '';
  let copied = false;
  let copyTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(async () => {
    try {
      imageUrl = await getPinImage(label);
    } catch (err) {
      console.error('failed to load pin image', err);
      void close();
    }
  });

  async function close() {
    try {
      await closePin(label);
    } catch (err) {
      console.error('closePin failed', err);
      try {
        await getCurrentWindow().close();
      } catch {
        /* noop */
      }
    }
  }

  async function copy() {
    try {
      await copyPin(label);
      copied = true;
      if (copyTimer) clearTimeout(copyTimer);
      copyTimer = setTimeout(() => {
        copied = false;
      }, 1200);
    } catch (err) {
      console.error('copyPin failed', err);
    }
  }

  // Drag the whole pin by pressing on the image.
  function handleMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    void getCurrentWindow().startDragging();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      void close();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'c') {
      e.preventDefault();
      void copy();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div class="pin" on:mousedown={handleMouseDown} on:contextmenu|preventDefault={copy}>
  {#if imageUrl}
    <img src={imageUrl} alt="pinned screenshot" draggable="false" />
  {/if}

  <!-- Toolbar shown on hover. -->
  <div class="toolbar">
    <button class="tool-btn" on:mousedown|stopPropagation on:click|stopPropagation={copy} title="复制到剪贴板">
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="icon">
        <path d="M7 3a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2V7.414A2 2 0 0 0 14.414 6L11 2.586A2 2 0 0 0 9.586 2H7a2 2 0 0 0-2 1z" />
        <path d="M3 7a2 2 0 0 1 2-2v10h8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
      </svg>
    </button>
    <button class="tool-btn close" on:mousedown|stopPropagation on:click|stopPropagation={close} title="关闭 (Esc)">
      <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="icon">
        <path fill-rule="evenodd" d="M4.293 4.293a1 1 0 0 1 1.414 0L10 8.586l4.293-4.293a1 1 0 1 1 1.414 1.414L11.414 10l4.293 4.293a1 1 0 0 1-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 0 1-1.414-1.414L8.586 10 4.293 5.707a1 1 0 0 1 0-1.414z" clip-rule="evenodd" />
      </svg>
    </button>
  </div>

  {#if copied}
    <div class="copied-toast">已复制</div>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    margin: 0;
    padding: 0;
    background: transparent !important;
    overflow: hidden;
  }

  .pin {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    cursor: move;
    user-select: none;
    border: 1px solid rgba(56, 189, 248, 0.7);
    box-sizing: border-box;
    overflow: hidden;
    background: transparent;
  }

  .pin img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: fill;
    pointer-events: none;
  }

  .toolbar {
    position: absolute;
    top: 4px;
    right: 4px;
    display: flex;
    gap: 4px;
    opacity: 0;
    transition: opacity 0.12s ease;
  }

  .pin:hover .toolbar {
    opacity: 1;
  }

  .tool-btn {
    width: 22px;
    height: 22px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: none;
    border-radius: 4px;
    background: rgba(15, 23, 42, 0.75);
    color: #e2e8f0;
    cursor: pointer;
    padding: 0;
  }

  .tool-btn:hover {
    background: rgba(30, 41, 59, 0.95);
  }

  .tool-btn.close:hover {
    background: #ef4444;
    color: #fff;
  }

  .icon {
    width: 14px;
    height: 14px;
  }

  .copied-toast {
    position: absolute;
    bottom: 6px;
    left: 50%;
    transform: translateX(-50%);
    padding: 2px 10px;
    font-size: 12px;
    color: #fff;
    background: rgba(15, 23, 42, 0.85);
    border-radius: 6px;
    pointer-events: none;
  }
</style>
