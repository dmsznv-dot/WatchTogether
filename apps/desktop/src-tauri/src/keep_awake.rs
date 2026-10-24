//! Keeps the computer and display awake while a video plays. mpv does this itself
//! only when it owns the video window; embedded through `vo=libmpv` it does not,
//! so Windows went to sleep mid-movie when the mouse stayed still.
use std::sync::mpsc;

/// Holds a "do not sleep" request while on; requests run on one dedicated thread
/// because Windows ties `SetThreadExecutionState` to the calling thread.
pub struct KeepAwake {
    requests: mpsc::Sender<(bool, mpsc::Sender<Result<(), String>>)>,
}

impl KeepAwake {
    pub fn new() -> Self {
        let (requests, rx) = mpsc::channel::<(bool, mpsc::Sender<Result<(), String>>)>();
        std::thread::Builder::new()
            .name("keep-awake".into())
            .spawn(move || {
                let mut guard = platform::Guard::default();
                while let Ok((on, reply)) = rx.recv() {
                    let _ = reply.send(guard.set(on));
                }
                // Channel closed (app exits): dropping the guard releases the request.
            })
            .expect("spawn keep-awake thread");
        Self { requests }
    }

    /// Prevents (true) or allows again (false) system and display sleep.
    pub fn set(&self, on: bool) -> Result<(), String> {
        let (reply, result) = mpsc::channel();
        self.requests
            .send((on, reply))
            .map_err(|_| "keep-awake thread stopped".to_string())?;
        result
            .recv()
            .map_err(|_| "keep-awake thread stopped".to_string())?
    }
}

impl Default for KeepAwake {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_os = "windows")]
mod platform {
    const ES_CONTINUOUS: u32 = 0x8000_0000;
    const ES_SYSTEM_REQUIRED: u32 = 0x0000_0001;
    const ES_DISPLAY_REQUIRED: u32 = 0x0000_0002;
    #[link(name = "kernel32")]
    extern "system" {
        fn SetThreadExecutionState(flags: u32) -> u32;
    }

    #[derive(Default)]
    pub struct Guard {
        on: bool,
    }
    impl Guard {
        pub fn set(&mut self, on: bool) -> Result<(), String> {
            if on == self.on {
                return Ok(());
            }
            let flags = if on {
                ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED
            } else {
                ES_CONTINUOUS
            };
            // Returns the previous state, or 0 on failure.
            if unsafe { SetThreadExecutionState(flags) } == 0 {
                return Err("SetThreadExecutionState failed".into());
            }
            self.on = on;
            Ok(())
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = self.set(false);
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::{c_char, c_void};
    type CFStringRef = *const c_void;
    const UTF8: u32 = 0x0800_0100;
    const ASSERTION_LEVEL_ON: u32 = 255;
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFStringCreateWithCString(alloc: *const c_void, s: *const c_char, enc: u32) -> CFStringRef;
        fn CFRelease(cf: *const c_void);
    }
    #[link(name = "IOKit", kind = "framework")]
    extern "C" {
        fn IOPMAssertionCreateWithName(
            kind: CFStringRef,
            level: u32,
            name: CFStringRef,
            id: *mut u32,
        ) -> i32;
        fn IOPMAssertionRelease(id: u32) -> i32;
    }

    #[derive(Default)]
    pub struct Guard {
        assertion: Option<u32>,
    }
    impl Guard {
        pub fn set(&mut self, on: bool) -> Result<(), String> {
            match (on, self.assertion) {
                (true, None) => unsafe {
                    // Keeps the display (and with it the system) awake, like a video player.
                    let kind = CFStringCreateWithCString(
                        std::ptr::null(),
                        c"PreventUserIdleDisplaySleep".as_ptr(),
                        UTF8,
                    );
                    let name = CFStringCreateWithCString(
                        std::ptr::null(),
                        c"Watch Together video playback".as_ptr(),
                        UTF8,
                    );
                    let mut id = 0u32;
                    let result = IOPMAssertionCreateWithName(kind, ASSERTION_LEVEL_ON, name, &mut id);
                    CFRelease(kind);
                    CFRelease(name);
                    if result != 0 {
                        return Err(format!("IOPMAssertionCreateWithName failed: {result}"));
                    }
                    self.assertion = Some(id);
                },
                (false, Some(id)) => {
                    unsafe { IOPMAssertionRelease(id) };
                    self.assertion = None;
                }
                _ => {}
            }
            Ok(())
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = self.set(false);
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
mod platform {
    #[derive(Default)]
    pub struct Guard;
    impl Guard {
        pub fn set(&mut self, _on: bool) -> Result<(), String> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_and_releases_sleep_prevention() {
        let awake = KeepAwake::new();
        awake.set(true).unwrap();
        awake.set(true).unwrap();
        #[cfg(target_os = "macos")]
        {
            let listed = std::process::Command::new("pmset")
                .args(["-g", "assertions"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).contains("Watch Together video playback"))
                .unwrap_or(true);
            assert!(listed, "display-sleep assertion is not registered");
        }
        awake.set(false).unwrap();
        awake.set(false).unwrap();
    }
}
