//! Room server crate for Watch Together (Milestone 2).
//!
//! Provides room lifecycle, state management, permission enforcement,
//! participant registry, and WebSocket broadcasting.

pub mod error;
pub mod participant;
pub mod permissions;
pub mod room;
pub mod server;

pub use error::RoomServerError;
pub use participant::{ParticipantRegistry, ParticipantSession};
pub use permissions::PermissionPolicy;
pub use room::{
    generate_room_code, generate_room_token, is_valid_playback_speed, Room,
    ALLOWED_PLAYBACK_SPEEDS, CROCKFORD_CHARSET,
};
pub use server::{current_time_ms, serve_connection, RoomManager, RoomServer, RoomServerHandle};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
