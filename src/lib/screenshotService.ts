import { invoke } from '@tauri-apps/api/core';

export interface CaptureInfo {
  width: number;
  height: number;
  scaleFactor: number;
}

export interface PinCreateArgs {
  dataUrl: string;
  x: number;
  y: number;
  width: number;
  height: number;
}

let starting = false;

/**
 * Trigger a full-screen capture and open the selection overlay window.
 * Guards against re-entrancy so a held-down shortcut doesn't spawn multiple overlays.
 */
export async function startScreenshot(): Promise<void> {
  if (starting) return;
  starting = true;
  try {
    await invoke<CaptureInfo>('screenshot_capture');
  } catch (err) {
    console.error('screenshot_capture failed', err);
  } finally {
    // Small delay so a repeated shortcut press doesn't immediately re-fire.
    setTimeout(() => {
      starting = false;
    }, 300);
  }
}

/** Overlay: fetch the frozen full-screen frame as a data URL. */
export function getCapture(): Promise<string> {
  return invoke<string>('screenshot_get_capture');
}

/** Overlay: cancel the capture and close the overlay window. */
export function cancelScreenshot(): Promise<void> {
  return invoke('screenshot_cancel');
}

/** Overlay: create a pinned window from the cropped selection. */
export function createPin(args: PinCreateArgs): Promise<string> {
  return invoke<string>('pin_create', { args });
}

/** Pin: fetch this pin's image as a data URL. */
export function getPinImage(id: string): Promise<string> {
  return invoke<string>('pin_get_image', { id });
}

/** Pin: copy this pin's image to the clipboard. */
export function copyPin(id: string): Promise<void> {
  return invoke('pin_copy', { id });
}

/** Pin: close this pin window and drop its image. */
export function closePin(id: string): Promise<void> {
  return invoke('pin_close', { id });
}
