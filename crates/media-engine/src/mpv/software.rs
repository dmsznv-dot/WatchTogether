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
pub struct SoftwareRenderer {
    context: *mut c_void,
    render_fn: Render,
    free_fn: Free,
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
                _player: self.clone(),
            })
        }
    }
}
impl SoftwareRenderer {
    pub fn render(&mut self, width: u32, height: u32) -> Result<Vec<u8>, MediaEngineError> {
        if width == 0 || height == 0 || width > 1920 || height > 1080 {
            return Err(MediaEngineError::Internal("Invalid frame size".into()));
        }
        // At least pixel alignment is required by mpv's SW render API.
        let mut storage = vec![0u32; width as usize * height as usize];
        let mut size = [width as i32, height as i32];
        let mut stride = width as usize * 4;
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
                data: storage.as_mut_ptr().cast(),
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
        let mut bytes =
            unsafe { std::slice::from_raw_parts(storage.as_ptr().cast::<u8>(), storage.len() * 4) }
                .to_vec();
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
