//! Main-thread-only AppKit surface. The caller must destroy it before the player.
use super::MpvPlayer;
use crate::error::MediaEngineError;
use std::ffi::c_void;

#[repr(C)]
struct RenderApi {
    create: *const c_void,
    render: *const c_void,
    free: *const c_void,
    report_swap: *const c_void,
    update: *const c_void,
    set_update_callback: *const c_void,
    set_property_string: *const c_void,
    get_property_string: *const c_void,
    mpv_free: *const c_void,
}
extern "C" {
    fn wt_surface_create(parent: *mut c_void, handle: *mut c_void, api: RenderApi) -> *mut c_void;
    fn wt_surface_destroy(surface: *mut c_void);
}
impl MpvPlayer {
    /// # Safety
    /// Call on the AppKit main thread with a live NSView. Keep this player alive
    /// until `destroy_macos_surface` returns, also on the main thread.
    pub unsafe fn attach_macos_surface(
        &self,
        parent: *mut c_void,
    ) -> Result<*mut c_void, MediaEngineError> {
        let api = RenderApi {
            create: self.api.render_symbol(b"mpv_render_context_create\0")?,
            render: self.api.render_symbol(b"mpv_render_context_render\0")?,
            free: self.api.render_symbol(b"mpv_render_context_free\0")?,
            report_swap: self
                .api
                .render_symbol(b"mpv_render_context_report_swap\0")?,
            update: self.api.render_symbol(b"mpv_render_context_update\0")?,
            set_update_callback: self
                .api
                .render_symbol(b"mpv_render_context_set_update_callback\0")?,
            // HDR switching (target-trc/peak) and detecting HDR video (video-params/gamma).
            set_property_string: self.api.mpv_set_property_string as *const c_void,
            get_property_string: self.api.mpv_get_property_string as *const c_void,
            mpv_free: self.api.mpv_free as *const c_void,
        };
        let surface = wt_surface_create(parent, self.handle.0.cast(), api);
        if surface.is_null() {
            return Err(MediaEngineError::LibMpvInitFailed(
                "Cannot create embedded OpenGL surface".into(),
            ));
        }
        Ok(surface)
    }

    /// # Safety
    /// `surface` must be a live surface returned by this player; call exactly
    /// once on the AppKit main thread before destroying the player.
    pub unsafe fn destroy_macos_surface(&self, surface: *mut c_void) {
        wt_surface_destroy(surface);
    }
}
