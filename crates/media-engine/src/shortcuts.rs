use crate::error::MediaEngineError;
use crate::traits::MediaPlayer;
use serde::{Deserialize, Serialize};

/// Keyboard shortcut actions according to ORIGINAL_REQUEST.md § R1
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeyAction {
    SpaceTogglePlayPause,
    SeekRelative(i64), // milliseconds (+5000, -5000, +30000, -30000)
    ToggleFullscreen,
    ExitFullscreen,
    ToggleMute,
}

/// Dispatches a keyboard shortcut action to the media player and fullscreen state.
pub fn handle_shortcut(
    player: &dyn MediaPlayer,
    action: KeyAction,
    is_fullscreen: &mut bool,
) -> Result<(), MediaEngineError> {
    match action {
        KeyAction::SpaceTogglePlayPause => {
            let state = player.get_state()?;
            if state.is_playing {
                player.pause()?;
            } else {
                player.play()?;
            }
        }
        KeyAction::SeekRelative(delta_ms) => {
            let state = player.get_state()?;
            let cur = state.position_ms as i64;
            let target = (cur + delta_ms).max(0) as u64;
            let final_target = if state.duration_ms > 0 {
                target.min(state.duration_ms)
            } else {
                target
            };
            player.seek(final_target, true)?;
        }
        KeyAction::ToggleFullscreen => {
            *is_fullscreen = !*is_fullscreen;
        }
        KeyAction::ExitFullscreen => {
            *is_fullscreen = false;
        }
        KeyAction::ToggleMute => {
            let state = player.get_state()?;
            player.set_mute(!state.is_muted)?;
        }
    }
    Ok(())
}
