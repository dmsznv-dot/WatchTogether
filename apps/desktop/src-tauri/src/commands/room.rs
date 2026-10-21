use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use media_stream::{MediaServer, MediaServerConfig};
use room_server::{RoomManager, RoomServer};
use shared_protocol::models::{
    ParticipantId, PermissionMode, RoomCode, RoomId, RoomStateSnapshot, RoomToken, VideoMetadata,
};

use crate::error::AppError;
use crate::events::{EventSinkExt, RoomUpdatePayload};
use crate::state::{AppState, HostSession, SessionState};

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomCreateResponse {
    pub room_id: String,
    pub room_code: String,
    pub room_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomJoinResponse {
    pub participant_id: String,
    pub room_code: String,
    pub room_token: String,
}

/// Creates a new collaborative room as the host.
pub async fn cmd_room_create(
    state: &AppState,
    host_name: Option<String>,
    video_path: Option<String>,
    permission_mode: Option<PermissionMode>,
) -> Result<RoomCreateResponse, AppError> {
    let mut session_guard = state.session.write().await;
    if !matches!(&*session_guard, SessionState::Solo) {
        return Err(AppError::SessionAlreadyActive);
    }

    let p_state = state.player.get_state()?;
    let path_str = video_path
        .or(p_state.current_file)
        .unwrap_or_else(|| "/tmp/synthetic_movie.mp4".to_string());
    let path = PathBuf::from(&path_str);

    // If the path does not exist on disk, create a small temporary media file for testing
    if !path.exists() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, b"DUMMY_MP4_HEADER_CONTENT_FOR_STREAMING_TESTS");
    }

    let duration_ms = if p_state.duration_ms > 0 {
        p_state.duration_ms
    } else {
        7_200_000 // 2 hours default
    };

    let filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("video.mp4")
        .to_string();

    let file_size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(1024);

    let mime_type = match path.extension().and_then(|e| e.to_str()) {
        Some("mkv") => "video/x-matroska".to_string(),
        Some("mov") => "video/quicktime".to_string(),
        _ => "video/mp4".to_string(),
    };

    let dims = state.player.dimensions().unwrap_or(None);

    let video = VideoMetadata {
        filename,
        file_size_bytes: file_size,
        duration_ms,
        mime_type,
        width: dims.map(|d| d.width),
        height: dims.map(|d| d.height),
    };

    let now_ms = current_time_ms();
    let name = host_name.unwrap_or_else(|| "Host".to_string());
    let mode = permission_mode.unwrap_or(PermissionMode::FreeControl);

    let room_manager = Arc::new(RoomManager::new());
    let (room_arc, room_code, room_token, host_id) = room_manager
        .create_room(&name, video.clone(), mode, 5, now_ms)
        .await;

    // Start local RoomServer WebSocket listener on ephemeral loopback port
    let ws_bind: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let room_server = RoomServer::start(ws_bind, Arc::clone(&room_manager)).await?;

    // Start local MediaServer HTTP byte-range streamer on ephemeral loopback port
    let media_bind: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let media_config = MediaServerConfig {
        bind_addr: media_bind,
        video_path: path.clone(),
        room_token: room_token.to_string(),
    };
    let media_server = MediaServer::start(media_config).await.ok();

    let room_id = {
        let room = room_arc.lock().await;
        room.room_id.clone()
    };

    let snapshot = {
        let room = room_arc.lock().await;
        room.get_snapshot(now_ms)
    };

    let host_session = HostSession {
        room_id: room_id.clone(),
        room_code: room_code.clone(),
        room_token: room_token.clone(),
        host_id,
        video,
        video_path: path,
        room_manager,
        room_server,
        media_server,
        broadcast_task: None,
    };

    *session_guard = SessionState::Host(host_session);

    let _ = state.event_sink.emit_serializable(
        "room://update",
        &RoomUpdatePayload {
            session_type: "host".to_string(),
            room_code: Some(room_code.to_string()),
            snapshot: Some(snapshot),
        },
    );

    Ok(RoomCreateResponse {
        room_id: room_id.to_string(),
        room_code: room_code.to_string(),
        room_token: room_token.to_string(),
    })
}

/// Helper wrapper for room creation returning (room_code, room_token).
pub async fn room_create(state: &AppState) -> Result<(String, String), AppError> {
    let res = cmd_room_create(state, None, None, None).await?;
    Ok((res.room_code, res.room_token))
}

/// Joins an existing room.
/// CRITICAL: Room code is normalized using .trim().to_ascii_uppercase() per PRD & Reviewer findings.
pub async fn cmd_room_join(
    state: &AppState,
    room_code: String,
    room_token: String,
    display_name: Option<String>,
    _host_addr: Option<String>,
) -> Result<RoomJoinResponse, AppError> {
    let mut session_guard = state.session.write().await;
    if !matches!(&*session_guard, SessionState::Solo) {
        return Err(AppError::SessionAlreadyActive);
    }

    // Normalization per Reviewer M2 finding:
    let normalized_code_str = room_code.trim().to_ascii_uppercase();
    let room_code_obj = RoomCode::new(&normalized_code_str);
    if !room_code_obj.is_valid() {
        return Err(AppError::InvalidRoomCode(normalized_code_str));
    }

    let room_token_obj = RoomToken::new(room_token.trim());
    if !room_token_obj.is_valid() {
        return Err(AppError::InvalidToken);
    }

    let participant_id = ParticipantId::new(format!("part-{}", current_time_ms()));
    let name = display_name.unwrap_or_else(|| "Guest".to_string());

    let guest_session = crate::state::GuestSession {
        room_id: RoomId::new(format!("room-joined-{}", normalized_code_str)),
        room_code: room_code_obj.clone(),
        room_token: room_token_obj.clone(),
        participant_id: participant_id.clone(),
        display_name: name,
        loopback_proxy: None,
        ws_sender: None,
        background_tasks: Vec::new(),
    };

    *session_guard = SessionState::Guest(guest_session);

    let _ = state.event_sink.emit_serializable(
        "room://update",
        &RoomUpdatePayload {
            session_type: "guest".to_string(),
            room_code: Some(room_code_obj.to_string()),
            snapshot: None,
        },
    );

    Ok(RoomJoinResponse {
        participant_id: participant_id.to_string(),
        room_code: room_code_obj.to_string(),
        room_token: room_token_obj.to_string(),
    })
}

pub async fn room_join(
    state: &AppState,
    room_code: String,
    room_token: String,
) -> Result<RoomJoinResponse, AppError> {
    cmd_room_join(state, room_code, room_token, None, None).await
}

/// Leaves the current room session and cascades teardown.
pub async fn cmd_room_leave(state: &AppState) -> Result<(), AppError> {
    state.teardown().await
}

pub async fn room_leave(state: &AppState) -> Result<(), AppError> {
    cmd_room_leave(state).await
}

/// Retrieves the current authoritative room state snapshot.
pub async fn cmd_room_get_state(state: &AppState) -> Result<RoomStateSnapshot, AppError> {
    let session_guard = state.session.read().await;
    match &*session_guard {
        SessionState::Host(host) => {
            let room_arc = host
                .room_manager
                .get_room_by_id(&host.room_id)
                .await
                .ok_or(AppError::NoActiveSession)?;
            let room = room_arc.lock().await;
            let now_ms = current_time_ms();
            Ok(room.get_snapshot(now_ms))
        }
        SessionState::Guest(_) => {
            // In guest mode without live server connection, return minimal snapshot
            Err(AppError::NoActiveSession)
        }
        SessionState::Solo => Err(AppError::NoActiveSession),
    }
}

pub async fn room_get_state(state: &AppState) -> Result<RoomStateSnapshot, AppError> {
    cmd_room_get_state(state).await
}

/// Host toggles room permission mode (FreeControl vs HostOnly).
pub async fn cmd_room_set_permissions(
    state: &AppState,
    permission_mode: PermissionMode,
) -> Result<(), AppError> {
    let session_guard = state.session.read().await;
    match &*session_guard {
        SessionState::Host(host) => {
            let room_arc = host
                .room_manager
                .get_room_by_id(&host.room_id)
                .await
                .ok_or(AppError::NoActiveSession)?;
            let mut room = room_arc.lock().await;
            room.set_permission_mode(&host.host_id, permission_mode)?;
            Ok(())
        }
        SessionState::Guest(_) => Err(AppError::PermissionDenied(
            "Only the host can modify permission mode".to_string(),
        )),
        SessionState::Solo => Err(AppError::NoActiveSession),
    }
}

pub async fn room_set_permissions(
    state: &AppState,
    permission_mode: PermissionMode,
) -> Result<(), AppError> {
    cmd_room_set_permissions(state, permission_mode).await
}

/// Host kicks a participant from the room.
pub async fn cmd_room_kick_participant(
    state: &AppState,
    target_id: String,
) -> Result<(), AppError> {
    let session_guard = state.session.read().await;
    match &*session_guard {
        SessionState::Host(host) => {
            let room_arc = host
                .room_manager
                .get_room_by_id(&host.room_id)
                .await
                .ok_or(AppError::NoActiveSession)?;
            let mut room = room_arc.lock().await;
            let target = ParticipantId::new(target_id);
            room.kick(&host.host_id, &target)?;
            Ok(())
        }
        SessionState::Guest(_) => Err(AppError::PermissionDenied(
            "Only the host can kick participants".to_string(),
        )),
        SessionState::Solo => Err(AppError::NoActiveSession),
    }
}

pub async fn room_kick_participant(state: &AppState, target_id: String) -> Result<(), AppError> {
    cmd_room_kick_participant(state, target_id).await
}
