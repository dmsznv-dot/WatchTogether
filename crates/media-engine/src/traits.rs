use crate::dimensions::VideoDimensions;
use crate::error::MediaEngineError;
use crate::events::PlayerEvent;
use crate::state::PlayerState;
use crate::track::MediaTrack;
use shared_protocol::models::PlaybackState;
use tokio::sync::broadcast;

/// Unified thread-safe media player interface for Watch Together.
pub trait MediaPlayer: Send + Sync {
    /// Opens a local media file or remote stream URL.
    fn open_file(&self, path_or_url: &str) -> Result<(), MediaEngineError>;

    /// Alias for `open_file`.
    fn open(&self, path_or_url: &str) -> Result<(), MediaEngineError> {
        self.open_file(path_or_url)
    }

    /// Starts or resumes playback.
    fn play(&self) -> Result<(), MediaEngineError>;

    /// Pauses playback.
    fn pause(&self) -> Result<(), MediaEngineError>;

    /// Seeks to a specific position in milliseconds.
    fn seek_ms(&self, target_ms: u64, exact: bool) -> Result<(), MediaEngineError>;

    /// Alias for `seek_ms`.
    fn seek(&self, target_ms: u64, exact: bool) -> Result<(), MediaEngineError> {
        self.seek_ms(target_ms, exact)
    }

    /// Sets playback speed multiplier (0.5x to 2.0x).
    fn set_speed(&self, speed: f32) -> Result<(), MediaEngineError>;

    /// Sets volume (normalized 0.0 to 1.0).
    fn set_volume(&self, volume: f64) -> Result<(), MediaEngineError>;

    /// Sets mute state.
    fn set_mute(&self, muted: bool) -> Result<(), MediaEngineError>;

    /// Retrieves all available audio tracks.
    fn audio_tracks(&self) -> Result<Vec<MediaTrack>, MediaEngineError>;

    /// Selects an active audio track by ID.
    fn select_audio(&self, track_id: i64) -> Result<(), MediaEngineError>;

    /// Alias for `select_audio`.
    fn select_audio_track(&self, track_id: i64) -> Result<(), MediaEngineError> {
        self.select_audio(track_id)
    }

    /// Retrieves all available subtitle tracks.
    fn subtitle_tracks(&self) -> Result<Vec<MediaTrack>, MediaEngineError>;

    /// Selects an active subtitle track by ID (-1 to disable).
    fn select_subtitle(&self, track_id: i64) -> Result<(), MediaEngineError>;

    /// Alias for `select_subtitle`.
    fn select_subtitle_track(&self, track_id: i64) -> Result<(), MediaEngineError> {
        self.select_subtitle(track_id)
    }

    /// Loads an external subtitle file (.srt, .ass).
    fn add_external_subtitle(
        &self,
        path: &str,
        title: Option<&str>,
        lang: Option<&str>,
    ) -> Result<i64, MediaEngineError>;

    /// Retrieves current local player state snapshot.
    fn get_state(&self) -> Result<PlayerState, MediaEngineError>;

    /// Toggles fullscreen mode for the player.
    fn toggle_fullscreen(&self) -> Result<bool, MediaEngineError> {
        Ok(false)
    }

    /// Retrieves shared protocol playback state.
    fn playback_state(&self) -> Result<PlaybackState, MediaEngineError>;

    /// Retrieves video dimensions (width, height) for aspect ratio handling.
    fn dimensions(&self) -> Result<Option<VideoDimensions>, MediaEngineError>;

    /// Subscribes to real-time player event stream.
    fn subscribe_events(&self) -> broadcast::Receiver<PlayerEvent>;
}
