use crate::error::RoomServerError;
use crate::room::Room;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use shared_protocol::errors::ErrorCode;
use shared_protocol::messages::{ClientMessage, ServerMessage};
use shared_protocol::models::{
    ParticipantId, PermissionMode, RoomCode, RoomId, RoomToken, VideoMetadata,
};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex, RwLock};

pub fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub struct RoomManager {
    rooms: RwLock<HashMap<RoomId, Arc<Mutex<Room>>>>,
    codes: RwLock<HashMap<RoomCode, RoomId>>,
}

impl Default for RoomManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RoomManager {
    pub fn new() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
            codes: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_room(
        &self,
        host_name: &str,
        video: VideoMetadata,
        permission_mode: PermissionMode,
        max_capacity: usize,
        now_ms: u64,
    ) -> (Arc<Mutex<Room>>, RoomCode, RoomToken, ParticipantId) {
        let (room, host_id) = Room::new(host_name, video, permission_mode, max_capacity, now_ms);
        let room_id = room.room_id.clone();
        let room_code = room.room_code.clone();
        let room_token = room.room_token.clone();
        let room_arc = Arc::new(Mutex::new(room));

        self.rooms
            .write()
            .await
            .insert(room_id.clone(), Arc::clone(&room_arc));
        self.codes.write().await.insert(room_code.clone(), room_id);

        (room_arc, room_code, room_token, host_id)
    }

    pub async fn get_room_by_code(&self, code: &RoomCode) -> Option<Arc<Mutex<Room>>> {
        let codes = self.codes.read().await;
        let room_id = codes.get(code)?;
        let rooms = self.rooms.read().await;
        rooms.get(room_id).cloned()
    }

    pub async fn get_room_by_id(&self, id: &RoomId) -> Option<Arc<Mutex<Room>>> {
        let rooms = self.rooms.read().await;
        rooms.get(id).cloned()
    }

    pub async fn remove_room(&self, id: &RoomId) {
        let mut rooms = self.rooms.write().await;
        if let Some(room) = rooms.remove(id) {
            let room_lock = room.lock().await;
            self.codes.write().await.remove(&room_lock.room_code);
        }
    }
}

pub struct RoomServerHandle {
    pub local_addr: SocketAddr,
    pub manager: Arc<RoomManager>,
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub join_handle: tokio::task::JoinHandle<()>,
}

impl RoomServerHandle {
    pub async fn shutdown(mut self) -> Result<(), RoomServerError> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.join_handle
            .await
            .map_err(|e| RoomServerError::Internal(e.to_string()))?;
        Ok(())
    }
}

pub struct RoomServer;

impl RoomServer {
    pub async fn start(
        bind_addr: SocketAddr,
        manager: Arc<RoomManager>,
    ) -> Result<RoomServerHandle, RoomServerError> {
        let app = Router::new()
            .route("/ws", get(ws_handler))
            .with_state(Arc::clone(&manager));

        let listener = TcpListener::bind(bind_addr).await?;
        let local_addr = listener.local_addr()?;
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let join_handle = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap_or_default();
        });

        Ok(RoomServerHandle {
            local_addr,
            manager,
            shutdown_tx: Some(shutdown_tx),
            join_handle,
        })
    }

    pub async fn start_default(bind_addr: SocketAddr) -> Result<RoomServerHandle, RoomServerError> {
        Self::start(bind_addr, Arc::new(RoomManager::new())).await
    }
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(manager): State<Arc<RoomManager>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, manager))
}

async fn handle_socket(socket: WebSocket, manager: Arc<RoomManager>) {
    let (mut ws_sink, mut ws_stream) = socket.split();
    let (tx_direct, mut rx_direct) = mpsc::unbounded_channel::<ServerMessage>();

    // Current session context
    let mut current_room: Option<Arc<Mutex<Room>>> = None;
    let mut current_participant: Option<ParticipantId> = None;
    let mut broadcast_rx: Option<broadcast::Receiver<ServerMessage>> = None;

    loop {
        tokio::select! {
            // Direct messages to this client
            Some(msg) = rx_direct.recv() => {
                let is_kick_or_close = matches!(
                    &msg,
                    ServerMessage::RoomClosed { .. } | ServerMessage::ParticipantKicked { .. }
                );
                if let Ok(text) = serde_json::to_string(&msg) {
                    if ws_sink.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
                if is_kick_or_close {
                    break;
                }
            }

            // Broadcast messages from room
            Ok(msg) = async {
                match broadcast_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => futures_util::future::pending().await,
                }
            } => {
                let should_close = match &msg {
                    ServerMessage::RoomClosed { .. } => true,
                    ServerMessage::ParticipantKicked { participant_id, .. } => {
                        Some(participant_id) == current_participant.as_ref()
                    }
                    _ => false,
                };

                if let Ok(text) = serde_json::to_string(&msg) {
                    if ws_sink.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }

                if should_close {
                    break;
                }
            }

            // Incoming messages from client
            inbound = ws_stream.next() => {
                let msg = match inbound {
                    Some(Ok(m)) => m,
                    _ => break, // Client disconnected or error
                };

                match msg {
                    Message::Text(text) => {
                        let client_msg: Result<ClientMessage, _> = serde_json::from_str(&text);
                        let now_ms = current_time_ms();

                        match client_msg {
                            Err(e) => {
                                let _ = tx_direct.send(ServerMessage::Error {
                                    code: ErrorCode::InvalidMessage.as_str().to_string(),
                                    message: format!("Malformed message: {}", e),
                                });
                            }
                            Ok(ClientMessage::RoomCreate { host_name, video, permission_mode }) => {
                                let (room_arc, _code, _token, host_id) = manager.create_room(
                                    &host_name,
                                    video,
                                    permission_mode,
                                    5,
                                    now_ms,
                                ).await;

                                let b_rx = {
                                    let mut room = room_arc.lock().await;
                                    room.participants.set_sender(&host_id, Some(tx_direct.clone()));
                                    room.broadcast_tx.subscribe()
                                };

                                let snapshot = {
                                    let room = room_arc.lock().await;
                                    room.get_snapshot(now_ms)
                                };

                                current_room = Some(room_arc);
                                current_participant = Some(host_id);
                                broadcast_rx = Some(b_rx);

                                let _ = tx_direct.send(ServerMessage::RoomState(snapshot));
                            }
                            Ok(ClientMessage::RoomJoin { room_code, room_token, display_name, .. }) => {
                                match manager.get_room_by_code(&room_code).await {
                                    None => {
                                        let _ = tx_direct.send(ServerMessage::Error {
                                            code: ErrorCode::RoomNotFound.as_str().to_string(),
                                            message: format!("Room code not found: {}", room_code.as_str()),
                                        });
                                    }
                                    Some(room_arc) => {
                                        let (join_res, b_rx) = {
                                            let mut room = room_arc.lock().await;
                                            let res = room.join(
                                                &room_code,
                                                &room_token,
                                                &display_name,
                                                Some(tx_direct.clone()),
                                                now_ms,
                                            );
                                            let rx = room.broadcast_tx.subscribe();
                                            (res, rx)
                                        };

                                        match join_res {
                                            Err(err) => {
                                                let _ = tx_direct.send(ServerMessage::Error {
                                                    code: err.to_error_code().as_str().to_string(),
                                                    message: err.to_string(),
                                                });
                                            }
                                            Ok(guest) => {
                                                let snapshot = {
                                                    let room = room_arc.lock().await;
                                                    room.get_snapshot(now_ms)
                                                };

                                                current_room = Some(room_arc);
                                                current_participant = Some(guest.id);
                                                broadcast_rx = Some(b_rx);

                                                let _ = tx_direct.send(ServerMessage::RoomState(snapshot));
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(ClientMessage::PlaybackPlay { revision, .. }) => {
                                if let (Some(room_arc), Some(p_id)) = (&current_room, &current_participant) {
                                    let mut room = room_arc.lock().await;
                                    if let Err(err) = room.apply_play(p_id, revision, now_ms) {
                                        let _ = tx_direct.send(ServerMessage::Error {
                                            code: err.to_error_code().as_str().to_string(),
                                            message: err.to_string(),
                                        });
                                    }
                                }
                            }
                            Ok(ClientMessage::PlaybackPause { revision, position_ms, .. }) => {
                                if let (Some(room_arc), Some(p_id)) = (&current_room, &current_participant) {
                                    let mut room = room_arc.lock().await;
                                    if let Err(err) = room.apply_pause(p_id, revision, position_ms, now_ms) {
                                        let _ = tx_direct.send(ServerMessage::Error {
                                            code: err.to_error_code().as_str().to_string(),
                                            message: err.to_string(),
                                        });
                                    }
                                }
                            }
                            Ok(ClientMessage::PlaybackSeek { revision, target_position_ms, .. }) => {
                                if let (Some(room_arc), Some(p_id)) = (&current_room, &current_participant) {
                                    let mut room = room_arc.lock().await;
                                    if let Err(err) = room.apply_seek(p_id, revision, target_position_ms, now_ms) {
                                        let _ = tx_direct.send(ServerMessage::Error {
                                            code: err.to_error_code().as_str().to_string(),
                                            message: err.to_string(),
                                        });
                                    }
                                }
                            }
                            Ok(ClientMessage::PlaybackSpeed { revision, speed, .. }) => {
                                if let (Some(room_arc), Some(p_id)) = (&current_room, &current_participant) {
                                    let mut room = room_arc.lock().await;
                                    if let Err(err) = room.apply_speed(p_id, revision, speed, now_ms) {
                                        let _ = tx_direct.send(ServerMessage::Error {
                                            code: err.to_error_code().as_str().to_string(),
                                            message: err.to_string(),
                                        });
                                    }
                                }
                            }
                            Ok(ClientMessage::ParticipantStatus { buffered_position_ms, current_position_ms, is_buffering, ping_ms, .. }) => {
                                if let (Some(room_arc), Some(p_id)) = (&current_room, &current_participant) {
                                    let mut room = room_arc.lock().await;
                                    room.participants.update_status(
                                        p_id,
                                        buffered_position_ms,
                                        current_position_ms,
                                        is_buffering,
                                        ping_ms,
                                        now_ms,
                                    );
                                }
                            }
                            Ok(ClientMessage::SyncPing { ping_id, client_send_time_ms }) => {
                                let _ = tx_direct.send(ServerMessage::SyncPong {
                                    ping_id,
                                    client_send_time_ms,
                                    server_receive_time_ms: now_ms,
                                    server_send_time_ms: now_ms,
                                });
                            }
                            Ok(ClientMessage::RoomSetPermissions { permission_mode }) => {
                                if let (Some(room_arc), Some(p_id)) = (&current_room, &current_participant) {
                                    let mut room = room_arc.lock().await;
                                    if let Err(err) = room.set_permission_mode(p_id, permission_mode) {
                                        let _ = tx_direct.send(ServerMessage::Error {
                                            code: err.to_error_code().as_str().to_string(),
                                            message: err.to_string(),
                                        });
                                    }
                                }
                            }
                            Ok(ClientMessage::ParticipantKick { target_id }) => {
                                if let (Some(room_arc), Some(p_id)) = (&current_room, &current_participant) {
                                    let mut room = room_arc.lock().await;
                                    if let Err(err) = room.kick(p_id, &target_id) {
                                        let _ = tx_direct.send(ServerMessage::Error {
                                            code: err.to_error_code().as_str().to_string(),
                                            message: err.to_string(),
                                        });
                                    }
                                }
                            }
                            Ok(ClientMessage::RoomLeave { .. }) => {
                                if let (Some(room_arc), Some(p_id)) = (&current_room, &current_participant) {
                                    let mut room = room_arc.lock().await;
                                    let _ = room.leave(p_id);
                                }
                                break;
                            }
                        }
                    }
                    Message::Ping(payload) => {
                        if ws_sink.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Message::Close(_) => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    // Clean up on disconnect
    if let (Some(room_arc), Some(p_id)) = (current_room, current_participant) {
        let mut room = room_arc.lock().await;
        let _ = room.leave(&p_id);
    }
}
