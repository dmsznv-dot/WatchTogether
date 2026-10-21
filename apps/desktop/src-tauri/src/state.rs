use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};

use media_engine::MediaPlayer;
use media_stream::MediaServerHandle;
use network::LoopbackProxyHandle;
use room_server::{RoomManager, RoomServerHandle};
use shared_protocol::messages::ClientMessage;
use shared_protocol::models::{
    ParticipantId, RoomCode, RoomId, RoomToken, VideoMetadata,
};
use sync_engine::ClientSyncCoordinator;

use crate::error::AppError;
use crate::events::{EventSink, EventSinkExt, RoomUpdatePayload};

/// Room session states for the application.
pub enum SessionState {
    /// Solo player mode: no room active.
    Solo,
    /// Host mode: running local RoomServer and MediaServer.
    Host(HostSession),
    /// Guest mode: connected to remote room server with LoopbackProxy.
    Guest(GuestSession),
}

pub struct HostSession {
    pub room_id: RoomId,
    pub room_code: RoomCode,
    pub room_token: RoomToken,
    pub host_id: ParticipantId,
    pub video: VideoMetadata,
    pub video_path: PathBuf,
    pub room_manager: Arc<RoomManager>,
    pub room_server: RoomServerHandle,
    pub media_server: Option<MediaServerHandle>,
    pub broadcast_task: Option<tokio::task::JoinHandle<()>>,
}

pub struct GuestSession {
    pub room_id: RoomId,
    pub room_code: RoomCode,
    pub room_token: RoomToken,
    pub participant_id: ParticipantId,
    pub display_name: String,
    pub loopback_proxy: Option<LoopbackProxyHandle>,
    pub ws_sender: Option<mpsc::UnboundedSender<ClientMessage>>,
    pub background_tasks: Vec<tokio::task::JoinHandle<()>>,
}

/// Global concurrent state managed by desktop backend.
pub struct AppState {
    pub player: Arc<dyn MediaPlayer>,
    pub session: Arc<RwLock<SessionState>>,
    pub sync_coordinator: Arc<Mutex<ClientSyncCoordinator>>,
    pub event_sink: Arc<dyn EventSink>,
    pub time_pos_throttler: Arc<std::sync::Mutex<TimePosThrottler>>,
}

pub struct TimePosThrottler {
    last_emitted_instant: Option<std::time::Instant>,
    min_interval: std::time::Duration,
}

impl Default for TimePosThrottler {
    fn default() -> Self {
        Self::new()
    }
}

impl TimePosThrottler {
    pub fn new() -> Self {
        Self {
            last_emitted_instant: None,
            min_interval: std::time::Duration::from_millis(250), // 250ms (4 Hz)
        }
    }

    pub fn should_emit(&mut self) -> bool {
        let now = std::time::Instant::now();
        match self.last_emitted_instant {
            Some(last) if now.duration_since(last) < self.min_interval => false,
            _ => {
                self.last_emitted_instant = Some(now);
                true
            }
        }
    }

    pub fn force_emit(&mut self) {
        self.last_emitted_instant = Some(std::time::Instant::now());
    }
}

impl AppState {
    pub fn new(player: Arc<dyn MediaPlayer>, event_sink: Arc<dyn EventSink>) -> Arc<Self> {
        let state = Arc::new(Self {
            player,
            session: Arc::new(RwLock::new(SessionState::Solo)),
            sync_coordinator: Arc::new(Mutex::new(ClientSyncCoordinator::new(0))),
            event_sink,
            time_pos_throttler: Arc::new(std::sync::Mutex::new(TimePosThrottler::new())),
        });
        state.start_player_event_listener();
        state
    }

    pub fn new_mock() -> Arc<Self> {
        let player = Arc::new(media_engine::HeadlessPlayer::new());
        let event_sink = Arc::new(crate::events::MockEventSink::new());
        Self::new(player, event_sink)
    }

    pub fn start_player_event_listener(self: &Arc<Self>) {
        let mut rx = self.player.subscribe_events();
        let this = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            while let Ok(event) = rx.recv().await {
                match event {
                    media_engine::PlayerEvent::TimePos {
                        position_ms,
                        duration_ms,
                    } => {
                        let should_emit = this.time_pos_throttler.lock().unwrap().should_emit();
                        if should_emit {
                            let payload = crate::events::TimePosPayload {
                                position_ms,
                                duration_ms,
                                position_sec: position_ms as f64 / 1000.0,
                                duration_sec: duration_ms as f64 / 1000.0,
                            };
                            let _ = this
                                .event_sink
                                .emit_serializable("player://time-pos", &payload);
                        }
                    }
                    media_engine::PlayerEvent::StateChange {
                        is_playing,
                        speed,
                        volume,
                        is_muted,
                    } => {
                        let payload = crate::events::StateChangePayload {
                            is_playing,
                            is_paused: !is_playing,
                            speed,
                            volume,
                            is_muted,
                        };
                        let _ = this
                            .event_sink
                            .emit_serializable("player://state-change", &payload);
                    }
                    media_engine::PlayerEvent::TracksChange { tracks } => {
                        let payload = crate::events::TracksChangePayload { tracks };
                        let _ = this
                            .event_sink
                            .emit_serializable("player://tracks-change", &payload);
                    }
                    _ => {}
                }
            }
        });
    }

    /// Cascades teardown of host/guest sessions, MediaServer, RoomServer, and LoopbackProxy.
    /// Resolves Reviewer M2 finding on ungraceful host drop socket/file leaks.
    pub async fn teardown(&self) -> Result<(), AppError> {
        let mut session_guard = self.session.write().await;
        match std::mem::replace(&mut *session_guard, SessionState::Solo) {
            SessionState::Host(mut host) => {
                // 1. Teardown room in RoomManager (broadcasts RoomClosed and shuts down media_server_handle in Room)
                if let Some(room_arc) = host.room_manager.get_room_by_id(&host.room_id).await {
                    let mut room = room_arc.lock().await;
                    let _ = room.teardown(&host.host_id).await;
                }

                // 2. Guaranteed safety cascade: if media_server handle was retained separately, shut it down
                if let Some(media_server) = host.media_server.take() {
                    let _ = media_server.shutdown().await;
                }

                // 3. Shut down Room WebSocket server
                let _ = host.room_server.shutdown().await;

                // 4. Abort broadcast forwarder
                if let Some(task) = host.broadcast_task {
                    task.abort();
                }

                // 5. Emit room update event
                let _ = self.event_sink.emit_serializable(
                    "room://update",
                    &RoomUpdatePayload {
                        session_type: "none".to_string(),
                        room_code: None,
                        snapshot: None,
                    },
                );
            }
            SessionState::Guest(mut guest) => {
                // 1. Send RoomLeave message if ws_sender is active
                if let Some(sender) = guest.ws_sender.as_ref() {
                    let _ = sender.send(ClientMessage::RoomLeave {
                        participant_id: guest.participant_id.clone(),
                    });
                }

                // 2. Shut down LoopbackProxy
                if let Some(proxy) = guest.loopback_proxy.take() {
                    let _ = proxy.shutdown().await;
                }

                // 3. Abort background tasks
                for task in guest.background_tasks {
                    task.abort();
                }

                // 4. Emit room update event
                let _ = self.event_sink.emit_serializable(
                    "room://update",
                    &RoomUpdatePayload {
                        session_type: "none".to_string(),
                        room_code: None,
                        snapshot: None,
                    },
                );
            }
            SessionState::Solo => {}
        }

        // Pause media player
        let _ = self.player.pause();
        Ok(())
    }
}
