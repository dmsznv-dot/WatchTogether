use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaEngineError {
    #[error("libmpv dynamic library not found: {0}")]
    LibMpvNotFound(String),

    #[error("Failed to initialize libmpv context: {0}")]
    LibMpvInitFailed(String),

    #[error("libmpv command '{command}' failed with error code {code}: {message}")]
    CommandFailed {
        command: String,
        code: i32,
        message: String,
    },

    #[error("Failed to get/set property '{property}': {message}")]
    PropertyError {
        property: String,
        message: String,
    },

    #[error("Media file does not exist: {0}")]
    FileNotFound(String),

    #[error("Track ID {0} not found")]
    TrackNotFound(i64),

    #[error("Unsupported subtitle file extension '{0}', expected .srt or .ass")]
    UnsupportedSubtitleExtension(String),

    #[error("Unsupported audio file extension '{0}', expected an audio file such as .mp3, .m4a, .aac, .ac3, .flac, .wav or .mka")]
    UnsupportedAudioExtension(String),

    #[error("Invalid playback speed: {0} (must be between 0.5 and 2.0)")]
    InvalidSpeed(f32),

    #[error("Invalid volume: {0} (must be between 0.0 and 1.0)")]
    InvalidVolume(f64),

    #[error("Internal engine error: {0}")]
    Internal(String),
}
