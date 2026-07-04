<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { getCapture, cancelScreenshot, createPin } from '../../lib/screenshotService';

  // The frozen full-screen frame.
  let imageUrl = '';
  let img: HTMLImageElement | null = null;

  // Physical pixel width of the capture (natural size of the image).
  let naturalWidth = 0;

  // Logical (CSS) viewport width — the overlay covers the whole monitor.
  let viewW = 0;

  // Selection rectangle in CSS/logical pixels.
  let selecting = false;
  let hasSelection = false;
  let startX = 0;
  let startY = 0;
  let curX = 0;
  let curY = 0;

  $: selLeft = Math.min(startX, curX);
  $: selTop = Math.min(startY, curY);
  $: selW = Math.abs(curX - startX);
  $: selH = Math.abs(curY - startY);

  // Ratio physical/logical, used to crop from the natural-resolution image.
  $: pxRatio = viewW > 0 ? naturalWidth / viewW : 1;

  function updateViewport() {
    viewW = window.innerWidth;
  }

  onMount(async () => {
    updateViewport();
    window.addEventListener('resize', updateViewport);
    try {
      imageUrl = await getCapture();
    } catch (err) {
      console.error('failed to load capture', err);
      void cancel();
    }
  });

  onDestroy(() => {
    window.removeEventListener('resize', updateViewport);
  });

  function onImageLoad() {
    if (img) {
      naturalWidth = img.naturalWidth;
    }
  }

  function handleMouseDown(e: MouseEvent) {
    if (e.button !== 0) return;
    selecting = true;
    hasSelection = false;
    startX = e.clientX;
    startY = e.clientY;
    curX = e.clientX;
    curY = e.clientY;
  }

  function handleMouseMove(e: MouseEvent) {
    if (!selecting) return;
    curX = e.clientX;
    curY = e.clientY;
  }

  function handleMouseUp() {
    if (!selecting) return;
    selecting = false;
    if (selW >= 4 && selH >= 4) {
      hasSelection = true;
      void confirm();
    } else {
      hasSelection = false;
    }
  }

  async function cancel() {
    try {
      await cancelScreenshot();
    } catch (err) {
      console.error('cancel failed', err);
      try {
        await getCurrentWindow().close();
      } catch {
        /* noop */
      }
    }
  }

  async function confirm() {
    if (selW < 4 || selH < 4 || !naturalWidth) {
      void cancel();
      return;
    }

    // Crop from the physical-resolution image for a sharp result.
    const cropX = Math.round(selLeft * pxRatio);
    const cropY = Math.round(selTop * pxRatio);
    const cropW = Math.round(selW * pxRatio);
    const cropH = Math.round(selH * pxRatio);

    const canvas = document.createElement('canvas');
    canvas.width = cropW;
    canvas.height = cropH;
    const ctx = canvas.getContext('2d');
    if (!ctx || !img) {
      void cancel();
      return;
    }
    ctx.drawImage(img, cropX, cropY, cropW, cropH, 0, 0, cropW, cropH);
    const dataUrl = canvas.toDataURL('image/png');

    try {
      await createPin({
        dataUrl,
        x: selLeft,
        y: selTop,
        width: selW,
        height: selH,
      });
    } catch (err) {
      console.error('createPin failed', err);
      void cancel();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      void cancel();
    } else if (e.key === 'Enter' && hasSelection) {
      e.preventDefault();
      void confirm();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<!-- svelte-ignore a11y-no-static-element-interactions -->
<div
  class="overlay"
  on:mousedown={handleMouseDown}
  on:mousemove={handleMouseMove}
  on:mouseup={handleMouseUp}
>
  {#if imageUrl}
    <img
      bind:this={img}
      src={imageUrl}
      alt="screen capture"
      class="frozen"
      draggable="false"
      on:load={onImageLoad}
    />
  {/if}

  <!-- Dimmed mask; the selection punches a clear hole via box-shadow. -->
  {#if selecting || hasSelection}
    <div
      class="selection"
      style="left:{selLeft}px; top:{selTop}px; width:{selW}px; height:{selH}px;"
    >
      <div class="size-label">{Math.round(selW * pxRatio)} × {Math.round(selH * pxRatio)}</div>
    </div>
  {:else}
    <div class="full-mask"></div>
    <div class="hint">拖动鼠标框选截图区域 · Esc 取消</div>
  {/if}
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    overflow: hidden;
    cursor: crosshair;
    user-select: none;
  }

  .frozen {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: fill;
    pointer-events: none;
  }

  .full-mask {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    pointer-events: none;
  }

  .selection {
    position: absolute;
    border: 1px solid #38bdf8;
    /* The huge spread shadow dims everything outside the selection. */
    box-shadow: 0 0 0 100000px rgba(0, 0, 0, 0.35);
    pointer-events: none;
  }

  .size-label {
    position: absolute;
    top: -22px;
    left: 0;
    padding: 1px 6px;
    font-size: 12px;
    line-height: 18px;
    color: #fff;
    background: rgba(15, 23, 42, 0.85);
    border-radius: 4px;
    white-space: nowrap;
  }

  .hint {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    padding: 8px 16px;
    font-size: 14px;
    color: #e2e8f0;
    background: rgba(15, 23, 42, 0.7);
    border-radius: 8px;
    pointer-events: none;
  }
</style>
