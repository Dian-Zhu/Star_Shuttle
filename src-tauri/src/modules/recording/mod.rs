// Screen recording feature.
//
// Records the application's own main window to an H.264/MP4 file using
// Windows Graphics Capture (WGC) for frame acquisition and Media Foundation
// for hardware-accelerated encoding. No external binary (ffmpeg) is bundled;
// everything runs through OS-native APIs already reachable via the `windows`
// crate (which xcap also depends on).
//
// Flow:
//   1. `recording_start` grabs the main window's HWND, spawns a dedicated
//      capture+encode thread, and returns immediately. Recording runs in the
//      background writing frames to a temporary .mp4.
//   2. `recording_is_active` lets the UI reflect the recording state.
//   3. `recording_stop` signals the worker to finalize the MP4, waits for it,
//      then opens a save dialog. On confirm the temp file is moved to the
//      chosen path; on cancel it is deleted. Returns the saved path (or None).
//
// Only Windows is supported. Other platforms return a descriptive error so the
// command surface stays uniform.

use crate::{ensure_app_unlocked_runtime, AppLockRuntimeState};
use crate::modules::db::DatabaseManager;
use std::sync::{Arc, Mutex};
use tauri::{command, AppHandle, State};

#[cfg(target_os = "windows")]
mod win;

/// Runtime state for an in-flight recording. Holds only Send+Sync primitives so
/// it can live in Tauri's managed state; the platform-specific handles are kept
/// inside the worker thread, not here.
#[derive(Default)]
pub struct RecordingState {
    #[cfg(target_os = "windows")]
    inner: Mutex<Option<win::RecordingSession>>,
    // Keep the type non-empty / field used on non-Windows builds.
    #[cfg(not(target_os = "windows"))]
    _unsupported: Mutex<()>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStopResult {
    /// Absolute path the recording was saved to, or null if the user cancelled.
    pub saved_path: Option<String>,
}

/// Start recording the main window. Errors if a recording is already active.
#[command]
pub async fn recording_start(
    app: AppHandle,
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, RecordingState>,
) -> Result<(), String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;

    #[cfg(target_os = "windows")]
    {
        let mut guard = state.inner.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Err("录制已在进行中".to_string());
        }
        let session = win::start_recording(&app)?;
        *guard = Some(session);
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, state);
        Err("录屏功能目前仅支持 Windows".to_string())
    }
}

/// Whether a recording is currently active.
#[command]
pub async fn recording_is_active(
    state: State<'_, RecordingState>,
) -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        let guard = state.inner.lock().map_err(|e| e.to_string())?;
        Ok(guard.is_some())
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = state;
        Ok(false)
    }
}

/// Stop the active recording, finalize the MP4, and prompt for a save location.
#[command]
pub async fn recording_stop(
    app: AppHandle,
    state: State<'_, RecordingState>,
) -> Result<RecordingStopResult, String> {
    #[cfg(target_os = "windows")]
    {
        let session = {
            let mut guard = state.inner.lock().map_err(|e| e.to_string())?;
            guard.take()
        };
        let session = session.ok_or_else(|| "当前没有正在进行的录制".to_string())?;
        let temp_path = win::finish_recording(session)?;
        let saved_path = win::prompt_save_and_move(&app, temp_path)?;
        Ok(RecordingStopResult { saved_path })
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, state);
        Err("录屏功能目前仅支持 Windows".to_string())
    }
}
