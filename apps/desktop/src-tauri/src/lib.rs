//! Watch Together Desktop Application Backend (Milestone 3).

pub mod commands;
pub mod error;
pub mod events;
mod keep_awake;
mod self_host;
#[cfg(target_os = "windows")]
mod gpu_windows;
#[cfg(target_os = "windows")]
mod video_frames;
pub mod state;

use media_engine::state::PlayerState;
use media_engine::track::MediaTrack;
use shared_protocol::models::{PermissionMode, RoomStateSnapshot};
use std::sync::Arc;
use tauri::Manager;

pub use error::AppError;
pub use events::{EventSink, MockEventSink, TauriEventSink};
pub use state::{AppState, SessionState};

#[tauri::command]
async fn cmd_player_open_file(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
) -> Result<PlayerState, String> {
    commands::cmd_player_open_file(&state, path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_play(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::cmd_player_play(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_pause(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::cmd_player_pause(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_seek(
    state: tauri::State<'_, Arc<AppState>>,
    position_seconds: f64,
    exact: Option<bool>,
) -> Result<(), String> {
    commands::cmd_player_seek(&state, position_seconds, exact.unwrap_or(true))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_set_speed(
    state: tauri::State<'_, Arc<AppState>>,
    speed: f64,
) -> Result<(), String> {
    commands::cmd_player_set_speed(&state, speed)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_set_volume(
    state: tauri::State<'_, Arc<AppState>>,
    volume: f64,
) -> Result<(), String> {
    commands::cmd_player_set_volume(&state, volume)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_set_mute(
    state: tauri::State<'_, Arc<AppState>>,
    muted: bool,
) -> Result<(), String> {
    commands::cmd_player_set_mute(&state, muted)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_get_state(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<PlayerState, String> {
    commands::cmd_player_get_state(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_get_audio_tracks(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<MediaTrack>, String> {
    commands::cmd_player_get_audio_tracks(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_select_audio(
    state: tauri::State<'_, Arc<AppState>>,
    track_id: i64,
) -> Result<(), String> {
    commands::cmd_player_select_audio(&state, track_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_get_subtitle_tracks(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<MediaTrack>, String> {
    commands::cmd_player_get_subtitle_tracks(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_select_subtitle(
    state: tauri::State<'_, Arc<AppState>>,
    track_id: i64,
) -> Result<(), String> {
    commands::cmd_player_select_subtitle(&state, track_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_load_external_subtitle(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
) -> Result<MediaTrack, String> {
    commands::cmd_player_load_external_subtitle(&state, path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_load_external_audio(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
) -> Result<MediaTrack, String> {
    commands::cmd_player_load_external_audio(&state, path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_add_subtitle(
    state: tauri::State<'_, Arc<AppState>>,
    path: String,
) -> Result<(), String> {
    commands::cmd_player_add_subtitle(&state, path)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_create(
    state: tauri::State<'_, Arc<AppState>>,
    host_name: Option<String>,
    video_path: Option<String>,
    permission_mode: Option<PermissionMode>,
) -> Result<self_host::CreateResponse, String> {
    self_host::create(&state, host_name, video_path, permission_mode)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_join(
    state: tauri::State<'_, Arc<AppState>>,
    room_code: String,
    room_token: String,
    display_name: Option<String>,
    host_addr: Option<String>,
) -> Result<commands::RoomJoinResponse, String> {
    self_host::join(&state, room_code, room_token, display_name, host_addr)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_leave(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::cmd_room_leave(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_get_state(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<RoomStateSnapshot, String> {
    self_host::get_state(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_set_permissions(
    state: tauri::State<'_, Arc<AppState>>,
    permission_mode: PermissionMode,
) -> Result<(), String> {
    self_host::set_permissions(&state, permission_mode)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_kick_participant(
    state: tauri::State<'_, Arc<AppState>>,
    target_id: String,
) -> Result<(), String> {
    self_host::kick(&state, target_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_window_toggle_fullscreen(window: tauri::WebviewWindow) -> Result<bool, String> {
    let fullscreen = !window.is_fullscreen().map_err(|e| e.to_string())?;
    window
        .set_fullscreen(fullscreen)
        .map_err(|e| e.to_string())?;
    Ok(fullscreen)
}

/// Window bounds saved while the mini player (picture-in-picture) is shown.
#[derive(Default)]
struct MiniPlayer(
    tokio::sync::Mutex<Option<(tauri::PhysicalPosition<i32>, tauri::PhysicalSize<u32>)>>,
);

const MINI_SIZE: tauri::LogicalSize<f64> = tauri::LogicalSize::new(480.0, 270.0);
const MINI_MIN_SIZE: tauri::LogicalSize<f64> = tauri::LogicalSize::new(240.0, 135.0);
const MINI_MARGIN: f64 = 24.0;
/// Must match app.windows[0].minWidth / minHeight in tauri.conf.json.
const NORMAL_MIN_SIZE: tauri::LogicalSize<f64> = tauri::LogicalSize::new(640.0, 360.0);

/// Shrinks the window into a small always-on-top player in the screen corner (true),
/// or restores its previous size and position (false). Returns whether it is mini now.
#[tauri::command]
async fn cmd_window_set_mini(
    window: tauri::WebviewWindow,
    mini: tauri::State<'_, MiniPlayer>,
    enabled: bool,
) -> Result<bool, String> {
    let e = |e: tauri::Error| e.to_string();
    let mut saved = mini.0.lock().await;
    if enabled == saved.is_some() {
        return Ok(enabled);
    }
    if !enabled {
        let (position, size) = saved.take().expect("checked above");
        window.set_always_on_top(false).map_err(e)?;
        window.set_visible_on_all_workspaces(false).map_err(e)?;
        window.set_min_size(Some(NORMAL_MIN_SIZE)).map_err(e)?;
        window.set_size(size).map_err(e)?;
        window.set_position(position).map_err(e)?;
        window.set_focus().map_err(e)?;
        return Ok(false);
    }
    if window.is_fullscreen().map_err(e)? {
        window.set_fullscreen(false).map_err(e)?;
        // macOS animates out of its fullscreen Space; resizing earlier is ignored.
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }
    if window.is_maximized().map_err(e)? {
        window.unmaximize().map_err(e)?;
    }
    *saved = Some((window.outer_position().map_err(e)?, window.inner_size().map_err(e)?));
    let frame = {
        let outer = window.outer_size().map_err(e)?;
        let inner = window.inner_size().map_err(e)?;
        (outer.width.saturating_sub(inner.width), outer.height.saturating_sub(inner.height))
    };
    let monitor = match window.current_monitor().map_err(e)? {
        Some(monitor) => monitor,
        None => window.primary_monitor().map_err(e)?.ok_or("No monitor")?,
    };
    let scale = monitor.scale_factor();
    let size = MINI_SIZE.to_physical::<u32>(scale);
    let margin = (MINI_MARGIN * scale) as i32;
    let work = monitor.work_area();
    let x = work.position.x + work.size.width as i32 - (size.width + frame.0) as i32 - margin;
    let y = work.position.y + work.size.height as i32 - (size.height + frame.1) as i32 - margin;
    window.set_min_size(Some(MINI_MIN_SIZE)).map_err(e)?;
    window.set_size(size).map_err(e)?;
    window
        .set_position(tauri::PhysicalPosition::new(x, y))
        .map_err(e)?;
    window.set_always_on_top(true).map_err(e)?;
    // Stay visible when another app is in a different Space (macOS).
    window.set_visible_on_all_workspaces(true).map_err(e)?;
    Ok(true)
}

#[tauri::command]
async fn cmd_dialog_open_file(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Option<String>, String> {
    commands::cmd_dialog_open_file(&state)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_toggle_stats(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    state.player.toggle_stats().map_err(|e| e.to_string())
}

/// Fallback video path: the frame travels through IPC. Empty when nothing changed.
#[tauri::command]
async fn cmd_player_frame(
    app: tauri::AppHandle,
    width: u32,
    height: u32,
    force: Option<bool>,
) -> Result<tauri::ipc::Response, String> {
    #[cfg(target_os = "windows")]
    {
        let frames = Arc::clone(&app.state::<Arc<video_frames::WindowsFrames>>());
        let bytes = tauri::async_runtime::spawn_blocking(move || {
            frames.render_copy(width, height, force.unwrap_or(true))
        })
        .await
        .map_err(|e| e.to_string())??;
        Ok(tauri::ipc::Response::new(bytes))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, width, height, force);
        Err("Canvas renderer is Windows-only".into())
    }
}

/// Posts the shared frame buffers to the page; returns how many it will receive.
#[tauri::command]
async fn cmd_video_share_frames(window: tauri::WebviewWindow) -> Result<usize, String> {
    #[cfg(target_os = "windows")]
    {
        let frames = Arc::clone(&window.state::<Arc<video_frames::WindowsFrames>>());
        let (tx, rx) = tokio::sync::oneshot::channel();
        window
            .with_webview(move |webview| {
                let _ = tx.send(frames.share(&webview));
            })
            .map_err(|e| e.to_string())?;
        rx.await.map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        Err("Shared video buffers are Windows-only".into())
    }
}

/// Renders the next frame into shared buffer `index`. `hdrPeakNits` asks for HDR frames
/// for a monitor with that peak. Returns `{ fresh, format: "rgba8" | "rgba16f" }`.
#[tauri::command]
async fn cmd_player_frame_shared(
    app: tauri::AppHandle,
    index: usize,
    width: u32,
    height: u32,
    force: bool,
    hdr_peak_nits: Option<f32>,
) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let frames = Arc::clone(&app.state::<Arc<video_frames::WindowsFrames>>());
        let info = tauri::async_runtime::spawn_blocking(move || {
            frames.render_shared(index, width, height, force, hdr_peak_nits)
        })
        .await
        .map_err(|e| e.to_string())??;
        serde_json::to_value(info).map_err(|e| e.to_string())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, index, width, height, force, hdr_peak_nits);
        Err("Shared video buffers are Windows-only".into())
    }
}

/// HDR state of the monitor showing the window: `{ hdr, peakNits, renderer }`.
/// `hdr` is true only when Windows HDR is on there and the GPU renderer can output it.
#[tauri::command]
async fn cmd_video_display_hdr(window: tauri::WebviewWindow) -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        let frames = Arc::clone(&window.state::<Arc<video_frames::WindowsFrames>>());
        let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
        let display = gpu_windows::display_hdr(hwnd)?;
        Ok(serde_json::json!({
            "hdr": display.hdr && frames.supports_hdr(),
            "peakNits": display.peak_nits,
            "gpu": frames.supports_hdr(),
        }))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        Ok(serde_json::json!({ "hdr": false, "peakNits": 0.0, "gpu": false }))
    }
}

/// Initializes desktop application runtime backend with Tauri 2.
pub fn run() {
    tracing_subscriber::fmt::init();
    tracing::info!("Watch Together Desktop Initializing...");

    let event_sink = Arc::new(TauriEventSink::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(MiniPlayer::default())
        .setup(move |app| {
            event_sink.set_app_handle(app.handle().clone());
            let window = app
                .get_webview_window("main")
                .ok_or("Main window missing")?;
            #[cfg(target_os = "macos")]
            let player = Arc::new(media_engine::MpvPlayer::new_with_options(&[(
                "vo", "libmpv",
            )])?);
            #[cfg(target_os = "windows")]
            let player = {
                let player = Arc::new(media_engine::MpvPlayer::new_with_options(&[(
                    "vo", "libmpv",
                )])?);
                app.manage(Arc::new(video_frames::WindowsFrames::new(&player)?));
                player
            };
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            return Err("Only macOS and Windows embedding is supported".into());

            #[cfg(target_os = "macos")]
            {
                // setup and window callbacks run on AppKit's main thread.
                let surface = unsafe { player.attach_macos_surface(window.ns_view()?)? } as usize;
                let surface_player = Arc::clone(&player);
                window.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::Destroyed) {
                        unsafe {
                            surface_player.destroy_macos_surface(surface as *mut std::ffi::c_void);
                        }
                    }
                });
            }
            let player: Arc<dyn media_engine::MediaPlayer> = player;
            app.manage(AppState::new(
                player,
                Arc::clone(&event_sink) as Arc<dyn EventSink>,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            cmd_player_frame,
            cmd_player_frame_shared,
            cmd_player_toggle_stats,
            cmd_video_share_frames,
            cmd_video_display_hdr,
            cmd_player_open_file,
            cmd_player_play,
            cmd_player_pause,
            cmd_player_seek,
            cmd_player_set_speed,
            cmd_player_set_volume,
            cmd_player_set_mute,
            cmd_player_get_state,
            cmd_player_get_audio_tracks,
            cmd_player_select_audio,
            cmd_player_get_subtitle_tracks,
            cmd_player_select_subtitle,
            cmd_player_load_external_subtitle,
            cmd_player_add_subtitle,
            cmd_player_load_external_audio,
            cmd_room_create,
            cmd_room_join,
            cmd_room_leave,
            cmd_room_get_state,
            cmd_room_set_permissions,
            cmd_room_kick_participant,
            cmd_window_toggle_fullscreen,
            cmd_window_set_mini,
            cmd_dialog_open_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
