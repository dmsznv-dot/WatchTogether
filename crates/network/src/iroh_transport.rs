//! Room transport over iroh: QUIC with NAT traversal (hole punching, UPnP/PCP/NAT-PMP)
//! and a relay fallback over HTTPS, so neither side configures its router.
//!
//! The host accepts connections on [`ALPN`]; every connection carries one bidirectional
//! stream of newline-delimited JSON, served by [`room_server::serve_connection`].
//! Guests find the host through a [ticket](encode_ticket): its endpoint id, relay and
//! direct addresses.
use futures_util::{future::ready, Stream, StreamExt};
use iroh::{
    endpoint::{presets, RecvStream, SendStream},
    Endpoint, EndpointAddr, EndpointId, RelayMode, RelayUrl,
};
use room_server::RoomManager;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::sync::watch;
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};

pub const ALPN: &[u8] = b"watchtogether/room/1";
/// Same limit as the WebSocket server's max message size.
const MAX_LINE: usize = 64 * 1024;
/// How long the host waits for its relay before publishing an invitation.
const ONLINE_TIMEOUT: Duration = Duration::from_secs(10);
/// Keeps invitations short enough to paste into a terminal or chat (<1 KB).
const MAX_TICKET_ADDRS: usize = 4;
/// Environment variable with comma-separated relay URLs that replace the public n0 relays.
pub const RELAY_ENV: &str = "WT_RELAY_URL";

/// Relay servers an endpoint uses when no direct path is possible.
#[derive(Clone, Debug)]
pub enum Relays {
    /// Public n0 relays plus n0 address lookup.
    Default,
    /// Own relay servers (`iroh-relay`), plus n0 address lookup.
    Custom(Vec<RelayUrl>),
    /// Direct addresses only, no relay and no address lookup (tests, LAN).
    Disabled,
}

impl Relays {
    /// Public n0 relays, unless [`RELAY_ENV`] lists custom relay URLs.
    pub fn from_env() -> Self {
        let Ok(raw) = std::env::var(RELAY_ENV) else {
            return Self::Default;
        };
        let urls: Vec<RelayUrl> = raw
            .split(',')
            .filter_map(|u| u.trim().parse().ok())
            .collect();
        if urls.is_empty() {
            Self::Default
        } else {
            Self::Custom(urls)
        }
    }
}

async fn bind(relays: &Relays, accept: bool) -> Result<Endpoint, String> {
    let builder = match relays {
        Relays::Default => Endpoint::builder(presets::N0),
        Relays::Custom(urls) => Endpoint::builder(presets::N0)
            .relay_mode(RelayMode::Custom(urls.iter().cloned().collect())),
        Relays::Disabled => Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Disabled),
    };
    let builder = if accept {
        builder.alpns(vec![ALPN.to_vec()])
    } else {
        builder
    };
    builder
        .bind()
        .await
        .map_err(|e| format!("Cannot start networking: {e}"))
}

type LineSink = FramedWrite<SendStream, LinesCodec>;

/// Frames a QUIC stream pair as newline-delimited JSON text.
fn lines(
    send: SendStream,
    recv: RecvStream,
) -> (LineSink, impl Stream<Item = String> + Unpin + Send) {
    let codec = || LinesCodec::new_with_max_length(MAX_LINE);
    let inbound = FramedRead::new(recv, codec())
        .take_while(|line| ready(line.is_ok()))
        .filter_map(|line| ready(line.ok()));
    (FramedWrite::new(send, codec()), inbound)
}

// ---------- host ----------

/// Accepts guest connections for the rooms in `manager` until [`RoomHost::shutdown`].
pub struct RoomHost {
    endpoint: Endpoint,
    shutdown: watch::Sender<bool>,
    accept: tokio::task::JoinHandle<()>,
}

impl RoomHost {
    pub async fn start(manager: Arc<RoomManager>, relays: &Relays) -> Result<Self, String> {
        let endpoint = bind(relays, true).await?;
        if !matches!(relays, Relays::Disabled)
            && tokio::time::timeout(ONLINE_TIMEOUT, endpoint.online())
                .await
                .is_err()
        {
            tracing::warn!("Relay is not reachable; the invitation lists direct addresses only");
        }
        let (shutdown, closed) = watch::channel(false);
        let accepting = endpoint.clone();
        let accept = tokio::spawn(async move {
            while let Some(incoming) = accepting.accept().await {
                let manager = Arc::clone(&manager);
                let closed = closed.clone();
                tokio::spawn(async move {
                    let mut cancel = closed.clone();
                    if *cancel.borrow() {
                        return;
                    }
                    let serve = async {
                        let conn = incoming.await.map_err(|e| e.to_string())?;
                        let (send, recv) = conn.accept_bi().await.map_err(|e| e.to_string())?;
                        let (outbound, inbound) = lines(send, recv);
                        room_server::serve_connection(outbound, inbound, manager, false, closed)
                            .await;
                        // The send stream was dropped above, which finishes it: give the
                        // guest a moment to read a final RoomClosed/kick before closing.
                        let _ = tokio::time::timeout(Duration::from_secs(2), conn.closed()).await;
                        conn.close(0u32.into(), b"done");
                        Ok::<_, String>(())
                    };
                    tokio::select! {
                        biased;
                        _ = cancel.changed() => {},
                        result = serve => if let Err(e) = result {
                            tracing::debug!("Guest connection ended: {e}");
                        },
                    }
                });
            }
        });
        Ok(Self {
            endpoint,
            shutdown,
            accept,
        })
    }

    /// Invitation part that tells guests how to reach this host.
    pub fn ticket(&self) -> String {
        encode_ticket(&self.endpoint.addr())
    }

    /// Disconnects every guest and releases the endpoint.
    pub async fn shutdown(&self) {
        let _ = self.shutdown.send(true);
        self.accept.abort();
        let _ = tokio::time::timeout(Duration::from_secs(2), self.endpoint.close()).await;
    }
}

impl Drop for RoomHost {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
        self.accept.abort();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let endpoint = self.endpoint.clone();
            runtime.spawn(async move { endpoint.close().await });
        }
    }
}

// ---------- guest ----------

/// An open connection from a guest to a host's room.
pub struct RoomLink {
    pub endpoint: Endpoint,
    pub outbound: LineSink,
    pub inbound: std::pin::Pin<Box<dyn Stream<Item = String> + Send>>,
}

impl RoomLink {
    pub async fn connect(ticket: &str, relays: &Relays, timeout: Duration) -> Result<Self, String> {
        let addr = decode_ticket(ticket)?;
        let endpoint = bind(relays, false).await?;
        let connect = async {
            let conn = endpoint
                .connect(addr, ALPN)
                .await
                .map_err(|e| format!("Cannot reach the host: {e}"))?;
            conn.open_bi()
                .await
                .map_err(|e| format!("Host rejected the connection: {e}"))
        };
        let (send, recv) = match tokio::time::timeout(timeout, connect).await {
            Ok(Ok(streams)) => streams,
            Ok(Err(e)) => {
                endpoint.close().await;
                return Err(e);
            }
            Err(_) => {
                endpoint.close().await;
                return Err(
                    "Host does not respond. Make sure the room is still open on the host computer."
                        .into(),
                );
            }
        };
        let (outbound, inbound) = lines(send, recv);
        Ok(Self {
            endpoint,
            outbound,
            inbound: Box::pin(inbound),
        })
    }
}

// ---------- tickets ----------

/// Encodes how to reach an endpoint: `base64url(json{i: id, r: [relay], a: [ip:port]})`.
/// Public addresses come first; at most [`MAX_TICKET_ADDRS`] are kept.
pub fn encode_ticket(addr: &EndpointAddr) -> String {
    let mut ips: Vec<SocketAddr> = addr.ip_addrs().copied().collect();
    ips.sort_by_key(|a| is_local(a.ip()));
    ips.truncate(MAX_TICKET_ADDRS);
    let relays: Vec<String> = addr.relay_urls().map(|u| u.to_string()).collect();
    let ips: Vec<String> = ips.iter().map(|a| a.to_string()).collect();
    let json = serde_json::json!({ "i": addr.id.to_string(), "r": relays, "a": ips });
    data_encoding::BASE64URL_NOPAD.encode(json.to_string().as_bytes())
}

pub fn decode_ticket(raw: &str) -> Result<EndpointAddr, String> {
    const DAMAGED: &str = "Host address in the invitation is damaged. Copy the whole invitation again.";
    let cleaned: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("Paste the complete invitation from the host".into());
    }
    let bytes = data_encoding::BASE64URL_NOPAD
        .decode(cleaned.as_bytes())
        .map_err(|_| DAMAGED.to_string())?;
    let v: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| DAMAGED.to_string())?;
    let id: EndpointId = v["i"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| DAMAGED.to_string())?;
    let mut addr = EndpointAddr::new(id);
    for r in v["r"].as_array().into_iter().flatten() {
        if let Some(Ok(url)) = r.as_str().map(|s| s.parse::<RelayUrl>()) {
            addr = addr.with_relay_url(url);
        }
    }
    for a in v["a"].as_array().into_iter().flatten() {
        if let Some(Ok(sa)) = a.as_str().map(|s| s.parse::<SocketAddr>()) {
            addr = addr.with_ip_addr(sa);
        }
    }
    Ok(addr)
}

fn is_local(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        std::net::IpAddr::V6(v6) => {
            v6.is_loopback()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_round_trips_and_rejects_damage() {
        let id: EndpointId = iroh::SecretKey::generate().public();
        let addr = EndpointAddr::new(id)
            .with_relay_url("https://relay.example.org./".parse().unwrap())
            .with_ip_addr("192.168.1.5:5000".parse().unwrap())
            .with_ip_addr("203.0.113.7:6000".parse().unwrap());
        let ticket = encode_ticket(&addr);
        assert!(ticket.len() < 400, "ticket too long: {}", ticket.len());
        let back = decode_ticket(&format!(" {}\n{} ", &ticket[..20], &ticket[20..])).unwrap();
        assert_eq!(back.id, id);
        assert_eq!(back.relay_urls().count(), 1);
        assert_eq!(back.ip_addrs().count(), 2);
        assert!(decode_ticket("").is_err());
        assert!(decode_ticket(&ticket[..ticket.len() / 2]).is_err());
        assert!(decode_ticket("not a ticket").is_err());
    }
}
