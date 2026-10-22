//! Desktop integration for rooms hosted in this application's process.
use crate::{
    events::{EventSinkExt, RoomErrorPayload, RoomUpdatePayload},
    AppError, AppState,
};
use network::local_sync::{LocalSession, SessionEvent};
use serde::Serialize;
use shared_protocol::{ClientMessage, PermissionMode, RoomStateSnapshot};
use std::sync::Arc;

#[derive(Serialize)]
pub struct CreateResponse {
    pub room_id: String,
    pub room_code: String,
    pub room_token: String,
    pub server_port: u16,
}
pub async fn create(
    state: &AppState,
    name: Option<String>,
    _path: Option<String>,
    mode: Option<PermissionMode>,
) -> Result<CreateResponse, AppError> {
    let mut live = state.live_room.lock().await;
    if live.is_some() {
        return Err(AppError::SessionAlreadyActive);
    }
    let session = LocalSession::host(
        state.player.clone(),
        "0.0.0.0:8999".parse().unwrap(),
        name.unwrap_or_else(|| "Host".into()),
        mode.unwrap_or(PermissionMode::FreeControl),
    )
    .await
    .map_err(AppError::Internal)?;
    let s = session.snapshot();
    let response = CreateResponse {
        room_id: s.room_id.to_string(),
        room_code: s.room_code.to_string(),
        room_token: session.token.clone(),
        server_port: session.listen_addr.unwrap().port(),
    };
    watch(state, &session).await;
    *live = Some(session);
    Ok(response)
}
pub async fn join(
    state: &AppState,
    code: String,
    token: String,
    name: Option<String>,
    host: Option<String>,
) -> Result<crate::commands::RoomJoinResponse, AppError> {
    let mut live = state.live_room.lock().await;
    if live.is_some() {
        return Err(AppError::SessionAlreadyActive);
    }
    let session = LocalSession::join(
        state.player.clone(),
        host.as_deref().unwrap_or(""),
        &code,
        &token,
        name.unwrap_or_else(|| "Guest".into()),
    )
    .await
    .map_err(AppError::Internal)?;
    let s = session.snapshot();
    let response = crate::commands::RoomJoinResponse {
        participant_id: session.participant_id.clone(),
        room_code: s.room_code.to_string(),
        room_token: session.token.clone(),
    };
    watch(state, &session).await;
    *live = Some(session);
    Ok(response)
}
fn update(state: &AppState, session: &LocalSession) {
    let s = session.snapshot();
    let _ = state.event_sink.emit_serializable(
        "room://update",
        &RoomUpdatePayload {
            session_type: if session.is_host { "host" } else { "guest" }.into(),
            room_code: Some(s.room_code.to_string()),
            snapshot: Some(s),
        },
    );
}
async fn watch(state: &AppState, session: &Arc<LocalSession>) {
    let mut events = session.subscribe();
    update(state, session);
    let sink = state.event_sink.clone();
    let live = state.live_room.clone();
    let weak = Arc::downgrade(session);
    let task = tokio::spawn(async move {
        while let Ok(event) = events.recv().await {
            match event {
                SessionEvent::Update(s) => {
                    let Some(current) = weak.upgrade() else { break };
                    let _ = sink.emit_serializable(
                        "room://update",
                        &RoomUpdatePayload {
                            session_type: if current.is_host { "host" } else { "guest" }.into(),
                            room_code: Some(s.room_code.to_string()),
                            snapshot: Some(s),
                        },
                    );
                }
                SessionEvent::Error(message) => {
                    let _ = sink.emit_serializable(
                        "room://error",
                        &RoomErrorPayload {
                            code: "CONNECTION_ERROR".into(),
                            message,
                        },
                    );
                }
                SessionEvent::Disconnected => {
                    let mut guard = live.lock().await;
                    if guard
                        .as_ref()
                        .is_some_and(|current| weak.ptr_eq(&Arc::downgrade(current)))
                    {
                        guard.take();
                        let _ = sink.emit_serializable(
                            "room://update",
                            &RoomUpdatePayload {
                                session_type: "none".into(),
                                room_code: None,
                                snapshot: None,
                            },
                        );
                    }
                    break;
                }
            }
        }
    });
    if let Some(old) = state.room_events_task.lock().await.replace(task) {
        old.abort();
    }
}
pub async fn get_state(state: &AppState) -> Result<RoomStateSnapshot, AppError> {
    state
        .live_room
        .lock()
        .await
        .as_ref()
        .map(|s| s.snapshot())
        .ok_or(AppError::NoActiveSession)
}
pub async fn set_permissions(state: &AppState, mode: PermissionMode) -> Result<(), AppError> {
    command(
        state,
        ClientMessage::RoomSetPermissions {
            permission_mode: mode,
        },
    )
    .await
}
pub async fn kick(state: &AppState, id: String) -> Result<(), AppError> {
    command(
        state,
        ClientMessage::ParticipantKick {
            target_id: shared_protocol::ParticipantId::new(id),
        },
    )
    .await
}
async fn command(state: &AppState, command: ClientMessage) -> Result<(), AppError> {
    let session = state
        .live_room
        .lock()
        .await
        .clone()
        .ok_or(AppError::NoActiveSession)?;
    session.command(command).await.map_err(AppError::Internal)
}
