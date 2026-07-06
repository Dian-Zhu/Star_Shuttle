// Windows screen-recording implementation.
//
// Capture:  Windows Graphics Capture (WGC) `CreateForWindow` on the main
//           window's HWND, delivering GPU frames via a free-threaded frame
//           pool. Each `FrameArrived` callback copies the GPU texture into a
//           CPU-side BGRA buffer (mirrors xcap's `texture_to_frame`, minus the
//           B/R swap since Media Foundation's RGB32 already expects BGRA byte
//           order) and hands it to the encode loop through a bounded channel.
// Encode:   Media Foundation Sink Writer, RGB32 in -> H.264/MP4 out, hardware
//           transforms enabled. Sample timestamps track real elapsed time, so
//           WGC's variable frame delivery is handled without stalling capture.
//
// Everything lives on one dedicated worker thread (COM initialised MTA); the
// `FrameArrived` callback fires on OS thread-pool threads but only touches the
// D3D device/context and the channel sender, all of which are Send+Sync.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, TrySendError};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use windows::core::{Interface, Ref, PCWSTR};
use windows::Foundation::TypedEventHandler;
use windows::Graphics::Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem};
use windows::Graphics::DirectX::Direct3D11::IDirect3DDevice;
use windows::Graphics::DirectX::DirectXPixelFormat;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D::D3D_DRIVER_TYPE_HARDWARE;
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
    D3D11_BOX, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE,
    D3D11_MAP_READ, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Media::MediaFoundation::{
    IMFAttributes, IMFMediaType, IMFSinkWriter, MFCreateAttributes, MFCreateMediaType,
    MFCreateMemoryBuffer, MFCreateSample, MFCreateSinkWriterFromURL, MFMediaType_Video, MFShutdown,
    MFStartup, MFVideoFormat_H264, MFVideoFormat_RGB32, MFVideoInterlace_Progressive,
    MF_MT_AVG_BITRATE, MF_MT_DEFAULT_STRIDE, MF_MT_FRAME_RATE, MF_MT_FRAME_SIZE,
    MF_MT_INTERLACE_MODE, MF_MT_MAJOR_TYPE, MF_MT_PIXEL_ASPECT_RATIO, MF_MT_SUBTYPE,
    MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, MF_SINK_WRITER_DISABLE_THROTTLING, MFSTARTUP_FULL,
    MF_VERSION,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::WinRT::Direct3D11::{
    CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess,
};
use windows::Win32::System::WinRT::Graphics::Capture::IGraphicsCaptureItemInterop;
use windows::core::factory;
use windows::Win32::Graphics::Dxgi::IDXGIDevice;

const TARGET_FPS: u32 = 60;

/// Handle to an in-flight recording, stored in Tauri managed state.
pub struct RecordingSession {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<Result<(), String>>,
    temp_path: PathBuf,
}

/// Begin recording the main window. Returns immediately; capture + encode run
/// on a dedicated worker thread until [`finish_recording`] is called.
pub fn start_recording(app: &AppHandle) -> Result<RecordingSession, String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;
    // tauri hands back an HWND from its own (older) `windows` crate; we only
    // need the raw pointer value, which we rebuild into our HWND on the worker.
    let raw_hwnd = window.hwnd().map_err(|e| format!("获取窗口句柄失败: {e}"))?.0 as isize;

    let temp_path = std::env::temp_dir().join(format!(
        "star-shuttle-recording-{}.mp4",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));

    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker_path = temp_path.clone();

    let handle = std::thread::Builder::new()
        .name("screen-recorder".into())
        .spawn(move || record_worker(raw_hwnd, worker_path, worker_stop))
        .map_err(|e| format!("启动录制线程失败: {e}"))?;

    Ok(RecordingSession {
        stop,
        handle,
        temp_path,
    })
}

/// Signal the worker to finalize the MP4 and wait for it. Returns the temp file
/// path on success; deletes the temp file and returns the error on failure.
pub fn finish_recording(session: RecordingSession) -> Result<PathBuf, String> {
    session.stop.store(true, Ordering::SeqCst);
    match session.handle.join() {
        Ok(Ok(())) => Ok(session.temp_path),
        Ok(Err(e)) => {
            let _ = std::fs::remove_file(&session.temp_path);
            Err(e)
        }
        Err(_) => {
            let _ = std::fs::remove_file(&session.temp_path);
            Err("录制线程异常终止".to_string())
        }
    }
}

/// Prompt for a save location and move the temp recording there. Returns the
/// final path, or `None` if the user cancelled (temp file is then deleted).
pub fn prompt_save_and_move(app: &AppHandle, temp_path: PathBuf) -> Result<Option<String>, String> {
    let default_name = format!(
        "录屏-{}.mp4",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );

    let chosen = app
        .dialog()
        .file()
        .set_file_name(&default_name)
        .add_filter("MP4 视频", &["mp4"])
        .blocking_save_file();

    let Some(chosen) = chosen else {
        let _ = std::fs::remove_file(&temp_path);
        return Ok(None);
    };

    let dest = chosen
        .into_path()
        .map_err(|e| format!("解析保存路径失败: {e}"))?;

    // `rename` fails across volumes (temp dir vs. user drive); fall back to copy.
    if std::fs::rename(&temp_path, &dest).is_err() {
        std::fs::copy(&temp_path, &dest).map_err(|e| format!("保存文件失败: {e}"))?;
        let _ = std::fs::remove_file(&temp_path);
    }

    Ok(Some(dest.to_string_lossy().to_string()))
}

// ---------------------------------------------------------------------------
// Worker: runs on its own thread with COM initialised MTA.
// ---------------------------------------------------------------------------

fn record_worker(raw_hwnd: isize, path: PathBuf, stop: Arc<AtomicBool>) -> Result<(), String> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|e| format!("COM 初始化失败: {e}"))?;
    }
    // Ensure COM is torn down no matter how we exit.
    let _com_guard = ComGuard;

    let result = record_inner(raw_hwnd, &path, &stop);
    if result.is_err() {
        let _ = std::fs::remove_file(&path);
    }
    result
}

struct ComGuard;
impl Drop for ComGuard {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn record_inner(raw_hwnd: isize, path: &PathBuf, stop: &Arc<AtomicBool>) -> Result<(), String> {
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|e| format!("Media Foundation 初始化失败: {e}"))?;
    }
    let _mf_guard = MfGuard;

    let hwnd = HWND(raw_hwnd as *mut core::ffi::c_void);

    // --- Create D3D11 device (BGRA support required for WGC) ---
    let (device, context) = create_d3d_device()?;

    // --- Build the WGC capture item for the window ---
    let item: GraphicsCaptureItem = unsafe {
        let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
            .map_err(|e| format!("获取捕获接口失败: {e}"))?;
        interop
            .CreateForWindow(hwnd)
            .map_err(|e| format!("为窗口创建捕获项失败: {e}"))?
    };

    let item_size = item.Size().map_err(|e| format!("读取窗口尺寸失败: {e}"))?;
    // Lock dimensions at start, rounded down to even (H.264 requires even dims).
    // If the window is later resized, frames are cropped/padded to these dims.
    let width = (item_size.Width.max(2) as u32) & !1;
    let height = (item_size.Height.max(2) as u32) & !1;

    // --- Media Foundation sink writer ---
    let writer = create_sink_writer(path)?;
    let stream_index = configure_streams(&writer, width, height)?;
    unsafe { writer.BeginWriting().map_err(|e| format!("开始写入失败: {e}"))? };

    // --- Frame pool + channel from callback to this (encode) thread ---
    let d3d_device: IDirect3DDevice = unsafe {
        let dxgi: IDXGIDevice = device.cast().map_err(|e| format!("获取 DXGI 设备失败: {e}"))?;
        let inspectable = CreateDirect3D11DeviceFromDXGIDevice(&dxgi)
            .map_err(|e| format!("创建 WinRT D3D 设备失败: {e}"))?;
        inspectable.cast().map_err(|e| format!("转换 D3D 设备失败: {e}"))?
    };

    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &d3d_device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        2,
        item_size,
    )
    .map_err(|e| format!("创建帧池失败: {e}"))?;

    // Bounded so a slow encoder drops frames instead of stalling capture.
    let (tx, rx) = sync_channel::<Vec<u8>>(3);

    let cb_context = context.clone();
    frame_pool
        .FrameArrived(&TypedEventHandler::<Direct3D11CaptureFramePool, _>::new(
            move |pool: Ref<Direct3D11CaptureFramePool>, _| {
                let Some(pool) = pool.as_ref() else {
                    return Ok(());
                };
                match copy_frame_bgra(pool, &cb_context, width, height) {
                    Ok(bytes) => match tx.try_send(bytes) {
                        Ok(()) | Err(TrySendError::Full(_)) => {}
                        Err(TrySendError::Disconnected(_)) => {}
                    },
                    Err(e) => log::warn!("录屏帧拷贝失败: {e}"),
                }
                Ok(())
            },
        ))
        .map_err(|e| format!("注册帧回调失败: {e}"))?;

    let session = frame_pool
        .CreateCaptureSession(&item)
        .map_err(|e| format!("创建捕获会话失败: {e}"))?;
    let _ = session.SetIsCursorCaptureEnabled(true);
    let _ = session.SetIsBorderRequired(false);
    session
        .StartCapture()
        .map_err(|e| format!("开始捕获失败: {e}"))?;

    // --- Encode loop: pull CPU frames, write H.264 samples ---
    let start = Instant::now();
    let mut last_ts: i64 = 0;
    let frame_stride = (width * 4) as usize;
    let expected_len = frame_stride * height as usize;

    while !stop.load(Ordering::SeqCst) {
        // Short timeout so we re-check the stop flag promptly even if no frames.
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(bytes) => {
                if bytes.len() != expected_len {
                    continue;
                }
                // 100ns units since capture start.
                let ts = (start.elapsed().as_nanos() / 100) as i64;
                let dur = (ts - last_ts).max(1);
                write_sample(&writer, stream_index, &bytes, ts, dur)?;
                last_ts = ts;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // Stop capture first so no further callbacks fire, then drain what's queued.
    let _ = session.Close();
    let _ = frame_pool.Close();
    while let Ok(bytes) = rx.try_recv() {
        if bytes.len() != expected_len {
            continue;
        }
        let ts = (start.elapsed().as_nanos() / 100) as i64;
        let dur = (ts - last_ts).max(1);
        write_sample(&writer, stream_index, &bytes, ts, dur)?;
        last_ts = ts;
    }

    unsafe { writer.Finalize().map_err(|e| format!("完成写入失败: {e}"))? };
    Ok(())
}

struct MfGuard;
impl Drop for MfGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = MFShutdown();
        }
    }
}

// ---------------------------------------------------------------------------
// D3D11 device + GPU->CPU frame copy (mirrors xcap, no B/R swap).
// ---------------------------------------------------------------------------

fn create_d3d_device() -> Result<(ID3D11Device, ID3D11DeviceContext), String> {
    unsafe {
        let mut device = None;
        let mut context = None;
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_HARDWARE,
            Default::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .map_err(|e| format!("创建 D3D11 设备失败: {e}"))?;
        let device = device.ok_or_else(|| "D3D11 设备为空".to_string())?;
        let context = context.ok_or_else(|| "D3D11 上下文为空".to_string())?;
        Ok((device, context))
    }
}

/// Copy the latest GPU frame into a tightly-packed BGRA buffer of `width`x
/// `height`. Regions beyond the source (window shrank) are left black; a larger
/// source (window grew) is cropped. Output stride is always `width*4`.
fn copy_frame_bgra(
    frame_pool: &Direct3D11CaptureFramePool,
    context: &ID3D11DeviceContext,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    unsafe {
        let frame = frame_pool
            .TryGetNextFrame()
            .map_err(|e| format!("获取帧失败: {e}"))?;

        let run = || -> Result<Vec<u8>, String> {
            let surface = frame.Surface().map_err(|e| format!("获取表面失败: {e}"))?;
            let access: IDirect3DDxgiInterfaceAccess =
                surface.cast().map_err(|e| format!("转换表面失败: {e}"))?;
            let source: ID3D11Texture2D = access
                .GetInterface()
                .map_err(|e| format!("获取纹理失败: {e}"))?;

            let mut src_desc = D3D11_TEXTURE2D_DESC::default();
            source.GetDesc(&mut src_desc);

            let copy_w = width.min(src_desc.Width);
            let copy_h = height.min(src_desc.Height);

            let mut staging_desc = src_desc;
            staging_desc.Width = copy_w;
            staging_desc.Height = copy_h;
            staging_desc.BindFlags = 0;
            staging_desc.MiscFlags = 0;
            staging_desc.Usage = D3D11_USAGE_STAGING;
            staging_desc.CPUAccessFlags = D3D11_CPU_ACCESS_READ.0 as u32;

            let device: ID3D11Device = source
                .GetDevice()
                .map_err(|e| format!("获取纹理设备失败: {e}"))?;

            let mut staging: Option<ID3D11Texture2D> = None;
            device
                .CreateTexture2D(&staging_desc, None, Some(&mut staging))
                .map_err(|e| format!("创建暂存纹理失败: {e}"))?;
            let staging = staging.ok_or_else(|| "暂存纹理为空".to_string())?;

            let region = D3D11_BOX {
                left: 0,
                top: 0,
                right: copy_w,
                bottom: copy_h,
                front: 0,
                back: 1,
            };
            let dst_res: ID3D11Resource = staging.cast().map_err(|e| e.to_string())?;
            let src_res: ID3D11Resource = source.cast().map_err(|e| e.to_string())?;
            context.CopySubresourceRegion(&dst_res, 0, 0, 0, 0, &src_res, 0, Some(&region));

            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            context
                .Map(&dst_res, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
                .map_err(|e| format!("映射暂存纹理失败: {e}"))?;

            let dst_stride = (width * 4) as usize;
            let copy_bytes = (copy_w * 4) as usize;
            let mut out = vec![0u8; dst_stride * height as usize];
            let src_ptr = mapped.pData as *const u8;
            for row in 0..copy_h as usize {
                let src_off = row * mapped.RowPitch as usize;
                let dst_off = row * dst_stride;
                let src_slice = std::slice::from_raw_parts(src_ptr.add(src_off), copy_bytes);
                out[dst_off..dst_off + copy_bytes].copy_from_slice(src_slice);
            }
            context.Unmap(&dst_res, 0);
            Ok(out)
        };

        let r = run();
        let _ = frame.Close();
        r
    }
}

// ---------------------------------------------------------------------------
// Media Foundation sink writer setup + sample writing.
// ---------------------------------------------------------------------------

fn create_sink_writer(path: &PathBuf) -> Result<IMFSinkWriter, String> {
    let wide: Vec<u16> = path
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mut attrs: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attrs, 2).map_err(|e| format!("创建写入属性失败: {e}"))?;
        let attrs = attrs.ok_or_else(|| "写入属性为空".to_string())?;
        attrs
            .SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1)
            .map_err(|e| e.to_string())?;
        attrs
            .SetUINT32(&MF_SINK_WRITER_DISABLE_THROTTLING, 1)
            .map_err(|e| e.to_string())?;

        MFCreateSinkWriterFromURL(PCWSTR(wide.as_ptr()), None, &attrs)
            .map_err(|e| format!("创建 MP4 写入器失败: {e}"))
    }
}

fn configure_streams(
    writer: &IMFSinkWriter,
    width: u32,
    height: u32,
) -> Result<u32, String> {
    let frame_size = ((width as u64) << 32) | height as u64;
    let frame_rate = ((TARGET_FPS as u64) << 32) | 1u64;
    let par = (1u64 << 32) | 1u64;
    // Rough bitrate target scaled by pixel throughput, clamped to sane bounds.
    let bitrate = ((width as u64 * height as u64 * TARGET_FPS as u64) / 10)
        .clamp(4_000_000, 16_000_000) as u32;

    unsafe {
        // Output: H.264.
        let out_type: IMFMediaType = MFCreateMediaType().map_err(|e| e.to_string())?;
        out_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|e| e.to_string())?;
        out_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).map_err(|e| e.to_string())?;
        out_type.SetUINT32(&MF_MT_AVG_BITRATE, bitrate).map_err(|e| e.to_string())?;
        out_type
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(|e| e.to_string())?;
        out_type.SetUINT64(&MF_MT_FRAME_SIZE, frame_size).map_err(|e| e.to_string())?;
        out_type.SetUINT64(&MF_MT_FRAME_RATE, frame_rate).map_err(|e| e.to_string())?;
        out_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, par).map_err(|e| e.to_string())?;
        let stream_index = writer
            .AddStream(&out_type)
            .map_err(|e| format!("添加视频流失败: {e}"))?;

        // Input: uncompressed RGB32 (BGRA byte order), top-down.
        let in_type: IMFMediaType = MFCreateMediaType().map_err(|e| e.to_string())?;
        in_type.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(|e| e.to_string())?;
        in_type.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(|e| e.to_string())?;
        in_type
            .SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)
            .map_err(|e| e.to_string())?;
        in_type
            .SetUINT32(&MF_MT_DEFAULT_STRIDE, (width * 4) as u32)
            .map_err(|e| e.to_string())?;
        in_type.SetUINT64(&MF_MT_FRAME_SIZE, frame_size).map_err(|e| e.to_string())?;
        in_type.SetUINT64(&MF_MT_FRAME_RATE, frame_rate).map_err(|e| e.to_string())?;
        in_type.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, par).map_err(|e| e.to_string())?;
        writer
            .SetInputMediaType(stream_index, &in_type, None)
            .map_err(|e| format!("设置输入格式失败: {e}"))?;

        Ok(stream_index)
    }
}

fn write_sample(
    writer: &IMFSinkWriter,
    stream_index: u32,
    data: &[u8],
    sample_time: i64,
    duration: i64,
) -> Result<(), String> {
    unsafe {
        let buffer = MFCreateMemoryBuffer(data.len() as u32).map_err(|e| e.to_string())?;
        let mut ptr: *mut u8 = std::ptr::null_mut();
        buffer
            .Lock(&mut ptr, None, None)
            .map_err(|e| format!("锁定缓冲区失败: {e}"))?;
        std::ptr::copy_nonoverlapping(data.as_ptr(), ptr, data.len());
        buffer.Unlock().map_err(|e| e.to_string())?;
        buffer.SetCurrentLength(data.len() as u32).map_err(|e| e.to_string())?;

        let sample = MFCreateSample().map_err(|e| e.to_string())?;
        sample.AddBuffer(&buffer).map_err(|e| e.to_string())?;
        sample.SetSampleTime(sample_time).map_err(|e| e.to_string())?;
        sample.SetSampleDuration(duration).map_err(|e| e.to_string())?;

        writer
            .WriteSample(stream_index, &sample)
            .map_err(|e| format!("写入采样失败: {e}"))?;
    }
    Ok(())
}
