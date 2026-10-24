//! GPU frame renderer: libmpv draws with OpenGL into an offscreen framebuffer and the
//! frame is read back into a caller-owned buffer. Unlike the CPU path it tone-maps HDR
//! video for SDR screens, and it can output HDR itself: linear light in half floats,
//! 1.0 = SDR reference white, values above 1.0 up to the display's peak.
//!
//! The caller owns the OpenGL context (WGL on Windows, CGL in tests); every method must
//! run on the thread where that context is current.
use super::MpvPlayer;
use crate::MediaEngineError;
use std::{
    ffi::{c_char, c_void, CStr, CString},
    sync::Arc,
};

/// `get_proc_address` for the current OpenGL context.
pub type GetProcAddress = unsafe extern "C" fn(ctx: *mut c_void, name: *const c_char) -> *mut c_void;

/// What the frames are for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DynamicRange {
    /// 8-bit sRGB; HDR video is tone-mapped down.
    Sdr,
    /// Linear BT.709 half floats (scRGB-like) for an HDR display with this peak.
    Hdr { peak_nits: f32 },
}

impl DynamicRange {
    /// Bytes per pixel in frames rendered for this range.
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            DynamicRange::Sdr => 4,
            DynamicRange::Hdr { .. } => 8,
        }
    }
}

#[repr(C)]
struct Param {
    kind: i32,
    data: *mut c_void,
}
#[repr(C)]
struct GlInitParams {
    get_proc_address: GetProcAddress,
    ctx: *mut c_void,
}
#[repr(C)]
struct GlFbo {
    fbo: i32,
    w: i32,
    h: i32,
    internal_format: i32,
}
type Create = unsafe extern "C" fn(*mut *mut c_void, *mut super::ffi::mpv_handle, *mut Param) -> i32;
type Render = unsafe extern "C" fn(*mut c_void, *mut Param) -> i32;
type Free = unsafe extern "C" fn(*mut c_void);
type Update = unsafe extern "C" fn(*mut c_void) -> u64;

const GL_TEXTURE_2D: u32 = 0x0DE1;
const GL_TEXTURE_MIN_FILTER: u32 = 0x2801;
const GL_TEXTURE_MAG_FILTER: u32 = 0x2800;
const GL_NEAREST: i32 = 0x2600;
const GL_RGBA: u32 = 0x1908;
const GL_RGBA8: i32 = 0x8058;
const GL_RGBA16F: i32 = 0x881A;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_HALF_FLOAT: u32 = 0x140B;
const GL_FRAMEBUFFER: u32 = 0x8D40;
const GL_COLOR_ATTACHMENT0: u32 = 0x8CE0;
const GL_FRAMEBUFFER_COMPLETE: u32 = 0x8CD5;
const GL_PACK_ALIGNMENT: u32 = 0x0D05;
const GL_VERSION: u32 = 0x1F02;
const GL_RENDERER: u32 = 0x1F01;

/// The few OpenGL entry points the readback needs (mpv loads its own).
struct Gl {
    gen_framebuffers: unsafe extern "system" fn(i32, *mut u32),
    bind_framebuffer: unsafe extern "system" fn(u32, u32),
    framebuffer_texture_2d: unsafe extern "system" fn(u32, u32, u32, u32, i32),
    check_framebuffer_status: unsafe extern "system" fn(u32) -> u32,
    delete_framebuffers: unsafe extern "system" fn(i32, *const u32),
    gen_textures: unsafe extern "system" fn(i32, *mut u32),
    bind_texture: unsafe extern "system" fn(u32, u32),
    tex_image_2d: unsafe extern "system" fn(u32, i32, i32, i32, i32, i32, u32, u32, *const c_void),
    tex_parameteri: unsafe extern "system" fn(u32, u32, i32),
    delete_textures: unsafe extern "system" fn(i32, *const u32),
    read_pixels: unsafe extern "system" fn(i32, i32, i32, i32, u32, u32, *mut c_void),
    pixel_storei: unsafe extern "system" fn(u32, i32),
    get_string: unsafe extern "system" fn(u32) -> *const u8,
}

impl Gl {
    // Each pointer is transmuted to the type of the field it initializes.
    #[allow(clippy::missing_transmute_annotations)]
    unsafe fn load(get: GetProcAddress, ctx: *mut c_void) -> Result<Self, MediaEngineError> {
        macro_rules! load {
            ($name:literal) => {{
                let name = CString::new($name).unwrap();
                let ptr = get(ctx, name.as_ptr());
                if ptr.is_null() {
                    return Err(MediaEngineError::LibMpvInitFailed(format!(
                        "OpenGL function {} is unavailable",
                        $name
                    )));
                }
                std::mem::transmute(ptr)
            }};
        }
        Ok(Self {
            gen_framebuffers: load!("glGenFramebuffers"),
            bind_framebuffer: load!("glBindFramebuffer"),
            framebuffer_texture_2d: load!("glFramebufferTexture2D"),
            check_framebuffer_status: load!("glCheckFramebufferStatus"),
            delete_framebuffers: load!("glDeleteFramebuffers"),
            gen_textures: load!("glGenTextures"),
            bind_texture: load!("glBindTexture"),
            tex_image_2d: load!("glTexImage2D"),
            tex_parameteri: load!("glTexParameteri"),
            delete_textures: load!("glDeleteTextures"),
            read_pixels: load!("glReadPixels"),
            pixel_storei: load!("glPixelStorei"),
            get_string: load!("glGetString"),
        })
    }

    unsafe fn string(&self, name: u32) -> String {
        let ptr = (self.get_string)(name);
        if ptr.is_null() {
            return "?".into();
        }
        CStr::from_ptr(ptr.cast()).to_string_lossy().into_owned()
    }
}

pub struct GlFrameRenderer {
    context: *mut c_void,
    render_fn: Render,
    free_fn: Free,
    update_fn: Update,
    gl: Gl,
    fbo: u32,
    texture: u32,
    /// Size and range the framebuffer was allocated for.
    allocated: Option<(u32, u32, DynamicRange)>,
    range: DynamicRange,
    player: Arc<MpvPlayer>,
}

impl MpvPlayer {
    /// # Safety
    /// An OpenGL 3.x context must be current on this thread, `get_proc_address` must
    /// resolve its functions, and the renderer must be used and dropped on this thread.
    pub unsafe fn gl_frame_renderer(
        self: &Arc<Self>,
        get_proc_address: GetProcAddress,
        proc_ctx: *mut c_void,
    ) -> Result<GlFrameRenderer, MediaEngineError> {
        let create: Create = std::mem::transmute(self.api.render_symbol(b"mpv_render_context_create\0")?);
        let render_fn: Render = std::mem::transmute(self.api.render_symbol(b"mpv_render_context_render\0")?);
        let free_fn: Free = std::mem::transmute(self.api.render_symbol(b"mpv_render_context_free\0")?);
        let update_fn: Update = std::mem::transmute(self.api.render_symbol(b"mpv_render_context_update\0")?);
        let gl = Gl::load(get_proc_address, proc_ctx)?;
        let mut init = GlInitParams { get_proc_address, ctx: proc_ctx };
        let mut params = [
            Param { kind: 1, data: c"opengl".as_ptr() as *mut c_void },
            Param { kind: 2, data: (&mut init as *mut GlInitParams).cast() },
            Param { kind: 0, data: std::ptr::null_mut() },
        ];
        let mut context = std::ptr::null_mut();
        let result = create(&mut context, self.handle.0, params.as_mut_ptr());
        if result < 0 {
            return Err(MediaEngineError::LibMpvInitFailed(format!(
                "GPU renderer ({}, OpenGL {}): {result}",
                gl.string(GL_RENDERER),
                gl.string(GL_VERSION)
            )));
        }
        let mut renderer = GlFrameRenderer {
            context,
            render_fn,
            free_fn,
            update_fn,
            gl,
            fbo: 0,
            texture: 0,
            allocated: None,
            range: DynamicRange::Sdr,
            player: Arc::clone(self),
        };
        renderer.set_dynamic_range(DynamicRange::Sdr)?;
        Ok(renderer)
    }
}

impl GlFrameRenderer {
    /// OpenGL renderer and version, e.g. for diagnostics.
    pub fn describe(&self) -> String {
        unsafe { format!("{} (OpenGL {})", self.gl.string(GL_RENDERER), self.gl.string(GL_VERSION)) }
    }

    pub fn dynamic_range(&self) -> DynamicRange {
        self.range
    }

    /// Switches mpv's output between SDR (tone-mapped) and HDR for a display with the given peak.
    pub fn set_dynamic_range(&mut self, range: DynamicRange) -> Result<(), MediaEngineError> {
        let (trc, prim, peak) = match range {
            DynamicRange::Sdr => ("srgb", "bt.709", "auto".to_string()),
            // libplacebo/mpv treat 1.0 in linear light as SDR reference white (203 nits).
            DynamicRange::Hdr { peak_nits } => ("linear", "bt.709", format!("{}", peak_nits.max(203.0))),
        };
        self.player.set_property_string("target-trc", trc)?;
        self.player.set_property_string("target-prim", prim)?;
        self.player.set_property_string("target-peak", &peak)?;
        self.range = range;
        Ok(())
    }

    /// True when mpv has a new frame to show (see `SoftwareRenderer::has_new_frame`).
    pub fn has_new_frame(&mut self) -> bool {
        unsafe { (self.update_fn)(self.context) & 1 != 0 }
    }

    /// Renders the current frame and reads it into `dst` (top row first, tightly packed):
    /// RGBA8 for SDR, RGBA16F (linear) for HDR — see [`DynamicRange::bytes_per_pixel`].
    pub fn render_into(&mut self, width: u32, height: u32, dst: &mut [u8]) -> Result<(), MediaEngineError> {
        if width == 0 || height == 0 || width > super::MAX_WIDTH || height > super::MAX_HEIGHT {
            return Err(MediaEngineError::Internal("Invalid frame size".into()));
        }
        let needed = width as usize * height as usize * self.range.bytes_per_pixel();
        if dst.len() < needed {
            return Err(MediaEngineError::Internal("Frame buffer too small".into()));
        }
        unsafe {
            self.ensure_framebuffer(width, height)?;
            let gl = &self.gl;
            let internal_format = match self.range {
                DynamicRange::Sdr => GL_RGBA8,
                DynamicRange::Hdr { .. } => GL_RGBA16F,
            };
            let mut fbo = GlFbo { fbo: self.fbo as i32, w: width as i32, h: height as i32, internal_format };
            // flip 0: the image's top row lands at y = 0, so glReadPixels returns it first.
            let (mut flip, mut block) = (0i32, 0i32);
            let mut params = [
                Param { kind: 3, data: (&mut fbo as *mut GlFbo).cast() },
                Param { kind: 4, data: (&mut flip as *mut i32).cast() },
                Param { kind: 12, data: (&mut block as *mut i32).cast() },
                Param { kind: 0, data: std::ptr::null_mut() },
            ];
            let result = (self.render_fn)(self.context, params.as_mut_ptr());
            if result < 0 {
                return Err(MediaEngineError::Internal(format!("GPU frame rendering fails: {result}")));
            }
            let pixel_type = match self.range {
                DynamicRange::Sdr => GL_UNSIGNED_BYTE,
                DynamicRange::Hdr { .. } => GL_HALF_FLOAT,
            };
            (gl.bind_framebuffer)(GL_FRAMEBUFFER, self.fbo);
            (gl.pixel_storei)(GL_PACK_ALIGNMENT, 1);
            (gl.read_pixels)(0, 0, width as i32, height as i32, GL_RGBA, pixel_type, dst.as_mut_ptr().cast());
            (gl.bind_framebuffer)(GL_FRAMEBUFFER, 0);
        }
        Ok(())
    }

    unsafe fn ensure_framebuffer(&mut self, width: u32, height: u32) -> Result<(), MediaEngineError> {
        if self.allocated == Some((width, height, self.range)) {
            return Ok(());
        }
        let gl = &self.gl;
        if self.fbo == 0 {
            (gl.gen_framebuffers)(1, &mut self.fbo);
            (gl.gen_textures)(1, &mut self.texture);
        }
        let (internal_format, pixel_type) = match self.range {
            DynamicRange::Sdr => (GL_RGBA8, GL_UNSIGNED_BYTE),
            DynamicRange::Hdr { .. } => (GL_RGBA16F, GL_HALF_FLOAT),
        };
        (gl.bind_texture)(GL_TEXTURE_2D, self.texture);
        (gl.tex_parameteri)(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
        (gl.tex_parameteri)(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
        (gl.tex_image_2d)(
            GL_TEXTURE_2D, 0, internal_format, width as i32, height as i32, 0, GL_RGBA, pixel_type,
            std::ptr::null(),
        );
        (gl.bind_texture)(GL_TEXTURE_2D, 0);
        (gl.bind_framebuffer)(GL_FRAMEBUFFER, self.fbo);
        (gl.framebuffer_texture_2d)(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, self.texture, 0);
        let status = (gl.check_framebuffer_status)(GL_FRAMEBUFFER);
        (gl.bind_framebuffer)(GL_FRAMEBUFFER, 0);
        if status != GL_FRAMEBUFFER_COMPLETE {
            return Err(MediaEngineError::Internal(format!("Offscreen framebuffer incomplete: {status:#x}")));
        }
        self.allocated = Some((width, height, self.range));
        Ok(())
    }
}

impl Drop for GlFrameRenderer {
    fn drop(&mut self) {
        unsafe {
            // The render context first: it still uses the GL context and our FBO's name space.
            (self.free_fn)(self.context);
            if self.fbo != 0 {
                (self.gl.delete_framebuffers)(1, &self.fbo);
                (self.gl.delete_textures)(1, &self.texture);
            }
        }
    }
}
