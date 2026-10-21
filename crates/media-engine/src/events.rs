use crate::dimensions::VideoDimensions;
use crate::track::MediaTrack;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum PlayerEvent {
    #[serde(rename = "time-pos")]
    TimePos {
        position_ms: u64,
        duration_ms: u64,
    },
    #[serde(rename = "state-change")]
    StateChange {
        is_playing: bool,
        speed: f32,
        volume: f64,
        is_muted: bool,
    },
    #[serde(rename = "tracks-change")]
    TracksChange {
        tracks: Vec<MediaTrack>,
    },
    #[serde(rename = "dimensions-change")]
    DimensionsChange(VideoDimensions),
    #[serde(rename = "buffering")]
    Buffering(bool),
    #[serde(rename = "eof")]
    Eof,
    #[serde(rename = "error")]
    Error(String),
}
