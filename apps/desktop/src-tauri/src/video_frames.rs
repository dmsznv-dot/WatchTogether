//! Windows video output. libmpv renders each new frame on the CPU straight into
//! WebView2 shared memory; the page uploads it to a WebGL texture. Only a tiny
//! "render into buffer N" request crosses IPC, so 1080p60 does not push ~500 MB/s
//! through it. `render_copy` keeps the older copy-through-IPC path as a fallback
//! for WebView2 runtimes without shared buffers.
use media_engine::mpv::{SoftwareRenderer, MAX_HEIGHT, MAX_WIDTH};
use std::{cell::RefCell, sync::Mutex};
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2Environment12, ICoreWebView2SharedBuffer, ICoreWebView2_17,
    COREWEBVIEW2_SHARED_BUFFER_ACCESS_READ_ONLY,
};
use windows_core::{Interface, HSTRING};

/// One 1080p frame in `rgb0` (4 bytes per pixel).
const FRAME_BYTES: usize = MAX_WIDTH as usize * MAX_HEIGHT as usize * 4;
/// Two buffers: the page reads one while the next frame is rendered into the other.
const BUFFER_COUNT: usize = 2;

thread_local! {
    // WebView2 objects belong to the UI thread. Kept for the app's lifetime so the
    // shared memory stays mapped; the page may re-request them after a reload.
    static SHARED_BUFFERS: RefCell<Vec<ICoreWebView2SharedBuffer>> = const { RefCell::new(Vec::new()) };
}

pub struct WindowsFrames {
    renderer: Mutex<SoftwareRenderer>,
    /// Addresses of the shared buffers' memory, readable from any thread.
    regions: Mutex<Vec<usize>>,
}

impl WindowsFrames {
    pub fn new(renderer: SoftwareRenderer) -> Self {
        Self {
            renderer: Mutex::new(renderer),
            regions: Mutex::new(Vec::new()),
        }
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
    /// after a resize). Returns whether the buffer now holds a frame to upload.
    pub fn render_shared(
        &self,
        index: usize,
        width: u32,
        height: u32,
        force: bool,
    ) -> Result<bool, String> {
        let region = *self
            .regions
            .lock()
            .map_err(|e| e.to_string())?
            .get(index)
            .ok_or("Video buffer is not shared yet")?;
        let mut renderer = self.renderer.lock().map_err(|e| e.to_string())?;
        if !renderer.has_new_frame() && !force {
            return Ok(false);
        }
        // The page only reads buffer `index` after this call returns and never
        // requests it again while reading, so nothing else touches it now.
        let memory = unsafe { std::slice::from_raw_parts_mut(region as *mut u8, FRAME_BYTES) };
        renderer
            .render_into(width, height, width as usize * 4, memory)
            .map_err(|e| e.to_string())?;
        Ok(true)
    }

    /// Fallback: returns an opaque RGBA frame by value, or no bytes when nothing changed.
    pub fn render_copy(&self, width: u32, height: u32, force: bool) -> Result<Vec<u8>, String> {
        let mut renderer = self.renderer.lock().map_err(|e| e.to_string())?;
        if !renderer.has_new_frame() && !force {
            return Ok(Vec::new());
        }
        renderer.render(width, height).map_err(|e| e.to_string())
    }
}
