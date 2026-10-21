//! Standalone WebRTC signaling relay service for Watch Together.

pub mod protocol;
pub mod room;
pub mod server;
pub mod session;

pub use protocol::{
    ClientSignal, IceCandidate, IceCandidateType, PeerRole, ServerSignal, SessionDescription,
    SignalError,
};
pub use room::{RoomRegistry, RoomSession, MAX_ROOM_CAPACITY};
pub use server::{SignalingServer, SignalingServerHandle};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
