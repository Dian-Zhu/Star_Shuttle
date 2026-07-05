// Screenshot + pin-to-screen feature.
//
// Flow:
//   1. `screenshot_capture` grabs the primary monitor into an in-memory PNG and
//      opens a fullscreen, always-on-top, borderless overlay window that shows
//      the frozen frame for region selection.
//   2. The overlay fetches the frozen frame via `screenshot_get_capture` and lets
//      the user draw a selection rectangle.
//   3. On confirm the overlay sends the cropped PNG (data URL) + physical geometry
//      to `pin_create`, which stores the image and opens a borderless, transparent,
//      always-on-top pin window positioned exactly over the selected region.
//   4. Pin windows fetch their image via `pin_get_image`, can copy it to the
//      clipboard via `pin_copy`, and close themselves via `pin_close`.
//
// Large image payloads are kept in Rust state and pulled by id; they are never
// broadcast through events.

use crate::modules::db::DatabaseManager;
use crate::{ensure_app_unlocked_runtime, AppLockRuntimeState};
use base64::Engine;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{command, AppHandle, LogicalPosition, LogicalSize, Manager, State, WebviewUrl};
use tauri_plugin_clipboard_manager::ClipboardExt;

const OVERLAY_LABEL: &str = "screenshot-overlay";
const PIN_LABEL_PREFIX: &str = "pin-";

/// Runtime state holding pending capture + live pin images, keyed by id.
#[derive(Default)]
pub struct ScreenshotState {
    /// The most recent full-screen capture (PNG bytes) awaiting selection.
    capture: Mutex<Option<Vec<u8>>>,
    /// Cropped pin images keyed by pin id.
    pins: Mutex<HashMap<String, Vec<u8>>>,
}

static PIN_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureInfo {
    /// Physical pixel width of the captured monitor.
    pub width: u32,
    /// Physical pixel height of the captured monitor.
    pub height: u32,
    /// DPI scale factor (physical / logical).
    pub scale_factor: f64,
}

fn encode_png(image: &image::RgbaImage) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(std::io::Cursor::new(&mut buf));
    image
        .write_with_encoder(encoder)
        .map_err(|e| format!("PNG encode failed: {e}"))?;
    Ok(buf)
}

/// Capture the primary monitor and open the selection overlay window.
#[command]
pub async fn screenshot_capture(
    app: AppHandle,
    db: State<'_, Arc<Mutex<DatabaseManager>>>,
    app_lock_state: State<'_, Arc<Mutex<AppLockRuntimeState>>>,
    state: State<'_, ScreenshotState>,
) -> Result<CaptureInfo, String> {
    ensure_app_unlocked_runtime(db.inner(), app_lock_state.inner())?;
    use xcap::Monitor;

    let monitors = Monitor::all().map_err(|e| format!("enumerate monitors failed: {e}"))?;
    // Prefer the primary monitor; fall back to the first one.
    let monitor = monitors
        .iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| monitors.first())
        .ok_or_else(|| "no monitor found".to_string())?;

    let image = monitor
        .capture_image()
        .map_err(|e| format!("capture failed: {e}"))?;
    let width = image.width();
    let height = image.height();
    let scale_factor = monitor.scale_factor().unwrap_or(1.0) as f64;

    let png = encode_png(&image)?;

    {
        let mut guard = state.capture.lock().map_err(|e| e.to_string())?;
        *guard = Some(png);
    }

    // Logical size of the monitor for positioning the overlay.
    let logical_w = width as f64 / scale_factor;
    let logical_h = height as f64 / scale_factor;

    // Reuse an existing overlay if present, otherwise create it.
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.show();
        let _ = win.set_focus();
    } else {
        let win = tauri::WebviewWindowBuilder::new(
            &app,
            OVERLAY_LABEL,
            WebviewUrl::App("index.html".into()),
        )
        .title("Screenshot")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .position(0.0, 0.0)
        .inner_size(logical_w, logical_h)
        .build()
        .map_err(|e| format!("overlay window failed: {e}"))?;
        let _ = win.set_focus();
    }

    Ok(CaptureInfo {
        width,
        height,
        scale_factor,
    })
}

/// Overlay fetches the frozen frame as a base64 PNG data URL.
#[command]
pub async fn screenshot_get_capture(
    state: State<'_, ScreenshotState>,
) -> Result<String, String> {
    let guard = state.capture.lock().map_err(|e| e.to_string())?;
    let png = guard
        .as_ref()
        .ok_or_else(|| "no capture available".to_string())?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    Ok(format!("data:image/png;base64,{b64}"))
}

/// Cancel the current capture and close the overlay.
#[command]
pub async fn screenshot_cancel(
    app: AppHandle,
    state: State<'_, ScreenshotState>,
) -> Result<(), String> {
    if let Ok(mut guard) = state.capture.lock() {
        *guard = None;
    }
    if let Some(win) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = win.close();
    }
    Ok(())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinCreateArgs {
    /// Cropped image as a PNG data URL.
    pub data_url: String,
    /// Logical screen X of the top-left corner.
    pub x: f64,
    /// Logical screen Y of the top-left corner.
    pub y: f64,
    /// Logical width of the pin window.
    pub width: f64,
    /// Logical height of the pin window.
    pub height: f64,
}

fn decode_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let comma = data_url
        .find(',')
        .ok_or_else(|| "invalid data URL".to_string())?;
    let b64 = &data_url[comma + 1..];
    base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("base64 decode failed: {e}"))
}

/// Create a pinned window showing the cropped region.
#[command]
pub async fn pin_create(
    app: AppHandle,
    state: State<'_, ScreenshotState>,
    args: PinCreateArgs,
) -> Result<String, String> {
    let png = decode_data_url(&args.data_url)?;

    let id = PIN_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pin_id = format!("{PIN_LABEL_PREFIX}{id}");

    {
        let mut pins = state.pins.lock().map_err(|e| e.to_string())?;
        pins.insert(pin_id.clone(), png);
    }

    // Close the overlay before showing the pin.
    if let Some(overlay) = app.get_webview_window(OVERLAY_LABEL) {
        let _ = overlay.close();
    }
    if let Ok(mut guard) = state.capture.lock() {
        *guard = None;
    }

    let win = tauri::WebviewWindowBuilder::new(
        &app,
        &pin_id,
        WebviewUrl::App("index.html".into()),
    )
    .title("Pinned Screenshot")
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(true)
    .build()
    .map_err(|e| format!("pin window failed: {e}"))?;

    // Position and size using logical units so it lands exactly over the region.
    let _ = win.set_size(LogicalSize::new(args.width, args.height));
    let _ = win.set_position(LogicalPosition::new(args.x, args.y));
    let _ = win.set_focus();

    Ok(pin_id)
}

/// Pin window fetches its image as a base64 PNG data URL.
#[command]
pub async fn pin_get_image(
    id: String,
    state: State<'_, ScreenshotState>,
) -> Result<String, String> {
    let pins = state.pins.lock().map_err(|e| e.to_string())?;
    let png = pins
        .get(&id)
        .ok_or_else(|| format!("pin {id} not found"))?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
    Ok(format!("data:image/png;base64,{b64}"))
}

/// Copy a pin's image to the system clipboard.
#[command]
pub async fn pin_copy(
    app: AppHandle,
    id: String,
    state: State<'_, ScreenshotState>,
) -> Result<(), String> {
    let png = {
        let pins = state.pins.lock().map_err(|e| e.to_string())?;
        pins.get(&id)
            .ok_or_else(|| format!("pin {id} not found"))?
            .clone()
    };

    // Decode to raw RGBA for the clipboard image API.
    let dynimg = image::load_from_memory(&png).map_err(|e| format!("decode failed: {e}"))?;
    let rgba = dynimg.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    let image = tauri::image::Image::new_owned(rgba.into_raw(), w, h);
    app.clipboard()
        .write_image(&image)
        .map_err(|e| format!("clipboard write failed: {e}"))?;
    Ok(())
}

/// Close a pin window and drop its image.
#[command]
pub async fn pin_close(
    app: AppHandle,
    id: String,
    state: State<'_, ScreenshotState>,
) -> Result<(), String> {
    if let Ok(mut pins) = state.pins.lock() {
        pins.remove(&id);
    }
    if let Some(win) = app.get_webview_window(&id) {
        let _ = win.close();
    }
    Ok(())
}
