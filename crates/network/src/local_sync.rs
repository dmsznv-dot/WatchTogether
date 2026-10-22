//! Self-hosted rooms with independent local files. No media bytes are served.
use futures_util::{SinkExt, StreamExt};
use media_engine::MediaPlayer;
use room_server::{RoomManager, RoomServer, RoomServerHandle};
use shared_protocol::{
    ClientMessage, PermissionMode, PlaybackState, PlaybackStatus, RoomCode, RoomStateSnapshot,
    RoomToken, ServerMessage, VideoMetadata,
};
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{connect_async, tungstenite::Message};

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
#[derive(Clone, Debug)]
pub enum SessionEvent {
    Update(RoomStateSnapshot),
    Error(String),
    Disconnected,
}
pub struct LocalSession {
    pub is_host: bool,
    pub endpoint: String,
    pub listen_addr: Option<SocketAddr>,
    pub token: String,
    pub participant_id: String,
    snapshot: Arc<Mutex<RoomStateSnapshot>>,
    player: Arc<dyn MediaPlayer>,
    manager: Option<Arc<RoomManager>>,
    server: Mutex<Option<RoomServerHandle>>,
    commands: Option<mpsc::Sender<ClientMessage>>,
    tasks: Mutex<Vec<tokio::task::JoinHandle<()>>>,
    connected: Arc<AtomicBool>,
    events: broadcast::Sender<SessionEvent>,
}
impl LocalSession {
    pub fn snapshot(&self) -> RoomStateSnapshot {
        self.snapshot.lock().unwrap().clone()
    }
    pub fn room_code(&self) -> String {
        self.snapshot().room_code.to_string()
    }
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }
    pub fn subscribe(&self) -> broadcast::Receiver<SessionEvent> {
        self.events.subscribe()
    }

    pub async fn host(
        player: Arc<dyn MediaPlayer>,
        addr: SocketAddr,
        name: String,
        mode: PermissionMode,
    ) -> Result<Arc<Self>, String> {
        let p = player.get_state().map_err(|e| e.to_string())?;
        let file = p
            .current_file
            .as_ref()
            .ok_or("Open your local movie first")?;
        if p.duration_ms == 0 {
            return Err("Wait until the video finishes loading".into());
        }
        let video = VideoMetadata {
            filename: file.rsplit(['/', '\\']).next().unwrap_or(file).into(),
            duration_ms: p.duration_ms,
            file_size_bytes: std::fs::metadata(file).map(|m| m.len()).unwrap_or(0),
            mime_type: "application/octet-stream".into(),
            width: None,
            height: None,
        };
        let manager = Arc::new(RoomManager::new());
        let (room, code, token, _) = manager.create_room(&name, video, mode, 5, now()).await;
        let mut rx = {
            let mut r = room.lock().await;
            r.playback.position_ms = p.position_ms;
            r.playback.speed = p.speed;
            r.playback.status = if p.is_playing {
                PlaybackStatus::Playing
            } else {
                PlaybackStatus::Paused
            };
            r.broadcast_tx.subscribe()
        };
        let server = RoomServer::start_existing(addr, manager.clone())
            .await
            .map_err(|e| format!("Cannot start room server on {addr}: {e}"))?;
        let listen = server.local_addr;
        let host = if listen.ip().is_unspecified() {
            "127.0.0.1".into()
        } else {
            listen.ip().to_string()
        };
        let endpoint = format!(
            "ws://{}:{}/ws",
            if host.contains(':') {
                format!("[{host}]")
            } else {
                host
            },
            listen.port()
        );
        let initial = room.lock().await.get_snapshot(now());
        let (events, _) = broadcast::channel(64);
        let session = Arc::new(Self {
            is_host: true,
            endpoint,
            listen_addr: Some(listen),
            token: token.to_string(),
            participant_id: initial.host_id.to_string(),
            snapshot: Arc::new(Mutex::new(initial)),
            player: player.clone(),
            manager: Some(manager),
            server: Mutex::new(Some(server)),
            commands: None,
            tasks: Mutex::new(Vec::new()),
            connected: Arc::new(AtomicBool::new(true)),
            events,
        });
        let shared = session.snapshot.clone();
        let events = session.events.clone();
        let connected = session.connected.clone();
        let task = tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_millis(250));
            loop {
                let force = tokio::select! {
                    event=rx.recv()=>match event {Ok(ServerMessage::RoomClosed{..})=>break,Ok(_)=>true,Err(broadcast::error::RecvError::Lagged(_))=>true,Err(_)=>break},
                    _=tick.tick()=>false,
                };
                let s = room.lock().await.get_snapshot(now());
                if let Err(e) = synchronize(&*player, &s.playback, now(), force) {
                    let _ = events.send(SessionEvent::Error(e));
                }
                *shared.lock().unwrap() = s.clone();
                if force {
                    let _ = events.send(SessionEvent::Update(s));
                }
            }
            connected.store(false, Ordering::Release);
            let _ = events.send(SessionEvent::Disconnected);
        });
        session.tasks.lock().unwrap().push(task);
        let _ = code;
        Ok(session)
    }

    pub async fn join(
        player: Arc<dyn MediaPlayer>,
        endpoint: &str,
        code: &str,
        token: &str,
        name: String,
    ) -> Result<Arc<Self>, String> {
        let code = RoomCode::new(code.trim().to_ascii_uppercase());
        if !code.is_valid() {
            return Err("Invalid room code".into());
        }
        let token = RoomToken::new(token.trim());
        if !token.is_valid() {
            return Err("Invalid invitation token".into());
        }
        let local = player.get_state().map_err(|e| e.to_string())?;
        if local.current_file.is_none() || local.duration_ms == 0 {
            return Err("Open your own copy of the movie before joining".into());
        }
        let endpoint = normalize_endpoint(endpoint)?;
        let operation = async {
            let (mut socket, _) = connect_async(&endpoint).await.map_err(|e| {
                format!("Cannot connect to host: {e}. Check the address, TCP port and firewall.")
            })?;
            socket
                .send(Message::Text(
                    serde_json::to_string(&ClientMessage::RoomJoin {
                        room_code: code.clone(),
                        room_token: token.clone(),
                        display_name: name,
                        participant_id: None,
                    })
                    .unwrap(),
                ))
                .await
                .map_err(|e| e.to_string())?;
            let snapshot = loop {
                match socket.next().await {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ServerMessage>(&text)
                            .map_err(|e| e.to_string())?
                        {
                            ServerMessage::RoomState(s) => break s,
                            ServerMessage::Error { message, .. } => return Err(message),
                            _ => {}
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        socket
                            .send(Message::Pong(data))
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    _ => return Err("Host disconnects before completing the room handshake".into()),
                }
            };
            // Different filenames are allowed; different cuts/durations are not silently synced.
            if local.duration_ms.abs_diff(snapshot.video.duration_ms) > 1500 {
                return Err(format!(
                    "Movie duration differs: local {} s, host {} s. Open the same cut.",
                    local.duration_ms / 1000,
                    snapshot.video.duration_ms / 1000
                ));
            }
            socket
                .send(Message::Text(
                    serde_json::to_string(&ClientMessage::SessionIdentify).unwrap(),
                ))
                .await
                .map_err(|e| e.to_string())?;
            let participant_id = loop {
                match socket.next().await {
                    Some(Ok(Message::Text(text))) => {
                        if let ServerMessage::SessionIdentity { participant_id } =
                            serde_json::from_str::<ServerMessage>(&text)
                                .map_err(|e| e.to_string())?
                        {
                            break participant_id.to_string();
                        }
                    }
                    Some(Ok(Message::Ping(data))) => {
                        socket
                            .send(Message::Pong(data))
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                    _ => return Err("Host does not confirm participant identity".into()),
                }
            };
            Ok((socket, snapshot, participant_id))
        };
        let (mut socket, snapshot, participant_id) =
            tokio::time::timeout(Duration::from_secs(10), operation)
                .await
                .map_err(|_| {
                    "Host does not respond. Check the public address, TCP port forwarding or VPN."
                        .to_string()
                })??;
        let (tx, mut commands) = mpsc::channel::<ClientMessage>(32);
        let (events, _) = broadcast::channel(64);
        let session = Arc::new(Self {
            is_host: false,
            endpoint,
            listen_addr: None,
            token: token.to_string(),
            participant_id: participant_id.clone(),
            snapshot: Arc::new(Mutex::new(snapshot)),
            player: player.clone(),
            manager: None,
            server: Mutex::new(None),
            commands: Some(tx),
            tasks: Mutex::new(Vec::new()),
            connected: Arc::new(AtomicBool::new(true)),
            events,
        });
        let shared = session.snapshot.clone();
        let events = session.events.clone();
        let connected = session.connected.clone();
        let task = tokio::spawn(async move {
            let mut clock = sync_engine::ClockEstimator::default_config();
            let mut tick = tokio::time::interval(Duration::from_millis(250));
            let mut count = 0u64;
            let mut last = tokio::time::Instant::now();
            let result:Result<(),String>=async {
                loop {
                    tokio::select! {
                        command=commands.recv()=>{let Some(c)=command else{break};socket.send(Message::Text(serde_json::to_string(&c).unwrap())).await.map_err(|e|e.to_string())?;},
                        _=tick.tick()=>{
                            if last.elapsed()>Duration::from_secs(15){return Err("Connection to host lost".into())}
                            count+=1;
                            if count%4==1 {
                                for c in [ClientMessage::SyncPing{ping_id:count,client_send_time_ms:now()},ClientMessage::RoomGetState] {socket.send(Message::Text(serde_json::to_string(&c).unwrap())).await.map_err(|e|e.to_string())?;}
                            }
                            if let Some(server_now)=clock.estimate_server_time(now()) {let p=shared.lock().unwrap().playback.clone();synchronize(&*player,&p,server_now,false)?;}
                        },
                        message=socket.next()=>{
                            let Some(Ok(message))=message else{return Err("Host disconnects".into())};last=tokio::time::Instant::now();
                            match message {
                                Message::Ping(data)=>socket.send(Message::Pong(data)).await.map_err(|e|e.to_string())?,
                                Message::Close(_)=>break,
                                Message::Text(text)=>{
                                    let event=serde_json::from_str::<ServerMessage>(&text).map_err(|e|e.to_string())?;
                                    let mut force=false;
                                    match event {
                                        ServerMessage::RoomClosed{..}=>break,
                                        ServerMessage::ParticipantKicked{participant_id:id,..} if id.as_str()==participant_id=>break,
                                        ServerMessage::SessionIdentity{..}=>{},
                                        ServerMessage::SyncPong{client_send_time_ms,server_receive_time_ms,server_send_time_ms,..}=>{let _=clock.record_measurement(&sync_engine::NtpMeasurement{t0_client_send:client_send_time_ms,t1_server_recv:server_receive_time_ms,t2_server_send:server_send_time_ms,t3_client_recv:now()});},
                                        ServerMessage::Error{message,..}=>{let _=events.send(SessionEvent::Error(message));},
                                        other=>{
                                            let mut s=shared.lock().unwrap();
                                            match other {
                                                ServerMessage::RoomState(snapshot)=>{force=accept_snapshot(&mut s,snapshot);},
                                                ServerMessage::PlaybackUpdated{revision,status,position_ms,speed,updated_at_server_ms,..}=>{if revision>s.playback.revision {s.playback=PlaybackState{revision,status,position_ms,speed,updated_at_server_ms,duration_ms:s.video.duration_ms};force=true;}},
                                                ServerMessage::ParticipantJoined(p)=>{s.participants.retain(|old|old.id!=p.id);s.participants.push(p);},
                                                ServerMessage::ParticipantLeft{participant_id,..}|ServerMessage::ParticipantKicked{participant_id,..}=>s.participants.retain(|p|p.id!=participant_id),
                                                _=>{}
                                            }
                                            let _=events.send(SessionEvent::Update(s.clone()));
                                        }
                                    }
                                    if let Some(server_now)=clock.estimate_server_time(now()) {let p=shared.lock().unwrap().playback.clone();synchronize(&*player,&p,server_now,force)?;}
                                },_=>{}
                            }
                        }
                    }
                }
                Ok(())
            }.await;
            connected.store(false, Ordering::Release);
            let _ = player.pause();
            if let Err(e) = result {
                let _ = events.send(SessionEvent::Error(e));
            }
            let _ = events.send(SessionEvent::Disconnected);
        });
        session.tasks.lock().unwrap().push(task);
        Ok(session)
    }

    pub async fn command(&self, command: ClientMessage) -> Result<(), String> {
        if !self.is_connected() {
            return Err("Room disconnected".into());
        }
        if !self.is_host {
            let s = self.snapshot();
            if matches!(
                &command,
                ClientMessage::RoomSetPermissions { .. } | ClientMessage::ParticipantKick { .. }
            ) || s.permission_mode == PermissionMode::HostOnly
            {
                return Err("Only the host controls this room".into());
            }
            return self
                .commands
                .as_ref()
                .unwrap()
                .try_send(command)
                .map_err(|e| e.to_string());
        }
        let manager = self.manager.as_ref().unwrap();
        let room = manager
            .get_room_by_id(&self.snapshot().room_id)
            .await
            .ok_or("Room closed")?;
        let mut r = room.lock().await;
        let rev = r
            .playback
            .revision
            .checked_add(1)
            .ok_or("Room revision limit reached")?;
        let id = r.host_id.clone();
        let t = now();
        let result = match command {
            ClientMessage::PlaybackPlay { .. } => r.apply_play(&id, rev, t).map(|_| ()),
            ClientMessage::PlaybackPause { position_ms, .. } => {
                r.apply_pause(&id, rev, position_ms, t).map(|_| ())
            }
            ClientMessage::PlaybackSeek {
                target_position_ms, ..
            } => r.apply_seek(&id, rev, target_position_ms, t).map(|_| ()),
            ClientMessage::PlaybackSpeed { speed, .. } => {
                r.apply_speed(&id, rev, speed, t).map(|_| ())
            }
            ClientMessage::RoomSetPermissions { permission_mode } => {
                r.set_permission_mode(&id, permission_mode)
            }
            ClientMessage::ParticipantKick { target_id } => r.kick(&id, &target_id),
            _ => return Err("Unsupported room command".into()),
        };
        result.map_err(|e| e.to_string())?;
        let snapshot = r.get_snapshot(t);
        *self.snapshot.lock().unwrap() = snapshot.clone();
        let _ = r
            .broadcast_tx
            .send(ServerMessage::RoomState(snapshot.clone()));
        drop(r);
        synchronize(&*self.player, &snapshot.playback, t, true)?;
        let _ = self.events.send(SessionEvent::Update(snapshot));
        Ok(())
    }

    pub async fn close(&self) {
        if self.is_host {
            if let Some(manager) = &self.manager {
                if let Some(room) = manager.get_room_by_id(&self.snapshot().room_id).await {
                    let mut r = room.lock().await;
                    let id = r.host_id.clone();
                    let _ = r.close(&id);
                }
            }
        }
        self.connected.store(false, Ordering::Release);
        for task in self.tasks.lock().unwrap().drain(..) {
            task.abort();
        }
        let server = self.server.lock().unwrap().take();
        if let Some(server) = server {
            let _ = tokio::time::timeout(Duration::from_secs(2), server.shutdown()).await;
        }
        let _ = self.player.pause();
        let _ = self.events.send(SessionEvent::Disconnected);
    }
}
impl Drop for LocalSession {
    fn drop(&mut self) {
        for task in self.tasks.get_mut().unwrap().drain(..) {
            task.abort();
        }
        if let Some(server) = self.server.get_mut().unwrap().take() {
            server.join_handle.abort();
        }
    }
}
fn synchronize(
    player: &dyn MediaPlayer,
    state: &PlaybackState,
    server_now: u64,
    force: bool,
) -> Result<(), String> {
    let actual = player.get_state().map_err(|e| e.to_string())?;
    if actual.duration_ms == 0 {
        return Ok(());
    }
    let target = sync_engine::project_state_position(state, server_now);
    let threshold = if state.status == PlaybackStatus::Playing {
        200
    } else {
        20
    };
    if force || actual.position_ms.abs_diff(target) > threshold {
        player.seek(target, true).map_err(|e| e.to_string())?;
    }
    if (actual.speed - state.speed).abs() > 0.001 {
        player.set_speed(state.speed).map_err(|e| e.to_string())?;
    }
    let playing = state.status == PlaybackStatus::Playing
        && (state.duration_ms == 0 || target < state.duration_ms);
    if playing != actual.is_playing {
        if playing {
            player.play()
        } else {
            player.pause()
        }
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub fn normalize_endpoint(raw: &str) -> Result<String, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Enter the host address or paste a complete invitation".into());
    }
    if raw.len() > 2048 || raw.contains(['?', '#', '@']) {
        return Err("Invalid server address".into());
    }
    let endpoint = if raw.starts_with("ws://") || raw.starts_with("wss://") {
        raw.to_owned()
    } else {
        format!("ws://{raw}")
    };
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let request = endpoint
        .clone()
        .into_client_request()
        .map_err(|e| e.to_string())?;
    if request.uri().host().is_none() {
        return Err("Invalid host address".into());
    }
    let suffix = request.uri().path();
    if suffix != "/" && suffix != "/ws" {
        return Err("Server path must be /ws".into());
    }
    let authority = request.uri().authority().ok_or("Missing host")?;
    if request.uri().port_u16().is_none() && !endpoint.starts_with("wss://") {
        return Ok(format!("ws://{authority}:8999/ws"));
    }
    if suffix == "/" {
        Ok(format!("{}/ws", endpoint.trim_end_matches('/')))
    } else {
        Ok(endpoint)
    }
}

// Direct polling replies and broadcasts have independent queues. Never roll back
// a newer timeline when a previously queued polling response arrives late.
fn accept_snapshot(current: &mut RoomStateSnapshot, incoming: RoomStateSnapshot) -> bool {
    let a = &current.playback;
    let b = &incoming.playback;
    if b.revision < a.revision
        || (b.revision == a.revision && b.updated_at_server_ms < a.updated_at_server_ms)
    {
        return false;
    }
    let changed = b.revision > a.revision;
    *current = incoming;
    changed
}

#[cfg(test)]
mod snapshot_order_tests {
    use super::*;
    #[test]
    fn late_poll_cannot_resume_a_newer_pause_or_rewind_the_timeline() {
        let video = VideoMetadata {
            filename: "movie.mkv".into(),
            duration_ms: 60000,
            file_size_bytes: 1,
            mime_type: "video/mp4".into(),
            width: None,
            height: None,
        };
        let (room, _) =
            room_server::room::Room::new("Host", video, PermissionMode::FreeControl, 5, 1000);
        let mut current = room.get_snapshot(1000);
        let mut old = current.clone();
        old.playback.status = PlaybackStatus::Playing;
        current.playback.revision = 2;
        current.playback.position_ms = 20000;
        current.playback.updated_at_server_ms = 2000;
        assert!(!accept_snapshot(&mut current, old));
        assert_eq!(current.playback.status, PlaybackStatus::Paused);
        assert_eq!(current.playback.position_ms, 20000);
        let mut old_time = current.clone();
        old_time.playback.updated_at_server_ms = 1500;
        old_time.playback.position_ms = 15000;
        assert!(!accept_snapshot(&mut current, old_time));
        assert_eq!(current.playback.position_ms, 20000);
    }
}
