use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerState {
    pub current_file: Option<String>,
    pub is_playing: bool,
    pub position_ms: u64,
    pub duration_ms: u64,
    pub speed: f32,
    pub volume: f64,
    pub is_muted: bool,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            current_file: None,
            is_playing: false,
            position_ms: 0,
            duration_ms: 0,
            speed: 1.0,
            volume: 1.0,
            is_muted: false,
        }
    }
}
