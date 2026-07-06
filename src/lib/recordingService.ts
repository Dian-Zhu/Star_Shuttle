import { invoke } from '@tauri-apps/api/core';
import { writable } from 'svelte/store';

/** Whether a screen recording is currently in progress. */
export const isRecording = writable(false);

export interface RecordingStopResult {
  savedPath: string | null;
}

let busy = false;

/**
 * Start recording the main window. Guards against re-entrancy so a held-down
 * shortcut or double-click doesn't spawn overlapping recordings.
 */
export async function startRecording(): Promise<void> {
  if (busy) return;
  busy = true;
  try {
    await invoke('recording_start');
    isRecording.set(true);
  } catch (err) {
    console.error('recording_start failed', err);
    isRecording.set(false);
    throw err;
  } finally {
    busy = false;
  }
}

/**
 * Stop the active recording and open a save dialog. Resolves with the saved
 * path, or null if the user cancelled the save.
 */
export async function stopRecording(): Promise<string | null> {
  if (busy) return null;
  busy = true;
  try {
    const result = await invoke<RecordingStopResult>('recording_stop');
    return result.savedPath;
  } catch (err) {
    console.error('recording_stop failed', err);
    throw err;
  } finally {
    isRecording.set(false);
    busy = false;
  }
}

/** Toggle recording: start if idle, stop (and save) if active. */
export async function toggleRecording(): Promise<void> {
  let active = false;
  const unsub = isRecording.subscribe((v) => (active = v));
  unsub();
  if (active) {
    await stopRecording();
  } else {
    await startRecording();
  }
}

/** Query the backend for the current recording state (e.g. on window focus). */
export async function refreshRecordingState(): Promise<void> {
  try {
    const active = await invoke<boolean>('recording_is_active');
    isRecording.set(active);
  } catch (err) {
    console.error('recording_is_active failed', err);
  }
}
