//! Watch Together Desktop Application Backend (Milestone 3).

pub mod commands;
pub mod error;
pub mod events;
pub mod state;

use std::sync::Arc;
use media_engine::state::PlayerState;
use media_engine::track::MediaTrack;
use shared_protocol::models::{PermissionMode, RoomStateSnapshot};

pub use error::AppError;
pub use events::{EventSink, MockEventSink, TauriEventSink};
pub use state::{AppState, SessionState};

#[tauri::command]
async fn cmd_player_open_file(state: tauri::State<'_, Arc<AppState>>, path: String) -> Result<PlayerState, String> {
    commands::cmd_player_open_file(&state, path).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_play(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::cmd_player_play(&state).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_pause(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::cmd_player_pause(&state).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_seek(state: tauri::State<'_, Arc<AppState>>, position_seconds: f64, exact: Option<bool>) -> Result<(), String> {
    commands::cmd_player_seek(&state, position_seconds, exact.unwrap_or(true)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_set_speed(state: tauri::State<'_, Arc<AppState>>, speed: f64) -> Result<(), String> {
    commands::cmd_player_set_speed(&state, speed).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_set_volume(state: tauri::State<'_, Arc<AppState>>, volume: f64) -> Result<(), String> {
    commands::cmd_player_set_volume(&state, volume).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_set_mute(state: tauri::State<'_, Arc<AppState>>, muted: bool) -> Result<(), String> {
    commands::cmd_player_set_mute(&state, muted).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_get_state(state: tauri::State<'_, Arc<AppState>>) -> Result<PlayerState, String> {
    commands::cmd_player_get_state(&state).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_get_audio_tracks(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<MediaTrack>, String> {
    commands::cmd_player_get_audio_tracks(&state).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_select_audio(state: tauri::State<'_, Arc<AppState>>, track_id: i64) -> Result<(), String> {
    commands::cmd_player_select_audio(&state, track_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_get_subtitle_tracks(state: tauri::State<'_, Arc<AppState>>) -> Result<Vec<MediaTrack>, String> {
    commands::cmd_player_get_subtitle_tracks(&state).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_select_subtitle(state: tauri::State<'_, Arc<AppState>>, track_id: i64) -> Result<(), String> {
    commands::cmd_player_select_subtitle(&state, track_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_load_external_subtitle(state: tauri::State<'_, Arc<AppState>>, path: String) -> Result<MediaTrack, String> {
    commands::cmd_player_load_external_subtitle(&state, path).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_player_add_subtitle(state: tauri::State<'_, Arc<AppState>>, path: String) -> Result<(), String> {
    commands::cmd_player_add_subtitle(&state, path).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_create(
    state: tauri::State<'_, Arc<AppState>>,
    host_name: Option<String>,
    video_path: Option<String>,
    permission_mode: Option<PermissionMode>,
) -> Result<commands::RoomCreateResponse, String> {
    commands::cmd_room_create(&state, host_name, video_path, permission_mode).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_join(
    state: tauri::State<'_, Arc<AppState>>,
    room_code: String,
    room_token: String,
    display_name: Option<String>,
) -> Result<commands::RoomJoinResponse, String> {
    commands::cmd_room_join(&state, room_code, room_token, display_name, None).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_leave(state: tauri::State<'_, Arc<AppState>>) -> Result<(), String> {
    commands::cmd_room_leave(&state).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_get_state(state: tauri::State<'_, Arc<AppState>>) -> Result<RoomStateSnapshot, String> {
    commands::cmd_room_get_state(&state).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_set_permissions(
    state: tauri::State<'_, Arc<AppState>>,
    permission_mode: PermissionMode,
) -> Result<(), String> {
    commands::cmd_room_set_permissions(&state, permission_mode).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_room_kick_participant(state: tauri::State<'_, Arc<AppState>>, target_id: String) -> Result<(), String> {
    commands::cmd_room_kick_participant(&state, target_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_window_toggle_fullscreen(state: tauri::State<'_, Arc<AppState>>) -> Result<bool, String> {
    commands::cmd_window_toggle_fullscreen(&state).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn cmd_dialog_open_file(state: tauri::State<'_, Arc<AppState>>) -> Result<Option<String>, String> {
    commands::cmd_dialog_open_file(&state).await.map_err(|e| e.to_string())
}

/// Initializes desktop application runtime backend with Tauri 2.
pub fn run() {
    tracing_subscriber::fmt::init();
    tracing::info!("Watch Together Desktop Initializing...");

    let event_sink = Arc::new(TauriEventSink::new());

    let player: Arc<dyn media_engine::MediaPlayer> = match media_engine::MpvPlayer::new() {
        Ok(p) => {
            tracing::info!("Hardware-accelerated libmpv player successfully initialized.");
            Arc::new(p)
        }
        Err(e) => {
            tracing::warn!("libmpv could not be initialized ({:?}). Falling back to HeadlessPlayer.", e);
            Arc::new(media_engine::HeadlessPlayer::new())
        }
    };

    let app_state = AppState::new(player, Arc::clone(&event_sink) as Arc<dyn EventSink>);

    let event_sink_clone = Arc::clone(&event_sink);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(app_state)
        .setup(move |app| {
            event_sink_clone.set_app_handle(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
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
            cmd_room_create,
            cmd_room_join,
            cmd_room_leave,
            cmd_room_get_state,
            cmd_room_set_permissions,
            cmd_room_kick_participant,
            cmd_window_toggle_fullscreen,
            cmd_dialog_open_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
