//! Native container below WebView2 so the HUD retains keyboard and mouse focus.
use std::ffi::c_void;
type Hwnd = *mut c_void;
#[link(name = "user32")]
extern "system" {
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Hwnd,
        menu: Hwnd,
        instance: Hwnd,
        param: *mut c_void,
    ) -> Hwnd;
    fn SetWindowPos(hwnd: Hwnd, after: Hwnd, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
}
pub fn attach(window: &tauri::WebviewWindow) -> Result<usize, Box<dyn std::error::Error>> {
    let parent = window.hwnd()?.0 as Hwnd;
    let size = window.inner_size()?;
    let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
    let hwnd = unsafe {
        CreateWindowExW(
            0,
            class.as_ptr(),
            std::ptr::null(),
            0x54000000,
            0,
            0,
            size.width as i32,
            size.height as i32,
            parent,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if hwnd.is_null() {
        return Err(std::io::Error::last_os_error().into());
    }
    let id = hwnd as usize;
    resize(id, size.width, size.height);
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::Resized(size) = event {
            resize(id, size.width, size.height);
        }
    });
    // The OS destroys this child with its parent window.
    Ok(id)
}
fn resize(id: usize, width: u32, height: u32) {
    // HWND_BOTTOM, SWP_NOACTIVATE.
    unsafe {
        SetWindowPos(
            id as Hwnd,
            1usize as Hwnd,
            0,
            0,
            width as i32,
            height as i32,
            0x0010,
        );
    }
}
