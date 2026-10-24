//! Windows GPU video: an OpenGL (WGL) context on a hidden window, owned by one thread
//! that runs libmpv's GPU renderer, plus the HDR state of the window's monitor (DXGI).
use media_engine::mpv::{DynamicRange, GlFrameRenderer, MpvPlayer};
use std::{
    ffi::{c_char, c_void},
    sync::{mpsc, Arc},
};
use windows::{
    core::{s, w, PCSTR},
    Win32::{
        Foundation::HWND,
        Graphics::{
            Dxgi::{Common::DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020, CreateDXGIFactory1, IDXGIFactory1, IDXGIOutput6},
            Gdi::{GetDC, MonitorFromWindow, MONITOR_DEFAULTTONEAREST},
            OpenGL::{
                wglCreateContext, wglGetProcAddress, wglMakeCurrent, ChoosePixelFormat, SetPixelFormat,
                PFD_DOUBLEBUFFER, PFD_DRAW_TO_WINDOW, PFD_MAIN_PLANE, PFD_SUPPORT_OPENGL, PFD_TYPE_RGBA,
                PIXELFORMATDESCRIPTOR,
            },
        },
        System::LibraryLoader::{GetModuleHandleA, GetProcAddress},
        UI::WindowsAndMessaging::{CreateWindowExW, WINDOW_EX_STYLE, WS_POPUP},
    },
};
use windows_core::Interface;

/// HDR state of the monitor showing the window.
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct DisplayHdr {
    /// Windows "Use HDR" is on for this monitor.
    pub hdr: bool,
    /// Peak luminance the monitor reports, in nits (0 when unknown).
    pub peak_nits: f32,
}

pub fn display_hdr(window: isize) -> Result<DisplayHdr, String> {
    unsafe {
        let monitor = MonitorFromWindow(HWND(window as *mut c_void), MONITOR_DEFAULTTONEAREST);
        let factory: IDXGIFactory1 = CreateDXGIFactory1().map_err(|e| e.to_string())?;
        let mut adapter_index = 0;
        while let Ok(adapter) = factory.EnumAdapters1(adapter_index) {
            adapter_index += 1;
            let mut output_index = 0;
            while let Ok(output) = adapter.EnumOutputs(output_index) {
                output_index += 1;
                let Ok(output6) = output.cast::<IDXGIOutput6>() else { continue };
                let Ok(desc) = output6.GetDesc1() else { continue };
                if desc.Monitor == monitor {
                    return Ok(DisplayHdr {
                        hdr: desc.ColorSpace == DXGI_COLOR_SPACE_RGB_FULL_G2084_NONE_P2020,
                        peak_nits: desc.MaxLuminance,
                    });
                }
            }
        }
    }
    Ok(DisplayHdr { hdr: false, peak_nits: 0.0 })
}

/// OpenGL entry points: wglGetProcAddress covers extensions and GL >= 1.2; the
/// OpenGL 1.1 core lives in opengl32.dll itself.
unsafe extern "C" fn get_proc_address(_: *mut c_void, name: *const c_char) -> *mut c_void {
    let name = PCSTR(name.cast());
    if let Some(f) = wglGetProcAddress(name) {
        let address = f as usize;
        // Some drivers return small sentinels instead of NULL for unknown names.
        if !matches!(address, 1 | 2 | 3 | usize::MAX) {
            return address as *mut c_void;
        }
    }
    match GetModuleHandleA(s!("opengl32.dll")) {
        Ok(module) => GetProcAddress(module, name).map_or(std::ptr::null_mut(), |f| f as *mut c_void),
        Err(_) => std::ptr::null_mut(),
    }
}

unsafe fn make_gl_context_current() -> Result<(), String> {
    let window = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("STATIC"),
        w!("Watch Together GPU"),
        WS_POPUP,
        0,
        0,
        1,
        1,
        None,
        None,
        None,
        None,
    )
    .map_err(|e| e.to_string())?;
    let dc = GetDC(Some(window));
    let format = PIXELFORMATDESCRIPTOR {
        nSize: std::mem::size_of::<PIXELFORMATDESCRIPTOR>() as u16,
        nVersion: 1,
        dwFlags: PFD_DRAW_TO_WINDOW | PFD_SUPPORT_OPENGL | PFD_DOUBLEBUFFER,
        iPixelType: PFD_TYPE_RGBA,
        cColorBits: 32,
        cAlphaBits: 8,
        iLayerType: PFD_MAIN_PLANE.0 as u8,
        ..Default::default()
    };
    let index = ChoosePixelFormat(dc, &format);
    if index == 0 {
        return Err("No OpenGL pixel format".into());
    }
    SetPixelFormat(dc, index, &format).map_err(|e| e.to_string())?;
    // A compatibility context: drivers give their newest OpenGL version (mpv needs 3.x).
    let context = wglCreateContext(dc).map_err(|e| e.to_string())?;
    wglMakeCurrent(dc, context).map_err(|e| e.to_string())?;
    Ok(())
}

struct Request {
    region: usize,
    capacity: usize,
    width: u32,
    height: u32,
    force: bool,
    range: DynamicRange,
    reply: mpsc::Sender<Result<bool, String>>,
}

/// The GPU render thread; every OpenGL call happens there.
pub struct GpuVideo {
    requests: mpsc::Sender<Request>,
    pub description: String,
}

impl GpuVideo {
    /// Starts the render thread. Fails (and the caller uses the CPU path) when there is
    /// no usable OpenGL 3 driver, e.g. on virtual machines without a GPU.
    pub fn start(player: Arc<MpvPlayer>) -> Result<Self, String> {
        let (requests, rx) = mpsc::channel::<Request>();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<String, String>>();
        std::thread::Builder::new()
            .name("gpu-video".into())
            .spawn(move || {
                let renderer = unsafe {
                    make_gl_context_current().and_then(|()| {
                        player
                            .gl_frame_renderer(get_proc_address, std::ptr::null_mut())
                            .map_err(|e| e.to_string())
                    })
                };
                let mut renderer: GlFrameRenderer = match renderer {
                    Ok(r) => {
                        let _ = ready_tx.send(Ok(r.describe()));
                        r
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                while let Ok(request) = rx.recv() {
                    let _ = request.reply.send(serve(&mut renderer, &request));
                }
            })
            .map_err(|e| e.to_string())?;
        let description = ready_rx.recv().map_err(|_| "GPU video thread stopped".to_string())??;
        Ok(Self { requests, description })
    }

    /// Renders into the shared buffer at `region` (`capacity` bytes) if there is a new
    /// frame, the size or range changed, or `force`.
    pub fn render(
        &self,
        region: usize,
        capacity: usize,
        width: u32,
        height: u32,
        force: bool,
        range: DynamicRange,
    ) -> Result<bool, String> {
        let (reply, result) = mpsc::channel();
        self.requests
            .send(Request { region, capacity, width, height, force, range, reply })
            .map_err(|_| "GPU video thread stopped".to_string())?;
        result.recv().map_err(|_| "GPU video thread stopped".to_string())?
    }
}

fn serve(renderer: &mut GlFrameRenderer, r: &Request) -> Result<bool, String> {
    let mut force = r.force;
    if renderer.dynamic_range() != r.range {
        renderer.set_dynamic_range(r.range).map_err(|e| e.to_string())?;
        force = true;
    }
    if !renderer.has_new_frame() && !force {
        return Ok(false);
    }
    // The page reads this buffer only after the reply and never while it is requested.
    let memory = unsafe { std::slice::from_raw_parts_mut(r.region as *mut u8, r.capacity) };
    renderer.render_into(r.width, r.height, memory).map_err(|e| e.to_string())?;
    Ok(true)
}
