use crate::error::{NetworkError, NetworkResult};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IceCandidateType {
    Host,
    ServerReflexive,
    Relay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IceCandidate {
    pub candidate_type: IceCandidateType,
    pub ip: String,
    pub port: u16,
    pub protocol: String,
    pub priority: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionDescription {
    pub sdp_type: String,
    pub sdp: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum SignalingMessage {
    #[serde(rename = "signal.join")]
    Join { room_code: String, peer_id: String },
    #[serde(rename = "signal.offer")]
    Offer {
        sender_id: String,
        target_id: String,
        desc: SessionDescription,
    },
    #[serde(rename = "signal.answer")]
    Answer {
        sender_id: String,
        target_id: String,
        desc: SessionDescription,
    },
    #[serde(rename = "signal.ice_candidate")]
    IceCandidate {
        sender_id: String,
        target_id: String,
        candidate: IceCandidate,
    },
    #[serde(rename = "signal.peer_disconnected")]
    PeerDisconnected { peer_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelRequest {
    pub request_id: u64,
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TunnelResponse {
    pub request_id: u64,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

#[async_trait::async_trait]
pub trait P2pTransport: Send + Sync {
    async fn send_control(&self, msg: &str) -> NetworkResult<()>;
    async fn recv_control(&self) -> NetworkResult<String>;
    async fn send_media_request(&self, req: TunnelRequest) -> NetworkResult<()>;
    async fn recv_media_request(&self) -> NetworkResult<TunnelRequest>;
    async fn send_media_response(&self, resp: TunnelResponse) -> NetworkResult<()>;
    async fn recv_media_response(&self) -> NetworkResult<TunnelResponse>;
    async fn close(&self) -> NetworkResult<()>;
    fn is_connected(&self) -> bool;
}

pub struct MockP2pTransport {
    pub control_tx: mpsc::Sender<String>,
    pub control_rx: Mutex<mpsc::Receiver<String>>,
    pub media_req_tx: mpsc::Sender<TunnelRequest>,
    pub media_req_rx: Mutex<mpsc::Receiver<TunnelRequest>>,
    pub media_resp_tx: mpsc::Sender<TunnelResponse>,
    pub media_resp_rx: Mutex<mpsc::Receiver<TunnelResponse>>,
    pub connected: AtomicBool,
}

impl MockP2pTransport {
    pub fn create_pair() -> (Arc<Self>, Arc<Self>) {
        let (c1_tx, c1_rx) = mpsc::channel(64);
        let (c2_tx, c2_rx) = mpsc::channel(64);
        let (req1_tx, req1_rx) = mpsc::channel(64);
        let (req2_tx, req2_rx) = mpsc::channel(64);
        let (resp1_tx, resp1_rx) = mpsc::channel(64);
        let (resp2_tx, resp2_rx) = mpsc::channel(64);

        let t1 = Arc::new(Self {
            control_tx: c2_tx,
            control_rx: Mutex::new(c1_rx),
            media_req_tx: req2_tx,
            media_req_rx: Mutex::new(req1_rx),
            media_resp_tx: resp2_tx,
            media_resp_rx: Mutex::new(resp1_rx),
            connected: AtomicBool::new(true),
        });

        let t2 = Arc::new(Self {
            control_tx: c1_tx,
            control_rx: Mutex::new(c2_rx),
            media_req_tx: req1_tx,
            media_req_rx: Mutex::new(req2_rx),
            media_resp_tx: resp1_tx,
            media_resp_rx: Mutex::new(resp2_rx),
            connected: AtomicBool::new(true),
        });

        (t1, t2)
    }
}

#[async_trait::async_trait]
impl P2pTransport for MockP2pTransport {
    async fn send_control(&self, msg: &str) -> NetworkResult<()> {
        if !self.is_connected() {
            return Err(NetworkError::TransportDisconnected);
        }
        self.control_tx
            .send(msg.to_string())
            .await
            .map_err(|_| NetworkError::TransportDisconnected)
    }

    async fn recv_control(&self) -> NetworkResult<String> {
        let mut rx = self.control_rx.lock().await;
        rx.recv().await.ok_or(NetworkError::TransportDisconnected)
    }

    async fn send_media_request(&self, req: TunnelRequest) -> NetworkResult<()> {
        if !self.is_connected() {
            return Err(NetworkError::TransportDisconnected);
        }
        self.media_req_tx
            .send(req)
            .await
            .map_err(|_| NetworkError::TransportDisconnected)
    }

    async fn recv_media_request(&self) -> NetworkResult<TunnelRequest> {
        let mut rx = self.media_req_rx.lock().await;
        rx.recv().await.ok_or(NetworkError::TransportDisconnected)
    }

    async fn send_media_response(&self, resp: TunnelResponse) -> NetworkResult<()> {
        if !self.is_connected() {
            return Err(NetworkError::TransportDisconnected);
        }
        self.media_resp_tx
            .send(resp)
            .await
            .map_err(|_| NetworkError::TransportDisconnected)
    }

    async fn recv_media_response(&self) -> NetworkResult<TunnelResponse> {
        let mut rx = self.media_resp_rx.lock().await;
        rx.recv().await.ok_or(NetworkError::TransportDisconnected)
    }

    async fn close(&self) -> NetworkResult<()> {
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }
}
