use std::path::Path;
use std::sync::RwLock;
use tokio::sync::broadcast;

use crate::dimensions::VideoDimensions;
use crate::error::MediaEngineError;
use crate::events::PlayerEvent;
use crate::state::PlayerState;
use crate::track::{MediaTrack, TrackType};
use crate::traits::MediaPlayer;
use shared_protocol::models::{PlaybackState, PlaybackStatus};

pub const ALLOWED_SPEEDS: [f32; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

pub fn is_valid_speed(speed: f32) -> bool {
    ALLOWED_SPEEDS.iter().any(|&s| (s - speed).abs() < 1e-4)
}

struct InnerState {
    current_file: Option<String>,
    is_playing: bool,
    position_ms: u64,
    duration_ms: u64,
    speed: f32,
    volume: f64,
    is_muted: bool,
    is_fullscreen: bool,
    audio_tracks: Vec<MediaTrack>,
    subtitle_tracks: Vec<MediaTrack>,
    dimensions: Option<VideoDimensions>,
}

/// Fully deterministic in-memory media player implementation of `MediaPlayer`.
/// Provides 100% CI headless testability without display or audio hardware.
pub struct HeadlessPlayer {
    inner: RwLock<InnerState>,
    event_tx: broadcast::Sender<PlayerEvent>,
}

impl Default for HeadlessPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl HeadlessPlayer {
    pub fn new() -> Self {
        let (event_tx, _) = broadcast::channel(128);
        let inner = InnerState {
            current_file: None,
            is_playing: false,
            position_ms: 0,
            duration_ms: 0,
            speed: 1.0,
            volume: 1.0,
            is_muted: false,
            is_fullscreen: false,
            audio_tracks: vec![
                MediaTrack {
                    id: 1,
                    track_type: TrackType::Audio,
                    title: Some("English Stereo".to_string()),
                    lang: Some("eng".to_string()),
                    is_selected: true,
                    is_external: false,
                    codec: Some("aac".to_string()),
                },
                MediaTrack {
                    id: 2,
                    track_type: TrackType::Audio,
                    title: Some("Director Commentary".to_string()),
                    lang: Some("eng".to_string()),
                    is_selected: false,
                    is_external: false,
                    codec: Some("aac".to_string()),
                },
            ],
            subtitle_tracks: vec![MediaTrack {
                id: 1,
                track_type: TrackType::Subtitle,
                title: Some("English Full".to_string()),
                lang: Some("eng".to_string()),
                is_selected: false,
                is_external: false,
                codec: Some("subrip".to_string()),
            }],
            dimensions: Some(VideoDimensions::new(1920, 1080)),
        };

        Self {
            inner: RwLock::new(inner),
            event_tx,
        }
    }

    pub fn with_duration(self, duration_ms: u64) -> Self {
        {
            let mut guard = self.inner.write().unwrap();
            guard.duration_ms = duration_ms;
        }
        self
    }

    pub fn open_file_with_duration(
        &self,
        path_or_url: &str,
        duration_ms: u64,
    ) -> Result<(), MediaEngineError> {
        let (tracks, dims) = {
            let mut state = self.inner.write().unwrap();
            state.current_file = Some(path_or_url.to_string());
            state.duration_ms = duration_ms;
            state.position_ms = 0;
            state.is_playing = false;
            (state.audio_tracks.clone(), state.dimensions)
        };

        let _ = self.event_tx.send(PlayerEvent::TimePos {
            position_ms: 0,
            duration_ms,
        });
        let _ = self.event_tx.send(PlayerEvent::StateChange {
            is_playing: false,
            speed: 1.0,
            volume: 1.0,
            is_muted: false,
        });
        let _ = self.event_tx.send(PlayerEvent::TracksChange { tracks });
        if let Some(d) = dims {
            let _ = self.event_tx.send(PlayerEvent::DimensionsChange(d));
        }

        Ok(())
    }

    pub fn set_dimensions(&self, dimensions: Option<VideoDimensions>) {
        {
            let mut state = self.inner.write().unwrap();
            state.dimensions = dimensions;
        }
        if let Some(d) = dimensions {
            let _ = self.event_tx.send(PlayerEvent::DimensionsChange(d));
        }
    }
}

impl MediaPlayer for HeadlessPlayer {
    fn open_file(&self, path_or_url: &str) -> Result<(), MediaEngineError> {
        // Default to 60,000ms if not explicitly set
        let existing_duration = {
            let state = self.inner.read().unwrap();
            state.duration_ms
        };
        let duration = if existing_duration > 0 {
            existing_duration
        } else {
            60_000
        };
        self.open_file_with_duration(path_or_url, duration)
    }

    fn play(&self) -> Result<(), MediaEngineError> {
        let (speed, volume, is_muted) = {
            let mut state = self.inner.write().unwrap();
            state.is_playing = true;
            (state.speed, state.volume, state.is_muted)
        };

        let _ = self.event_tx.send(PlayerEvent::StateChange {
            is_playing: true,
            speed,
            volume,
            is_muted,
        });
        Ok(())
    }

    fn pause(&self) -> Result<(), MediaEngineError> {
        let (speed, volume, is_muted) = {
            let mut state = self.inner.write().unwrap();
            state.is_playing = false;
            (state.speed, state.volume, state.is_muted)
        };

        let _ = self.event_tx.send(PlayerEvent::StateChange {
            is_playing: false,
            speed,
            volume,
            is_muted,
        });
        Ok(())
    }

    fn seek_ms(&self, target_ms: u64, _exact: bool) -> Result<(), MediaEngineError> {
        let (new_pos, duration) = {
            let mut state = self.inner.write().unwrap();
            let clamped = if state.duration_ms > 0 {
                target_ms.min(state.duration_ms)
            } else {
                target_ms
            };
            state.position_ms = clamped;
            (clamped, state.duration_ms)
        };

        let _ = self.event_tx.send(PlayerEvent::TimePos {
            position_ms: new_pos,
            duration_ms: duration,
        });
        Ok(())
    }

    fn set_speed(&self, speed: f32) -> Result<(), MediaEngineError> {
        if !is_valid_speed(speed) {
            return Err(MediaEngineError::InvalidSpeed(speed));
        }

        let (is_playing, volume, is_muted) = {
            let mut state = self.inner.write().unwrap();
            state.speed = speed;
            (state.is_playing, state.volume, state.is_muted)
        };

        let _ = self.event_tx.send(PlayerEvent::StateChange {
            is_playing,
            speed,
            volume,
            is_muted,
        });
        Ok(())
    }

    fn set_volume(&self, volume: f64) -> Result<(), MediaEngineError> {
        let clamped = volume.clamp(0.0, 1.0);
        let (is_playing, speed, is_muted) = {
            let mut state = self.inner.write().unwrap();
            state.volume = clamped;
            (state.is_playing, state.speed, state.is_muted)
        };

        let _ = self.event_tx.send(PlayerEvent::StateChange {
            is_playing,
            speed,
            volume: clamped,
            is_muted,
        });
        Ok(())
    }

    fn set_mute(&self, muted: bool) -> Result<(), MediaEngineError> {
        let (is_playing, speed, volume) = {
            let mut state = self.inner.write().unwrap();
            state.is_muted = muted;
            (state.is_playing, state.speed, state.volume)
        };

        let _ = self.event_tx.send(PlayerEvent::StateChange {
            is_playing,
            speed,
            volume,
            is_muted: muted,
        });
        Ok(())
    }

    fn audio_tracks(&self) -> Result<Vec<MediaTrack>, MediaEngineError> {
        let state = self.inner.read().unwrap();
        Ok(state.audio_tracks.clone())
    }

    fn select_audio(&self, track_id: i64) -> Result<(), MediaEngineError> {
        let mut state = self.inner.write().unwrap();
        if !state.audio_tracks.iter().any(|t| t.id == track_id) {
            return Err(MediaEngineError::TrackNotFound(track_id));
        }

        for track in &mut state.audio_tracks {
            track.is_selected = track.id == track_id;
        }
        let tracks = state.audio_tracks.clone();
        drop(state);

        let _ = self.event_tx.send(PlayerEvent::TracksChange { tracks });
        Ok(())
    }

    fn subtitle_tracks(&self) -> Result<Vec<MediaTrack>, MediaEngineError> {
        let state = self.inner.read().unwrap();
        Ok(state.subtitle_tracks.clone())
    }

    fn select_subtitle(&self, track_id: i64) -> Result<(), MediaEngineError> {
        let mut state = self.inner.write().unwrap();
        if track_id == -1 {
            for track in &mut state.subtitle_tracks {
                track.is_selected = false;
            }
            let tracks = state.subtitle_tracks.clone();
            drop(state);
            let _ = self.event_tx.send(PlayerEvent::TracksChange { tracks });
            return Ok(());
        }

        if !state.subtitle_tracks.iter().any(|t| t.id == track_id) {
            return Err(MediaEngineError::TrackNotFound(track_id));
        }

        for track in &mut state.subtitle_tracks {
            track.is_selected = track.id == track_id;
        }
        let tracks = state.subtitle_tracks.clone();
        drop(state);

        let _ = self.event_tx.send(PlayerEvent::TracksChange { tracks });
        Ok(())
    }

    fn add_external_subtitle(
        &self,
        path: &str,
        title: Option<&str>,
        lang: Option<&str>,
    ) -> Result<i64, MediaEngineError> {
        let p = Path::new(path);
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if ext != "srt" && ext != "ass" {
            return Err(MediaEngineError::UnsupportedSubtitleExtension(ext));
        }

        let mut state = self.inner.write().unwrap();
        for t in &mut state.subtitle_tracks {
            t.is_selected = false;
        }

        let new_id = (state.subtitle_tracks.len() as i64) + 1;
        state.subtitle_tracks.push(MediaTrack {
            id: new_id,
            track_type: TrackType::Subtitle,
            title: title.map(|s| s.to_string()),
            lang: lang.map(|s| s.to_string()),
            is_selected: true,
            is_external: true,
            codec: Some(ext),
        });

        let all_tracks = state.subtitle_tracks.clone();
        drop(state);

        let _ = self.event_tx.send(PlayerEvent::TracksChange { tracks: all_tracks });
        Ok(new_id)
    }

    fn get_state(&self) -> Result<PlayerState, MediaEngineError> {
        let state = self.inner.read().unwrap();
        Ok(PlayerState {
            current_file: state.current_file.clone(),
            is_playing: state.is_playing,
            position_ms: state.position_ms,
            duration_ms: state.duration_ms,
            speed: state.speed,
            volume: state.volume,
            is_muted: state.is_muted,
        })
    }

    fn toggle_fullscreen(&self) -> Result<bool, MediaEngineError> {
        let mut state = self.inner.write().unwrap();
        state.is_fullscreen = !state.is_fullscreen;
        Ok(state.is_fullscreen)
    }

    fn playback_state(&self) -> Result<PlaybackState, MediaEngineError> {
        let state = self.inner.read().unwrap();
        Ok(PlaybackState {
            revision: 0,
            status: if state.is_playing {
                PlaybackStatus::Playing
            } else {
                PlaybackStatus::Paused
            },
            position_ms: state.position_ms,
            speed: state.speed,
            updated_at_server_ms: 0,
            duration_ms: state.duration_ms,
        })
    }

    fn dimensions(&self) -> Result<Option<VideoDimensions>, MediaEngineError> {
        let state = self.inner.read().unwrap();
        Ok(state.dimensions)
    }

    fn subscribe_events(&self) -> broadcast::Receiver<PlayerEvent> {
        self.event_tx.subscribe()
    }
}
