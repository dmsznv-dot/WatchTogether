#![allow(non_camel_case_types)]

use libloading::{Library, Symbol};
use std::os::raw::{c_char, c_double, c_int, c_void};
use std::sync::Arc;

use crate::error::MediaEngineError;

#[repr(C)]
pub struct mpv_handle {
    _unused: [u8; 0],
}

pub type mpv_format = c_int;
pub const MPV_FORMAT_NONE: mpv_format = 0;
pub const MPV_FORMAT_STRING: mpv_format = 1;
pub const MPV_FORMAT_OSD_STRING: mpv_format = 2;
pub const MPV_FORMAT_FLAG: mpv_format = 3;
pub const MPV_FORMAT_INT64: mpv_format = 4;
pub const MPV_FORMAT_DOUBLE: mpv_format = 5;
pub const MPV_FORMAT_NODE: mpv_format = 6;
pub const MPV_FORMAT_NODE_ARRAY: mpv_format = 7;
pub const MPV_FORMAT_NODE_MAP: mpv_format = 8;
pub const MPV_FORMAT_BYTE_ARRAY: mpv_format = 9;

pub type mpv_event_id = c_int;
pub const MPV_EVENT_NONE: mpv_event_id = 0;
pub const MPV_EVENT_SHUTDOWN: mpv_event_id = 1;
pub const MPV_EVENT_LOG_MESSAGE: mpv_event_id = 2;
pub const MPV_EVENT_GET_PROPERTY_REPLY: mpv_event_id = 3;
pub const MPV_EVENT_SET_PROPERTY_REPLY: mpv_event_id = 4;
pub const MPV_EVENT_COMMAND_REPLY: mpv_event_id = 5;
pub const MPV_EVENT_START_FILE: mpv_event_id = 6;
pub const MPV_EVENT_END_FILE: mpv_event_id = 7;
pub const MPV_EVENT_FILE_LOADED: mpv_event_id = 8;
pub const MPV_EVENT_IDLE: mpv_event_id = 11;
pub const MPV_EVENT_TICK: mpv_event_id = 14;
pub const MPV_EVENT_PROPERTY_CHANGE: mpv_event_id = 22;

#[repr(C)]
pub struct mpv_event_property {
    pub name: *const c_char,
    pub format: mpv_format,
    pub data: *mut c_void,
}

#[repr(C)]
pub struct mpv_event {
    pub event_id: mpv_event_id,
    pub error: c_int,
    pub reply_userdata: u64,
    pub data: *mut c_void,
}

pub type FnMpvCreate = unsafe extern "C" fn() -> *mut mpv_handle;
pub type FnMpvInitialize = unsafe extern "C" fn(*mut mpv_handle) -> c_int;
pub type FnMpvDestroy = unsafe extern "C" fn(*mut mpv_handle);
pub type FnMpvTerminateDestroy = unsafe extern "C" fn(*mut mpv_handle);
pub type FnMpvCommand = unsafe extern "C" fn(*mut mpv_handle, *mut *const c_char) -> c_int;
pub type FnMpvCommandString = unsafe extern "C" fn(*mut mpv_handle, *const c_char) -> c_int;
pub type FnMpvSetOptionString =
    unsafe extern "C" fn(*mut mpv_handle, *const c_char, *const c_char) -> c_int;
pub type FnMpvSetProperty =
    unsafe extern "C" fn(*mut mpv_handle, *const c_char, mpv_format, *mut c_void) -> c_int;
pub type FnMpvSetPropertyString =
    unsafe extern "C" fn(*mut mpv_handle, *const c_char, *const c_char) -> c_int;
pub type FnMpvGetProperty =
    unsafe extern "C" fn(*mut mpv_handle, *const c_char, mpv_format, *mut c_void) -> c_int;
pub type FnMpvGetPropertyString =
    unsafe extern "C" fn(*mut mpv_handle, *const c_char) -> *mut c_char;
pub type FnMpvFree = unsafe extern "C" fn(*mut c_void);
pub type FnMpvObserveProperty =
    unsafe extern "C" fn(*mut mpv_handle, u64, *const c_char, mpv_format) -> c_int;
pub type FnMpvWaitEvent = unsafe extern "C" fn(*mut mpv_handle, c_double) -> *mut mpv_event;
pub type FnMpvWakeup = unsafe extern "C" fn(*mut mpv_handle);

pub type FnMpvErrorString = unsafe extern "C" fn(c_int) -> *const c_char;

/// Dynamic function table holding loaded `libmpv` symbols.
pub struct MpvApi {
    _lib: Library,
    pub mpv_create: FnMpvCreate,
    pub mpv_initialize: FnMpvInitialize,
    pub mpv_destroy: FnMpvDestroy,
    pub mpv_terminate_destroy: FnMpvTerminateDestroy,
    pub mpv_command: FnMpvCommand,
    pub mpv_command_string: FnMpvCommandString,
    pub mpv_set_option_string: FnMpvSetOptionString,
    pub mpv_set_property: FnMpvSetProperty,
    pub mpv_set_property_string: FnMpvSetPropertyString,
    pub mpv_get_property: FnMpvGetProperty,
    pub mpv_get_property_string: FnMpvGetPropertyString,
    pub mpv_free: FnMpvFree,
    pub mpv_observe_property: FnMpvObserveProperty,
    pub mpv_wait_event: FnMpvWaitEvent,
    pub mpv_error_string: FnMpvErrorString,
    pub mpv_wakeup: FnMpvWakeup,
}

// Safety: libmpv symbols are safe to share across threads.
unsafe impl Send for MpvApi {}
unsafe impl Sync for MpvApi {}

impl MpvApi {
    /// Attempts to dynamically load libmpv from known platform search paths.
    pub fn load() -> Result<Arc<Self>, MediaEngineError> {
        let mut candidate_paths: Vec<String> = Vec::new();

        if let Ok(env_path) = std::env::var("MPV_PATH") {
            candidate_paths.push(env_path);
        }

        // Relative to current executable (crucial for bundled .app Frameworks and Windows portable distribution)
        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                // macOS bundle Frameworks: Contents/MacOS/../Frameworks/
                candidate_paths.push(
                    exe_dir
                        .join("../Frameworks/libmpv.2.dylib")
                        .to_string_lossy()
                        .into_owned(),
                );
                candidate_paths.push(
                    exe_dir
                        .join("../Frameworks/libmpv.dylib")
                        .to_string_lossy()
                        .into_owned(),
                );
                // Windows next to exe
                candidate_paths.push(exe_dir.join("mpv-2.dll").to_string_lossy().into_owned());
                candidate_paths.push(exe_dir.join("libmpv-2.dll").to_string_lossy().into_owned());
                // Linux next to exe
                candidate_paths.push(exe_dir.join("libmpv.so.2").to_string_lossy().into_owned());
                candidate_paths.push(exe_dir.join("libmpv.so").to_string_lossy().into_owned());
            }
        }

        // macOS standard system paths
        candidate_paths.push("@rpath/libmpv.2.dylib".to_string());
        candidate_paths.push("../Frameworks/libmpv.2.dylib".to_string());
        candidate_paths.push("/opt/homebrew/lib/libmpv.2.dylib".to_string());
        candidate_paths.push("/opt/homebrew/lib/libmpv.dylib".to_string());
        candidate_paths.push("/usr/local/lib/libmpv.2.dylib".to_string());
        candidate_paths.push("/usr/local/lib/libmpv.dylib".to_string());

        // Windows paths
        candidate_paths.push("mpv-2.dll".to_string());
        candidate_paths.push("libmpv-2.dll".to_string());

        // Linux paths
        candidate_paths.push("libmpv.so.2".to_string());
        candidate_paths.push("libmpv.so".to_string());

        let mut last_error = None;
        for path in &candidate_paths {
            match unsafe { Library::new(path) } {
                Ok(lib) => match unsafe { Self::from_library(lib) } {
                    Ok(api) => return Ok(Arc::new(api)),
                    Err(e) => {
                        last_error = Some(e);
                    }
                },
                Err(e) => {
                    last_error = Some(MediaEngineError::LibMpvNotFound(format!("{}: {}", path, e)));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            MediaEngineError::LibMpvNotFound(
                "No dynamic library candidate paths succeeded".to_string(),
            )
        }))
    }

    #[cfg(target_os = "macos")]
    pub(super) unsafe fn render_symbol(
        &self,
        name: &[u8],
    ) -> Result<*const c_void, MediaEngineError> {
        self._lib
            .get::<*const c_void>(name)
            .map(|s| *s)
            .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))
    }

    unsafe fn from_library(lib: Library) -> Result<Self, MediaEngineError> {
        unsafe {
            let mpv_create: Symbol<FnMpvCreate> = lib
                .get(b"mpv_create\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_initialize: Symbol<FnMpvInitialize> = lib
                .get(b"mpv_initialize\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_destroy: Symbol<FnMpvDestroy> = lib
                .get(b"mpv_destroy\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_terminate_destroy: Symbol<FnMpvTerminateDestroy> = lib
                .get(b"mpv_terminate_destroy\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_command: Symbol<FnMpvCommand> = lib
                .get(b"mpv_command\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_command_string: Symbol<FnMpvCommandString> =
                lib.get(b"mpv_command_string\0")
                    .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_set_option_string: Symbol<FnMpvSetOptionString> = lib
                .get(b"mpv_set_option_string\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_set_property: Symbol<FnMpvSetProperty> = lib
                .get(b"mpv_set_property\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_set_property_string: Symbol<FnMpvSetPropertyString> = lib
                .get(b"mpv_set_property_string\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_get_property: Symbol<FnMpvGetProperty> = lib
                .get(b"mpv_get_property\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_get_property_string: Symbol<FnMpvGetPropertyString> = lib
                .get(b"mpv_get_property_string\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_free: Symbol<FnMpvFree> = lib
                .get(b"mpv_free\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_observe_property: Symbol<FnMpvObserveProperty> = lib
                .get(b"mpv_observe_property\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_wait_event: Symbol<FnMpvWaitEvent> = lib
                .get(b"mpv_wait_event\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_wakeup: Symbol<FnMpvWakeup> = lib
                .get(b"mpv_wakeup\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;
            let mpv_error_string: Symbol<FnMpvErrorString> = lib
                .get(b"mpv_error_string\0")
                .map_err(|e| MediaEngineError::LibMpvInitFailed(e.to_string()))?;

            Ok(Self {
                mpv_wakeup: *mpv_wakeup,
                mpv_create: *mpv_create,
                mpv_initialize: *mpv_initialize,
                mpv_destroy: *mpv_destroy,
                mpv_terminate_destroy: *mpv_terminate_destroy,
                mpv_command: *mpv_command,
                mpv_command_string: *mpv_command_string,
                mpv_set_option_string: *mpv_set_option_string,
                mpv_set_property: *mpv_set_property,
                mpv_set_property_string: *mpv_set_property_string,
                mpv_get_property: *mpv_get_property,
                mpv_get_property_string: *mpv_get_property_string,
                mpv_free: *mpv_free,
                mpv_observe_property: *mpv_observe_property,
                mpv_wait_event: *mpv_wait_event,
                mpv_error_string: *mpv_error_string,
                _lib: lib,
            })
        }
    }
}
