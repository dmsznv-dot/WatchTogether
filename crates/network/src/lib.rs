//! Network crate for Watch Together.
//!
//! Provides STUN NAT traversal (RFC 5389), configurable TURN relay fallback (RFC 5766),
//! P2P encrypted transport, local loopback HTTP bridge for libmpv, and
//! auto-reconnection state management.

pub mod error;
pub mod iroh_transport;
pub mod local_sync;
pub mod p2p;
pub mod proxy;
pub mod reconnect;
pub mod stun;
pub mod turn;

pub use error::{NetworkError, NetworkResult};
pub use p2p::{
    IceCandidate, IceCandidateType, MockP2pTransport, P2pTransport, SessionDescription,
    SignalingMessage, TunnelRequest, TunnelResponse,
};
pub use proxy::{LoopbackProxy, LoopbackProxyConfig, LoopbackProxyHandle};
pub use reconnect::{ConnectionState, ReconnectionConfig, ReconnectionStateMachine};
pub use stun::{
    compute_ice_candidate_priority, parse_mapped_address, parse_xor_mapped_address,
    resolve_server_reflexive_candidate, StunConfig, StunMessage, ATTR_MAPPED_ADDRESS,
    ATTR_XOR_MAPPED_ADDRESS, BINDING_REQUEST, BINDING_RESPONSE_SUCCESS, DEFAULT_STUN_SERVER,
    DEFAULT_STUN_TIMEOUT, STUN_MAGIC_COOKIE,
};
pub use turn::TurnConfig;

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
