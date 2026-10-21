use crate::protocol::*;
use crate::room::RoomRegistry;
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

pub fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub async fn handle_signaling_socket(socket: WebSocket, registry: RoomRegistry) {
    let (mut ws_sink, mut ws_stream) = socket.split();
    let (tx_out, mut rx_out) = mpsc::unbounded_channel::<ServerSignal>();

    let mut joined_room_code: Option<String> = None;
    let mut joined_peer_id: Option<String> = None;

    loop {
        tokio::select! {
            Some(signal) = rx_out.recv() => {
                let is_closed = matches!(&signal, ServerSignal::RoomClosed { .. });
                if let Ok(text) = serde_json::to_string(&signal) {
                    if ws_sink.send(Message::Text(text)).await.is_err() {
                        break;
                    }
                }
                if is_closed {
                    break;
                }
            }

            inbound = ws_stream.next() => {
                let msg = match inbound {
                    Some(Ok(m)) => m,
                    _ => break,
                };

                match msg {
                    Message::Text(text) => {
                        let parsed: Result<ClientSignal, _> = serde_json::from_str(&text);
                        let now_ms = current_time_ms();

                        match parsed {
                            Err(e) => {
                                let _ = tx_out.send(ServerSignal::Error {
                                    code: "INVALID_MESSAGE".into(),
                                    message: format!("Malformed message: {}", e),
                                });
                            }
                            Ok(ClientSignal::Join { room_code, peer_id, room_token, role }) => {
                                let effective_role = role.unwrap_or(PeerRole::Guest);
                                match registry.register_or_join(
                                    room_code.clone(),
                                    room_token,
                                    peer_id.clone(),
                                    effective_role,
                                    tx_out.clone(),
                                    now_ms,
                                ).await {
                                    Err(e) => {
                                        let _ = tx_out.send(ServerSignal::Error {
                                            code: e.error_code().into(),
                                            message: e.to_string(),
                                        });
                                    }
                                    Ok(existing_peers) => {
                                        joined_room_code = Some(room_code);
                                        joined_peer_id = Some(peer_id);

                                        // Inform newly joined peer of all existing peers
                                        for peer in existing_peers {
                                            let _ = tx_out.send(ServerSignal::PeerJoined {
                                                peer_id: peer,
                                                role: PeerRole::Host, // existing peer
                                            });
                                        }
                                    }
                                }
                            }
                            Ok(ClientSignal::Offer { sender_id, target_id, desc }) => {
                                match &joined_room_code {
                                    None => {
                                        let _ = tx_out.send(ServerSignal::Error {
                                            code: "PARTICIPANT_NOT_FOUND".into(),
                                            message: "Not joined in a room".into(),
                                        });
                                    }
                                    Some(room_code) => {
                                        let signal = ServerSignal::Offer {
                                            sender_id: sender_id.clone(),
                                            target_id: target_id.clone(),
                                            desc,
                                        };
                                        if let Err(e) = registry.forward_signal(room_code, &sender_id, &target_id, signal).await {
                                            let _ = tx_out.send(ServerSignal::Error {
                                                code: e.error_code().into(),
                                                message: e.to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                            Ok(ClientSignal::Answer { sender_id, target_id, desc }) => {
                                match &joined_room_code {
                                    None => {
                                        let _ = tx_out.send(ServerSignal::Error {
                                            code: "PARTICIPANT_NOT_FOUND".into(),
                                            message: "Not joined in a room".into(),
                                        });
                                    }
                                    Some(room_code) => {
                                        let signal = ServerSignal::Answer {
                                            sender_id: sender_id.clone(),
                                            target_id: target_id.clone(),
                                            desc,
                                        };
                                        if let Err(e) = registry.forward_signal(room_code, &sender_id, &target_id, signal).await {
                                            let _ = tx_out.send(ServerSignal::Error {
                                                code: e.error_code().into(),
                                                message: e.to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                            Ok(ClientSignal::IceCandidate { sender_id, target_id, candidate }) => {
                                match &joined_room_code {
                                    None => {
                                        let _ = tx_out.send(ServerSignal::Error {
                                            code: "PARTICIPANT_NOT_FOUND".into(),
                                            message: "Not joined in a room".into(),
                                        });
                                    }
                                    Some(room_code) => {
                                        let signal = ServerSignal::IceCandidate {
                                            sender_id: sender_id.clone(),
                                            target_id: target_id.clone(),
                                            candidate,
                                        };
                                        if let Err(e) = registry.forward_signal(room_code, &sender_id, &target_id, signal).await {
                                            let _ = tx_out.send(ServerSignal::Error {
                                                code: e.error_code().into(),
                                                message: e.to_string(),
                                            });
                                        }
                                    }
                                }
                            }
                            Ok(ClientSignal::PeerDisconnected { .. }) => {
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

    if let (Some(room_code), Some(peer_id)) = (joined_room_code, joined_peer_id) {
        registry.remove_peer(&room_code, &peer_id).await;
    }
}
