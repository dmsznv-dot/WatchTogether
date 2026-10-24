//! Windows video output. libmpv renders each new frame straight into WebView2 shared
//! memory; the page uploads it to a WebGL texture (SDR) or an HDR WebGPU canvas. Only a
//! tiny "render into buffer N" request crosses IPC, so 1080p60 does not push hundreds
//! of MB/s through it.
//!
//! Rendering uses the GPU when the driver offers OpenGL 3 (HDR output for HDR monitors,
//! correct tone mapping of HDR video for SDR ones), else libmpv's CPU renderer (SDR,
//! 8-bit, no tone mapping). `render_copy` keeps the copy-through-IPC path for WebView2
//! runtimes without shared buffers.
use crate::gpu_windows::GpuVideo;
use media_engine::mpv::{DynamicRange, MpvPlayer, SoftwareRenderer, MAX_HEIGHT, MAX_WIDTH};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex},
};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Environment12, ICoreWebView2SharedBuffer, ICoreWebView2_17,
    COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY,
};
use windows_core::{Interface, HSTRING};

/// One 1080p frame at up to 8 bytes per pixel (RGBA16F for HDR).
const FRAME_BYTES: usize = MAX_WIDTH as usize * MAX_HEIGHT as usize * 8;
/// Two buffers: the page reads one while the next frame is rendered into the other.
const BUFFER_COUNT: usize = 2;

thread_local! {
    // WebView2 objects belong to the UI thread. Kept for the app's lifetime so the
    // shared memory stays mapped; the page may re-request them after a reload.
    static SHARED_BUFFERS: RefCell<Vec<ICoreWebView2SharedBuffer>> = const { RefCell::new(Vec::new()) };
}

enum Backend {
    Gpu(GpuVideo),
    Software(Mutex<SoftwareRenderer>),
}

/// Pixel format of a rendered frame, as the page needs to know it.
#[derive(serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameFormat {
    /// 8-bit RGB (4th byte ignored).
    Rgba8,
    /// Linear-light half floats, 1.0 = SDR white.
    Rgba16f,
}

#[derive(serde::Serialize)]
pub struct FrameInfo {
    pub fresh: bool,
    pub format: FrameFormat,
}

pub struct WindowsFrames {
    backend: Backend,
    /// Addresses of the shared buffers' memory, readable from any thread.
    regions: Mutex<Vec<usize>>,
}

impl WindowsFrames {
    /// Picks the GPU renderer when possible; libmpv allows one render context per player.
    pub fn new(player: &Arc<MpvPlayer>) -> Result<Self, media_engine::MediaEngineError> {
        let backend = match GpuVideo::start(Arc::clone(player)) {
            Ok(gpu) => {
                tracing::info!("Video: GPU renderer, {}", gpu.description);
                Backend::Gpu(gpu)
            }
            Err(e) => {
                tracing::info!("Video: CPU renderer (no GPU path: {e})");
                Backend::Software(Mutex::new(player.software_renderer()?))
            }
        };
        Ok(Self {
            backend,
            regions: Mutex::new(Vec::new()),
        })
    }

    /// Whether frames can be HDR (only the GPU renderer can output them).
    pub fn supports_hdr(&self) -> bool {
        matches!(self.backend, Backend::Gpu(_))
    }

    /// Creates the shared buffers once and posts them to the page, which receives
    /// them as `sharedbufferreceived` events tagged `{"wtFrameBuffer": index}`.
    /// Must run on the UI thread (inside `with_webview`).
    pub fn share(&self, webview: &tauri::webview::PlatformWebview) -> Result<usize, String> {
        let unsupported = |e: windows_core::Error| {
            format!("WebView2 runtime does not support shared buffers: {e}")
        };
        unsafe {
            let environment: ICoreWebView2Environment12 =
                webview.environment().cast().map_err(unsupported)?;
            let core: ICoreWebView2_17 = webview
                .controller()
                .CoreWebView2()
                .map_err(|e| e.to_string())?
                .cast()
                .map_err(unsupported)?;
            let mut regions = self.regions.lock().map_err(|e| e.to_string())?;
            SHARED_BUFFERS.with(|cell| {
                let mut buffers = cell.borrow_mut();
                while buffers.len() < BUFFER_COUNT {
                    let buffer = environment
                        .CreateSharedBuffer(FRAME_BYTES as u64)
                        .map_err(|e| format!("Cannot allocate a video buffer: {e}"))?;
                    let mut memory = std::ptr::null_mut();
                    buffer.Buffer(&mut memory).map_err(|e| e.to_string())?;
                    regions.push(memory as usize);
                    buffers.push(buffer);
                }
                for (index, buffer) in buffers.iter().enumerate() {
                    let tag = HSTRING::from(format!("{{\"wtFrameBuffer\":{index}}}"));
                    core.PostSharedBufferToScript(
                        buffer,
                        COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY,
                        &tag,
                    )
                    .map_err(|e| e.to_string())?;
                }
                Ok::<_, String>(buffers.len())
            })
        }
    }

    /// Renders into shared buffer `index` if mpv has a new frame (or `force`, e.g.
    /// after a resize). `hdr_peak_nits` asks for HDR output for a monitor with that
    /// peak; the CPU renderer ignores it and answers with SDR frames.
    pub fn render_shared(
        &self,
        index: usize,
        width: u32,
        height: u32,
        force: bool,
        hdr_peak_nits: Option<f32>,
    ) -> Result<FrameInfo, String> {
        let region = *self
            .regions
            .lock()
            .map_err(|e| e.to_string())?
            .get(index)
            .ok_or("Video buffer is not shared yet")?;
        match &self.backend {
            Backend::Gpu(gpu) => {
                let range = match hdr_peak_nits {
                    Some(peak_nits) => DynamicRange::Hdr { peak_nits },
                    None => DynamicRange::Sdr,
                };
                let fresh = gpu.render(region, FRAME_BYTES, width, height, force, range)?;
                let format = match range {
                    DynamicRange::Sdr => FrameFormat::Rgba8,
                    DynamicRange::Hdr { .. } => FrameFormat::Rgba16f,
                };
                Ok(FrameInfo { fresh, format })
            }
            Backend::Software(renderer) => {
                let mut renderer = renderer.lock().map_err(|e| e.to_string())?;
                if !renderer.has_new_frame() && !force {
                    return Ok(FrameInfo { fresh: false, format: FrameFormat::Rgba8 });
                }
                // The page only reads buffer `index` after this call returns and never
                // requests it again while reading, so nothing else touches it now.
                let memory = unsafe { std::slice::from_raw_parts_mut(region as *mut u8, FRAME_BYTES) };
                renderer
                    .render_into(width, height, width as usize * 4, memory)
                    .map_err(|e| e.to_string())?;
                Ok(FrameInfo { fresh: true, format: FrameFormat::Rgba8 })
            }
        }
    }

    /// Fallback: returns an opaque RGBA8 frame by value, or no bytes when nothing changed.
    pub fn render_copy(&self, width: u32, height: u32, force: bool) -> Result<Vec<u8>, String> {
        match &self.backend {
            Backend::Software(renderer) => {
                let mut renderer = renderer.lock().map_err(|e| e.to_string())?;
                if !renderer.has_new_frame() && !force {
                    return Ok(Vec::new());
                }
                renderer.render(width, height).map_err(|e| e.to_string())
            }
            Backend::Gpu(gpu) => {
                let mut frame = vec![0u8; width as usize * height as usize * 4];
                let fresh = gpu.render(frame.as_mut_ptr() as usize, frame.len(), width, height, force, DynamicRange::Sdr)?;
                if !fresh {
                    return Ok(Vec::new());
                }
                for p in frame.chunks_exact_mut(4) {
                    p[3] = 255;
                }
                Ok(frame)
            }
        }
    }
}
