<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';

  // Route by window label:
  //   "main"                -> the full application (Layout)
  //   "screenshot-overlay"  -> the region-selection overlay
  //   "pin-*"               -> a pinned screenshot window
  // Overlay/pin windows lazy-load their tiny components so they never pull in
  // the terminal/xterm bundle that the main window needs.
  const label = getCurrentWindow().label;
  const isOverlay = label === 'screenshot-overlay';
  const isPin = label.startsWith('pin-');
  const isMain = !isOverlay && !isPin;
</script>

{#if isOverlay}
  {#await import('./components/screenshot/ScreenshotOverlay.svelte') then module}
    <svelte:component this={module.default} />
  {/await}
{:else if isPin}
  {#await import('./components/screenshot/PinWindow.svelte') then module}
    <svelte:component this={module.default} />
  {/await}
{:else if isMain}
  {#await import('./MainApp.svelte') then module}
    <svelte:component this={module.default} />
  {/await}
{/if}
