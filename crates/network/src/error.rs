use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("STUN protocol error: {0}")]
    Stun(String),

    #[error("STUN request timed out after {0:?}")]
    StunTimeout(std::time::Duration),

    #[error("TURN authentication failed for user: {0}")]
    TurnAuthFailed(String),

    #[error("Signaling error: {0}")]
    Signaling(String),

    #[error("P2P transport disconnected")]
    TransportDisconnected,

    #[error("Loopback proxy error: {0}")]
    Proxy(String),

    #[error("Reconnection exhausted after {0} attempts")]
    ReconnectionExhausted(usize),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Invalid message: {0}")]
    InvalidMessage(String),
}

pub type NetworkResult<T> = Result<T, NetworkError>;
