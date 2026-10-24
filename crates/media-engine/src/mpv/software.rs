//! CPU render API for WebView canvas embedding on Windows. No layered HWNDs.
use super::MpvPlayer;
use crate::MediaEngineError;
use std::{ffi::c_void, sync::Arc};
#[repr(C)]
struct Param {
    kind: i32,
    data: *mut c_void,
}
type Create =
    unsafe extern "C" fn(*mut *mut c_void, *mut super::ffi::mpv_handle, *mut Param) -> i32;
type Render = unsafe extern "C" fn(*mut c_void, *mut Param) -> i32;
type Free = unsafe extern "C" fn(*mut c_void);
type Update = unsafe extern "C" fn(*mut c_void) -> u64;
/// MPV_RENDER_UPDATE_FRAME: a new video frame (or a redraw) is due.
const UPDATE_FRAME: u64 = 1;
/// Largest frame the canvas path renders (1080p).
pub const MAX_WIDTH: u32 = 1920;
pub const MAX_HEIGHT: u32 = 1080;
pub struct SoftwareRenderer {
    context: *mut c_void,
    render_fn: Render,
    free_fn: Free,
    update_fn: Update,
    _player: Arc<MpvPlayer>,
}
// Render calls are serialized by a mutex. The SW API has no GPU thread affinity.
unsafe impl Send for SoftwareRenderer {}
impl MpvPlayer {
    pub fn software_renderer(self: &Arc<Self>) -> Result<SoftwareRenderer, MediaEngineError> {
        unsafe {
            let create: Create =
                std::mem::transmute(self.api.render_symbol(b"mpv_render_context_create\0")?);
            let render_fn: Render =
                std::mem::transmute(self.api.render_symbol(b"mpv_render_context_render\0")?);
            let free_fn: Free =
                std::mem::transmute(self.api.render_symbol(b"mpv_render_context_free\0")?);
            let update_fn: Update =
                std::mem::transmute(self.api.render_symbol(b"mpv_render_context_update\0")?);
            let mut context = std::ptr::null_mut();
            let mut params = [
                Param {
                    kind: 1,
                    data: b"sw\0".as_ptr() as *mut c_void,
                },
                Param {
                    kind: 0,
                    data: std::ptr::null_mut(),
                },
            ];
            let result = create(&mut context, self.handle.0, params.as_mut_ptr());
            if result < 0 {
                return Err(MediaEngineError::LibMpvInitFailed(format!(
                    "Canvas renderer: {result}"
                )));
            }
            Ok(SoftwareRenderer {
                context,
                render_fn,
                free_fn,
                update_fn,
                _player: self.clone(),
            })
        }
    }
}
impl SoftwareRenderer {
    /// True when mpv has a new frame to show. Polling this avoids re-rendering
    /// (and re-uploading) the same picture between video frames.
    pub fn has_new_frame(&mut self) -> bool {
        unsafe { (self.update_fn)(self.context) & UPDATE_FRAME != 0 }
    }

    /// Renders the current frame as `rgb0` (4 bytes per pixel, 4th byte undefined)
    /// straight into `dst`, which must hold `stride * height` bytes.
    pub fn render_into(
        &mut self,
        width: u32,
        height: u32,
        stride: usize,
        dst: &mut [u8],
    ) -> Result<(), MediaEngineError> {
        if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
            return Err(MediaEngineError::Internal("Invalid frame size".into()));
        }
        // mpv's SW render API needs 4-byte pixel alignment and a stride of whole pixels.
        if stride < width as usize * 4
            || !stride.is_multiple_of(4)
            || dst.len() < stride * height as usize
            || !(dst.as_ptr() as usize).is_multiple_of(4)
        {
            return Err(MediaEngineError::Internal("Invalid frame buffer".into()));
        }
        let mut size = [width as i32, height as i32];
        let mut stride = stride;
        let mut block = 0i32;
        let mut params = [
            Param {
                kind: 17,
                data: size.as_mut_ptr().cast(),
            },
            Param {
                kind: 18,
                data: b"rgb0\0".as_ptr() as *mut c_void,
            },
            Param {
                kind: 19,
                data: (&mut stride as *mut usize).cast(),
            },
            Param {
                kind: 20,
                data: dst.as_mut_ptr().cast(),
            },
            Param {
                kind: 12,
                data: (&mut block as *mut i32).cast(),
            },
            Param {
                kind: 0,
                data: std::ptr::null_mut(),
            },
        ];
        let result = unsafe { (self.render_fn)(self.context, params.as_mut_ptr()) };
        if result < 0 {
            return Err(MediaEngineError::Internal(format!(
                "Frame rendering fails: {result}"
            )));
        }
        Ok(())
    }

    /// Renders an opaque RGBA frame into a new buffer.
    pub fn render(&mut self, width: u32, height: u32) -> Result<Vec<u8>, MediaEngineError> {
        if width == 0 || height == 0 || width > MAX_WIDTH || height > MAX_HEIGHT {
            return Err(MediaEngineError::Internal("Invalid frame size".into()));
        }
        let stride = width as usize * 4;
        let mut bytes = vec![0u8; stride * height as usize];
        if (bytes.as_ptr() as usize).is_multiple_of(4) {
            // Allocators return aligned blocks this large in practice: no extra copy.
            self.render_into(width, height, stride, &mut bytes)?;
        } else {
            let mut storage = vec![0u32; width as usize * height as usize];
            let aligned = unsafe {
                std::slice::from_raw_parts_mut(storage.as_mut_ptr().cast::<u8>(), bytes.len())
            };
            self.render_into(width, height, stride, aligned)?;
            bytes.copy_from_slice(aligned);
        }
        for p in bytes.chunks_exact_mut(4) {
            p[3] = 255;
        }
        Ok(bytes)
    }
}
impl Drop for SoftwareRenderer {
    fn drop(&mut self) {
        unsafe { (self.free_fn)(self.context) };
    }
}
