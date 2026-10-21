use media_engine::track::MediaTrack;
use serde::Serialize;
use shared_protocol::models::RoomStateSnapshot;
use std::sync::{Arc, Mutex};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize)]
pub struct TimePosPayload {
    pub position_ms: u64,
    pub duration_ms: u64,
    pub position_sec: f64,
    pub duration_sec: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct StateChangePayload {
    pub is_playing: bool,
    pub is_paused: bool,
    pub speed: f32,
    pub volume: f64,
    pub is_muted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TracksChangePayload {
    pub tracks: Vec<MediaTrack>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoomUpdatePayload {
    pub session_type: String, // "none", "host", "guest"
    pub room_code: Option<String>,
    pub snapshot: Option<RoomStateSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RoomErrorPayload {
    pub code: String,
    pub message: String,
}

/// Abstract object-safe event sink interface for sending events to the frontend or test inspector.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), AppError>;
}

/// Extension trait providing serializable emission helper for any EventSink.
pub trait EventSinkExt {
    fn emit_serializable<T: Serialize>(&self, event: &str, payload: &T) -> Result<(), AppError>;
}

impl<S: ?Sized + EventSink> EventSinkExt for S {
    fn emit_serializable<T: Serialize>(&self, event: &str, payload: &T) -> Result<(), AppError> {
        let val = serde_json::to_value(payload).map_err(|e| AppError::Internal(e.to_string()))?;
        self.emit(event, val)
    }
}

/// In-memory mock event sink for unit and integration testing.
#[derive(Default, Clone)]
pub struct MockEventSink {
    events: Arc<Mutex<Vec<(String, serde_json::Value)>>>,
}

impl MockEventSink {
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn events(&self) -> Vec<(String, serde_json::Value)> {
        self.events.lock().unwrap().clone()
    }

    pub fn clear(&self) {
        self.events.lock().unwrap().clear();
    }

    pub fn has_event(&self, event_name: &str) -> bool {
        self.events
            .lock()
            .unwrap()
            .iter()
            .any(|(name, _)| name == event_name)
    }

    pub fn last_event(&self, event_name: &str) -> Option<serde_json::Value> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(name, _)| name == event_name)
            .map(|(_, val)| val.clone())
    }
}

impl EventSink for MockEventSink {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), AppError> {
        self.events
            .lock()
            .unwrap()
            .push((event.to_string(), payload));
        Ok(())
    }
}

/// Tauri event sink emitting to Webview frontends or logging when headless.
#[derive(Default)]
pub struct TauriEventSink;

impl TauriEventSink {
    pub fn new() -> Self {
        Self
    }
}

impl EventSink for TauriEventSink {
    fn emit(&self, event: &str, payload: serde_json::Value) -> Result<(), AppError> {
        tracing::debug!(event, %payload, "Emitted Tauri event");
        Ok(())
    }
}
