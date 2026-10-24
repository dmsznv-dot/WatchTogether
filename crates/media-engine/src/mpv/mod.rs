pub mod ffi;
mod gl_frames;
mod software;
pub use gl_frames::{DynamicRange, GetProcAddress, GlFrameRenderer};
pub use software::{SoftwareRenderer, MAX_HEIGHT, MAX_WIDTH};
#[cfg(target_os = "macos")]
mod macos;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int, c_void};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use tokio::sync::broadcast;

use crate::dimensions::VideoDimensions;
use crate::error::MediaEngineError;
use crate::events::PlayerEvent;
use crate::mpv::ffi::*;
use crate::state::PlayerState;
use crate::track::{MediaTrack, TrackType};
use crate::traits::MediaPlayer;
use shared_protocol::models::{PlaybackState, PlaybackStatus};

struct MpvHandleWrapper(*mut mpv_handle);
unsafe impl Send for MpvHandleWrapper {}
unsafe impl Sync for MpvHandleWrapper {}

// All access uses libmpv's thread-safe client API; returned strings are owned here.
fn read_tracks(api: &MpvApi, handle: *mut mpv_handle) -> Vec<MediaTrack> {
    let string = |name: &str| -> Option<String> {
        let name = CString::new(name).ok()?;
        let ptr = unsafe { (api.mpv_get_property_string)(handle, name.as_ptr()) };
        if ptr.is_null() {
            return None;
        }
        let value = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { (api.mpv_free)(ptr.cast()) };
        Some(value)
    };
    let integer = |name: &str| -> Option<i64> {
        let name = CString::new(name).ok()?;
        let mut value: i64 = 0;
        let code = unsafe {
            (api.mpv_get_property)(
                handle,
                name.as_ptr(),
                MPV_FORMAT_INT64,
                (&mut value as *mut i64).cast(),
            )
        };
        (code >= 0).then_some(value)
    };
    let flag = |name: &str| -> bool {
        let name = CString::new(name).unwrap();
        let mut value: c_int = 0;
        let code = unsafe {
            (api.mpv_get_property)(
                handle,
                name.as_ptr(),
                MPV_FORMAT_FLAG,
                (&mut value as *mut c_int).cast(),
            )
        };
        code >= 0 && value != 0
    };
    (0..integer("track-list/count").unwrap_or(0))
        .filter_map(|i| {
            let prefix = format!("track-list/{i}");
            let track_type = match string(&format!("{prefix}/type"))?.as_str() {
                "audio" => TrackType::Audio,
                "sub" => TrackType::Subtitle,
                _ => return None,
            };
            Some(MediaTrack {
                id: integer(&format!("{prefix}/id"))?,
                track_type,
                title: string(&format!("{prefix}/title")),
                lang: string(&format!("{prefix}/lang")),
                is_selected: flag(&format!("{prefix}/selected")),
                is_external: flag(&format!("{prefix}/external")),
                codec: string(&format!("{prefix}/codec")),
            })
        })
        .collect()
}

/// Real hardware-accelerated media player integrating `libmpv`.
pub struct MpvPlayer {
    api: Arc<MpvApi>,
    handle: MpvHandleWrapper,
    event_tx: broadcast::Sender<PlayerEvent>,
    is_terminating: Arc<AtomicBool>,
    _observer_thread: Option<JoinHandle<()>>,
    current_file: Arc<std::sync::Mutex<Option<String>>>,
}

impl MpvPlayer {
    /// Sets an mpv property at run time (e.g. `target-peak`).
    pub fn set_property_string(&self, name: &str, value: &str) -> Result<(), MediaEngineError> {
        let c_name = CString::new(name).map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let c_value = CString::new(value).map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let res = unsafe {
            (self.api.mpv_set_property_string)(self.handle.0, c_name.as_ptr(), c_value.as_ptr())
        };
        self.check_error(res, &format!("set {name}={value}"))
    }

    /// Reads any mpv property as a string (e.g. `hwdec-current`, `frame-drop-count`).
    pub fn property_string(&self, name: &str) -> Option<String> {
        let name = CString::new(name).ok()?;
        let ptr = unsafe { (self.api.mpv_get_property_string)(self.handle.0, name.as_ptr()) };
        if ptr.is_null() {
            return None;
        }
        let value = unsafe { CStr::from_ptr(ptr) }
            .to_string_lossy()
            .into_owned();
        unsafe { (self.api.mpv_free)(ptr.cast()) };
        Some(value)
    }

    /// Attempts to load `libmpv` dynamically and initialize player context.
    pub fn new() -> Result<Self, MediaEngineError> {
        Self::new_with_options(&[])
    }

    /// Options applied before initialization, including embedded output selection.
    pub fn new_with_options(options: &[(&str, &str)]) -> Result<Self, MediaEngineError> {
        let api = MpvApi::load()?;
        let handle = unsafe { (api.mpv_create)() };
        if handle.is_null() {
            return Err(MediaEngineError::LibMpvInitFailed(
                "mpv_create returned null handle".to_string(),
            ));
        }

        // Apply configuration options before mpv_initialize
        let set_opt = |name: &str, value: &str| -> Result<(), MediaEngineError> {
            let c_name =
                CString::new(name).map_err(|e| MediaEngineError::Internal(e.to_string()))?;
            let c_val =
                CString::new(value).map_err(|e| MediaEngineError::Internal(e.to_string()))?;
            let res =
                unsafe { (api.mpv_set_option_string)(handle, c_name.as_ptr(), c_val.as_ptr()) };
            if res < 0 {
                unsafe {
                    (api.mpv_destroy)(handle);
                }
                return Err(MediaEngineError::LibMpvInitFailed(format!(
                    "Cannot set {name}={value}: {res}"
                )));
            }
            Ok(())
        };

        set_opt("vo", "gpu-next")?;
        set_opt("gpu-api", "auto")?;
        set_opt("hwdec", "auto-safe")?;
        set_opt("audio-pitch-correction", "yes")?;
        set_opt("idle", "yes")?;
        set_opt("keep-open", "yes")?;
        set_opt("terminal", "no")?;
        set_opt("msg-level", "all=warn")?;
        set_opt("input-default-bindings", "no")?;
        set_opt("input-vo-keyboard", "no")?;
        set_opt("osd-level", "1")?;
        set_opt("osd-bar", "yes")?;
        set_opt("osd-font-size", "28")?;
        set_opt("autofit-larger", "1280x720")?;
        set_opt("title", "Watch Together")?;

        for (name, value) in options {
            set_opt(name, value)?;
        }

        let init_res = unsafe { (api.mpv_initialize)(handle) };
        if init_res < 0 {
            unsafe { (api.mpv_destroy)(handle) };
            return Err(MediaEngineError::LibMpvInitFailed(format!(
                "mpv_initialize failed with code {}",
                init_res
            )));
        }

        // Setup property observation
        let observe = |id: u64, name: &str, format: mpv_format| {
            if let Ok(c_name) = CString::new(name) {
                unsafe { (api.mpv_observe_property)(handle, id, c_name.as_ptr(), format) };
            }
        };

        observe(1, "time-pos", MPV_FORMAT_DOUBLE);
        observe(2, "pause", MPV_FORMAT_FLAG);
        observe(3, "speed", MPV_FORMAT_DOUBLE);
        observe(4, "volume", MPV_FORMAT_DOUBLE);
        observe(5, "mute", MPV_FORMAT_FLAG);
        observe(6, "duration", MPV_FORMAT_DOUBLE);
        observe(7, "width", MPV_FORMAT_INT64);
        observe(8, "height", MPV_FORMAT_INT64);
        observe(9, "track-list", MPV_FORMAT_NONE);
        observe(10, "aid", MPV_FORMAT_NONE);
        observe(11, "sid", MPV_FORMAT_NONE);

        let (event_tx, _) = broadcast::channel(256);
        let event_tx_clone = event_tx.clone();
        let api_clone = Arc::clone(&api);
        let is_terminating = Arc::new(AtomicBool::new(false));
        let is_terminating_clone = Arc::clone(&is_terminating);
        let current_file = Arc::new(std::sync::Mutex::new(None));

        let raw_handle = handle as usize;
        let observer_thread = std::thread::spawn(move || {
            let h = raw_handle as *mut mpv_handle;

            let query_and_emit_state =
                |target_h: *mut mpv_handle, tx: &broadcast::Sender<PlayerEvent>| {
                    let mut paused: c_int = 0;
                    let c_pause = CString::new("pause").unwrap();
                    let _ = unsafe {
                        (api_clone.mpv_get_property)(
                            target_h,
                            c_pause.as_ptr(),
                            MPV_FORMAT_FLAG,
                            &mut paused as *mut _ as *mut c_void,
                        )
                    };

                    let mut spd: c_double = 1.0;
                    let c_spd = CString::new("speed").unwrap();
                    let _ = unsafe {
                        (api_clone.mpv_get_property)(
                            target_h,
                            c_spd.as_ptr(),
                            MPV_FORMAT_DOUBLE,
                            &mut spd as *mut _ as *mut c_void,
                        )
                    };

                    let mut vol: c_double = 100.0;
                    let c_vol = CString::new("volume").unwrap();
                    let _ = unsafe {
                        (api_clone.mpv_get_property)(
                            target_h,
                            c_vol.as_ptr(),
                            MPV_FORMAT_DOUBLE,
                            &mut vol as *mut _ as *mut c_void,
                        )
                    };

                    let mut muted: c_int = 0;
                    let c_mute = CString::new("mute").unwrap();
                    let _ = unsafe {
                        (api_clone.mpv_get_property)(
                            target_h,
                            c_mute.as_ptr(),
                            MPV_FORMAT_FLAG,
                            &mut muted as *mut _ as *mut c_void,
                        )
                    };

                    let _ = tx.send(PlayerEvent::StateChange {
                        is_playing: paused == 0,
                        speed: spd as f32,
                        volume: (vol / 100.0).clamp(0.0, 1.0),
                        is_muted: muted != 0,
                    });
                };

            while !is_terminating_clone.load(Ordering::SeqCst) {
                let event_ptr = unsafe { (api_clone.mpv_wait_event)(h, -1.0) };
                if event_ptr.is_null() {
                    continue;
                }
                let event = unsafe { &*event_ptr };
                match event.event_id {
                    MPV_EVENT_SHUTDOWN => {
                        break;
                    }
                    MPV_EVENT_FILE_LOADED => {
                        let _ = event_tx_clone.send(PlayerEvent::TracksChange {
                            tracks: read_tracks(&api_clone, h),
                        });
                        query_and_emit_state(h, &event_tx_clone);
                    }
                    MPV_EVENT_END_FILE => {
                        let _ = event_tx_clone.send(PlayerEvent::Eof);
                    }
                    MPV_EVENT_PROPERTY_CHANGE if !event.data.is_null() => {
                        let prop = unsafe { &*(event.data as *const mpv_event_property) };
                        if !prop.name.is_null() {
                            let name = unsafe { CStr::from_ptr(prop.name) }.to_string_lossy();
                            match name.as_ref() {
                                "track-list" | "aid" | "sid" => {
                                    let _ = event_tx_clone.send(PlayerEvent::TracksChange {
                                        tracks: read_tracks(&api_clone, h),
                                    });
                                }
                                "time-pos"
                                    if prop.format == MPV_FORMAT_DOUBLE && !prop.data.is_null() =>
                                {
                                    let sec = unsafe { *(prop.data as *const c_double) };
                                    let pos_ms = (sec * 1000.0).max(0.0) as u64;
                                    // fetch duration
                                    let mut dur_sec: c_double = 0.0;
                                    let c_dur = CString::new("duration").unwrap();
                                    let dur_ms = if unsafe {
                                        (api_clone.mpv_get_property)(
                                            h,
                                            c_dur.as_ptr(),
                                            MPV_FORMAT_DOUBLE,
                                            &mut dur_sec as *mut _ as *mut c_void,
                                        )
                                    } >= 0
                                    {
                                        (dur_sec * 1000.0).max(0.0) as u64
                                    } else {
                                        0
                                    };

                                    let _ = event_tx_clone.send(PlayerEvent::TimePos {
                                        position_ms: pos_ms,
                                        duration_ms: dur_ms,
                                    });
                                }
                                "pause" | "speed" | "volume" | "mute" => {
                                    query_and_emit_state(h, &event_tx_clone);
                                }
                                "width" | "height" => {
                                    let mut w: i64 = 0;
                                    let mut h_val: i64 = 0;
                                    let c_w = CString::new("width").unwrap();
                                    let c_h = CString::new("height").unwrap();
                                    let res_w = unsafe {
                                        (api_clone.mpv_get_property)(
                                            h,
                                            c_w.as_ptr(),
                                            MPV_FORMAT_INT64,
                                            &mut w as *mut _ as *mut c_void,
                                        )
                                    };
                                    let res_h = unsafe {
                                        (api_clone.mpv_get_property)(
                                            h,
                                            c_h.as_ptr(),
                                            MPV_FORMAT_INT64,
                                            &mut h_val as *mut _ as *mut c_void,
                                        )
                                    };
                                    if res_w >= 0 && res_h >= 0 && w > 0 && h_val > 0 {
                                        let dims = VideoDimensions::new(w as u32, h_val as u32);
                                        let _ = event_tx_clone
                                            .send(PlayerEvent::DimensionsChange(dims));
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok(Self {
            api,
            handle: MpvHandleWrapper(handle),
            event_tx,
            is_terminating,
            _observer_thread: Some(observer_thread),
            current_file,
        })
    }

    fn check_error(&self, code: c_int, context: &str) -> Result<(), MediaEngineError> {
        if code >= 0 {
            Ok(())
        } else {
            let msg = unsafe {
                let ptr = (self.api.mpv_error_string)(code);
                if ptr.is_null() {
                    "unknown error".to_string()
                } else {
                    CStr::from_ptr(ptr).to_string_lossy().into_owned()
                }
            };
            Err(MediaEngineError::CommandFailed {
                command: context.to_string(),
                code,
                message: msg,
            })
        }
    }
}

impl MediaPlayer for MpvPlayer {
    fn toggle_stats(&self) -> Result<(), MediaEngineError> {
        // mpv's built-in stats.lua overlay, drawn into the rendered video frames.
        let cmd = CString::new("script-binding").unwrap();
        let arg = CString::new("stats/display-stats-toggle").unwrap();
        let mut args: [*const c_char; 3] = [cmd.as_ptr(), arg.as_ptr(), std::ptr::null()];
        let res = unsafe { (self.api.mpv_command)(self.handle.0, args.as_mut_ptr()) };
        self.check_error(res, "script-binding stats/display-stats-toggle")
    }

    fn open_file(&self, path_or_url: &str) -> Result<(), MediaEngineError> {
        *self.current_file.lock().unwrap() = Some(path_or_url.to_string());
        let cmd =
            CString::new("loadfile").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let arg1 =
            CString::new(path_or_url).map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let arg2 =
            CString::new("replace").map_err(|e| MediaEngineError::Internal(e.to_string()))?;

        let mut args: [*const c_char; 4] =
            [cmd.as_ptr(), arg1.as_ptr(), arg2.as_ptr(), std::ptr::null()];
        let res = unsafe { (self.api.mpv_command)(self.handle.0, args.as_mut_ptr()) };
        self.check_error(res, &format!("loadfile {}", path_or_url))
    }

    fn play(&self) -> Result<(), MediaEngineError> {
        let prop = CString::new("pause").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let mut val: c_int = 0; // false = playing
        let res = unsafe {
            (self.api.mpv_set_property)(
                self.handle.0,
                prop.as_ptr(),
                MPV_FORMAT_FLAG,
                &mut val as *mut _ as *mut c_void,
            )
        };
        self.check_error(res, "set pause=no")
    }

    fn pause(&self) -> Result<(), MediaEngineError> {
        let prop = CString::new("pause").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let mut val: c_int = 1; // true = paused
        let res = unsafe {
            (self.api.mpv_set_property)(
                self.handle.0,
                prop.as_ptr(),
                MPV_FORMAT_FLAG,
                &mut val as *mut _ as *mut c_void,
            )
        };
        self.check_error(res, "set pause=yes")
    }

    fn seek_ms(&self, target_ms: u64, exact: bool) -> Result<(), MediaEngineError> {
        let target_sec = target_ms as f64 / 1000.0;
        let cmd = CString::new("seek").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let arg1 = CString::new(format!("{:.3}", target_sec))
            .map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let mode = if exact { "absolute+exact" } else { "absolute" };
        let arg2 = CString::new(mode).map_err(|e| MediaEngineError::Internal(e.to_string()))?;

        let mut args: [*const c_char; 4] =
            [cmd.as_ptr(), arg1.as_ptr(), arg2.as_ptr(), std::ptr::null()];
        let res = unsafe { (self.api.mpv_command)(self.handle.0, args.as_mut_ptr()) };
        self.check_error(res, &format!("seek {}", target_sec))
    }

    fn set_speed(&self, speed: f32) -> Result<(), MediaEngineError> {
        if speed < 0.5 || speed > 2.0 {
            return Err(MediaEngineError::InvalidSpeed(speed));
        }

        let prop = CString::new("speed").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let mut val: c_double = speed as c_double;
        let res = unsafe {
            (self.api.mpv_set_property)(
                self.handle.0,
                prop.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut val as *mut _ as *mut c_void,
            )
        };
        self.check_error(res, &format!("set speed={}", speed))
    }

    fn set_volume(&self, volume: f64) -> Result<(), MediaEngineError> {
        let clamped = volume.clamp(0.0, 1.0);
        let mpv_vol = clamped * 100.0;
        let prop = CString::new("volume").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let mut val: c_double = mpv_vol as c_double;
        let res = unsafe {
            (self.api.mpv_set_property)(
                self.handle.0,
                prop.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut val as *mut _ as *mut c_void,
            )
        };
        self.check_error(res, &format!("set volume={}", mpv_vol))
    }

    fn set_mute(&self, muted: bool) -> Result<(), MediaEngineError> {
        let prop = CString::new("mute").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let mut val: c_int = if muted { 1 } else { 0 };
        let res = unsafe {
            (self.api.mpv_set_property)(
                self.handle.0,
                prop.as_ptr(),
                MPV_FORMAT_FLAG,
                &mut val as *mut _ as *mut c_void,
            )
        };
        self.check_error(res, &format!("set mute={}", muted))
    }

    fn audio_tracks(&self) -> Result<Vec<MediaTrack>, MediaEngineError> {
        Ok(read_tracks(&self.api, self.handle.0)
            .into_iter()
            .filter(|t| t.track_type == TrackType::Audio)
            .collect())
    }

    fn select_audio(&self, track_id: i64) -> Result<(), MediaEngineError> {
        let prop = CString::new("aid").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let mut id_val: i64 = track_id;
        let res = unsafe {
            (self.api.mpv_set_property)(
                self.handle.0,
                prop.as_ptr(),
                MPV_FORMAT_INT64,
                &mut id_val as *mut _ as *mut c_void,
            )
        };
        self.check_error(res, &format!("select audio track {}", track_id))
    }

    fn subtitle_tracks(&self) -> Result<Vec<MediaTrack>, MediaEngineError> {
        Ok(read_tracks(&self.api, self.handle.0)
            .into_iter()
            .filter(|t| t.track_type == TrackType::Subtitle)
            .collect())
    }

    fn select_subtitle(&self, track_id: i64) -> Result<(), MediaEngineError> {
        let prop = CString::new("sid").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        if track_id == -1 {
            let val = CString::new("no").unwrap();
            let res = unsafe {
                (self.api.mpv_set_property_string)(self.handle.0, prop.as_ptr(), val.as_ptr())
            };
            self.check_error(res, "disable subtitles")
        } else {
            let mut id_val: i64 = track_id;
            let res = unsafe {
                (self.api.mpv_set_property)(
                    self.handle.0,
                    prop.as_ptr(),
                    MPV_FORMAT_INT64,
                    &mut id_val as *mut _ as *mut c_void,
                )
            };
            self.check_error(res, &format!("select subtitle track {}", track_id))
        }
    }

    fn add_external_audio(
        &self,
        path: &str,
        title: Option<&str>,
        lang: Option<&str>,
    ) -> Result<i64, MediaEngineError> {
        crate::track::external_audio_extension(path)?;
        let internal = |e: std::ffi::NulError| MediaEngineError::Internal(e.to_string());
        let cmd = CString::new("audio-add").map_err(internal)?;
        let arg_path = CString::new(path).map_err(internal)?;
        let arg_flags = CString::new("select").map_err(internal)?;
        // Without a title mpv shows the file name, which is what the user picked.
        let arg_title = CString::new(title.unwrap_or("")).map_err(internal)?;
        let arg_lang = lang.and_then(|l| CString::new(l).ok());

        let mut args: Vec<*const c_char> =
            vec![cmd.as_ptr(), arg_path.as_ptr(), arg_flags.as_ptr()];
        args.push(arg_title.as_ptr());
        if let Some(ref l) = arg_lang {
            args.push(l.as_ptr());
        }
        args.push(std::ptr::null());

        let res = unsafe { (self.api.mpv_command)(self.handle.0, args.as_mut_ptr()) };
        self.check_error(res, &format!("audio-add {}", path))?;

        let aid_prop = CString::new("aid").unwrap();
        let mut aid: i64 = 1;
        unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                aid_prop.as_ptr(),
                MPV_FORMAT_INT64,
                &mut aid as *mut _ as *mut c_void,
            )
        };
        Ok(aid)
    }

    fn add_external_subtitle(
        &self,
        path: &str,
        title: Option<&str>,
        lang: Option<&str>,
    ) -> Result<i64, MediaEngineError> {
        let p = Path::new(path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if ext != "srt" && ext != "ass" {
            return Err(MediaEngineError::UnsupportedSubtitleExtension(ext));
        }

        let cmd = CString::new("sub-add").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let arg_path = CString::new(path).map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let arg_flags =
            CString::new("select").map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let arg_title = CString::new(title.unwrap_or(""))
            .map_err(|e| MediaEngineError::Internal(e.to_string()))?;
        let arg_lang = lang.and_then(|l| CString::new(l).ok());

        let mut args: Vec<*const c_char> =
            vec![cmd.as_ptr(), arg_path.as_ptr(), arg_flags.as_ptr()];
        args.push(arg_title.as_ptr());
        if let Some(ref l) = arg_lang {
            args.push(l.as_ptr());
        }
        args.push(std::ptr::null());

        let res = unsafe { (self.api.mpv_command)(self.handle.0, args.as_mut_ptr()) };
        self.check_error(res, &format!("sub-add {}", path))?;

        // Query active sid
        let sid_prop = CString::new("sid").unwrap();
        let mut sid: i64 = 1;
        unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                sid_prop.as_ptr(),
                MPV_FORMAT_INT64,
                &mut sid as *mut _ as *mut c_void,
            )
        };
        Ok(sid)
    }

    fn get_state(&self) -> Result<PlayerState, MediaEngineError> {
        let c_path = CString::new("path").unwrap();
        let path_ptr =
            unsafe { (self.api.mpv_get_property_string)(self.handle.0, c_path.as_ptr()) };
        let current_file = if !path_ptr.is_null() {
            let s = unsafe { CStr::from_ptr(path_ptr) }
                .to_string_lossy()
                .into_owned();
            unsafe { (self.api.mpv_free)(path_ptr as *mut c_void) };
            Some(s)
        } else {
            self.current_file.lock().unwrap().clone()
        };

        let c_pause = CString::new("pause").unwrap();
        let mut paused: c_int = 1;
        unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                c_pause.as_ptr(),
                MPV_FORMAT_FLAG,
                &mut paused as *mut _ as *mut c_void,
            )
        };

        let c_pos = CString::new("time-pos").unwrap();
        let mut pos_sec: c_double = 0.0;
        let pos_res = unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                c_pos.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut pos_sec as *mut _ as *mut c_void,
            )
        };
        let position_ms = if pos_res >= 0 {
            (pos_sec * 1000.0).max(0.0) as u64
        } else {
            0
        };

        let c_dur = CString::new("duration").unwrap();
        let mut dur_sec: c_double = 0.0;
        let dur_res = unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                c_dur.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut dur_sec as *mut _ as *mut c_void,
            )
        };
        let duration_ms = if dur_res >= 0 {
            (dur_sec * 1000.0).max(0.0) as u64
        } else {
            0
        };

        let c_spd = CString::new("speed").unwrap();
        let mut spd: c_double = 1.0;
        unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                c_spd.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut spd as *mut _ as *mut c_void,
            )
        };

        let c_vol = CString::new("volume").unwrap();
        let mut vol: c_double = 100.0;
        unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                c_vol.as_ptr(),
                MPV_FORMAT_DOUBLE,
                &mut vol as *mut _ as *mut c_void,
            )
        };

        let c_mute = CString::new("mute").unwrap();
        let mut muted: c_int = 0;
        unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                c_mute.as_ptr(),
                MPV_FORMAT_FLAG,
                &mut muted as *mut _ as *mut c_void,
            )
        };

        Ok(PlayerState {
            current_file,
            is_playing: paused == 0,
            position_ms,
            duration_ms,
            speed: spd as f32,
            volume: (vol / 100.0).clamp(0.0, 1.0),
            is_muted: muted != 0,
        })
    }

    fn toggle_fullscreen(&self) -> Result<bool, MediaEngineError> {
        let cmd = CString::new("cycle").unwrap();
        let arg = CString::new("fullscreen").unwrap();
        let mut args: [*const c_char; 3] = [cmd.as_ptr(), arg.as_ptr(), std::ptr::null()];
        let _ = unsafe { (self.api.mpv_command)(self.handle.0, args.as_mut_ptr()) };

        let prop = CString::new("fullscreen").unwrap();
        let mut fs: c_int = 0;
        let _ = unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                prop.as_ptr(),
                MPV_FORMAT_FLAG,
                &mut fs as *mut _ as *mut c_void,
            )
        };
        Ok(fs != 0)
    }

    fn playback_state(&self) -> Result<PlaybackState, MediaEngineError> {
        let state = self.get_state()?;
        Ok(PlaybackState {
            revision: 0,
            status: if state.is_playing {
                PlaybackStatus::Playing
            } else {
                PlaybackStatus::Paused
            },
            position_ms: state.position_ms,
            speed: state.speed,
            updated_at_server_ms: 0,
            duration_ms: state.duration_ms,
        })
    }

    fn dimensions(&self) -> Result<Option<VideoDimensions>, MediaEngineError> {
        let mut w: i64 = 0;
        let mut h: i64 = 0;
        let c_w = CString::new("width").unwrap();
        let c_h = CString::new("height").unwrap();
        let res_w = unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                c_w.as_ptr(),
                MPV_FORMAT_INT64,
                &mut w as *mut _ as *mut c_void,
            )
        };
        let res_h = unsafe {
            (self.api.mpv_get_property)(
                self.handle.0,
                c_h.as_ptr(),
                MPV_FORMAT_INT64,
                &mut h as *mut _ as *mut c_void,
            )
        };

        if res_w >= 0 && res_h >= 0 && w > 0 && h > 0 {
            Ok(Some(VideoDimensions::new(w as u32, h as u32)))
        } else {
            Ok(None)
        }
    }

    fn subscribe_events(&self) -> broadcast::Receiver<PlayerEvent> {
        self.event_tx.subscribe()
    }
}

impl Drop for MpvPlayer {
    fn drop(&mut self) {
        self.is_terminating.store(true, Ordering::SeqCst);
        // The observer must leave mpv_wait_event before its handle is freed.
        unsafe {
            (self.api.mpv_wakeup)(self.handle.0);
        }
        if let Some(thread) = self._observer_thread.take() {
            let _ = thread.join();
        }
        unsafe {
            (self.api.mpv_terminate_destroy)(self.handle.0);
        }
    }
}
