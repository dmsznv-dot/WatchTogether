import { useEffect } from 'react';

export interface KeyboardShortcutsHandlers {
  onTogglePlayPause?: () => void;
  onSeekRelative?: (seconds: number) => void;
  onToggleFullscreen?: () => void;
  onToggleMute?: () => void;
  onAdjustVolume?: (delta: number) => void;
  onOpenFile?: () => void;
  onEscape?: () => void;
  disabled?: boolean;
}

export function useKeyboardShortcuts({
  onTogglePlayPause,
  onSeekRelative,
  onToggleFullscreen,
  onToggleMute,
  onAdjustVolume,
  onOpenFile,
  onEscape,
  disabled = false,
}: KeyboardShortcutsHandlers) {
  useEffect(() => {
    if (disabled) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      // Input field guard: Never intercept key events when typing into an input or editable field
      const target = e.target as HTMLElement | null;
      if (target) {
        const tagName = target.tagName?.toLowerCase();
        const isEditable = typeof target.getAttribute === 'function' && target.getAttribute('contenteditable') === 'true';
        if (tagName === 'input' || tagName === 'textarea' || isEditable) {
          return;
        }
      }

      // Cmd+O (macOS) / Ctrl+O (Windows)
      if ((e.metaKey || e.ctrlKey) && (e.key === 'o' || e.key === 'O')) {
        e.preventDefault();
        onOpenFile?.();
        return;
      }

      // Space: Play / Pause
      if (e.key === ' ' || e.code === 'Space') {
        e.preventDefault();
        onTogglePlayPause?.();
        return;
      }

      // Arrow Left / Right: Seek
      if (e.key === 'ArrowLeft') {
        e.preventDefault();
        const delta = e.shiftKey ? -30 : -5;
        onSeekRelative?.(delta);
        return;
      }
      if (e.key === 'ArrowRight') {
        e.preventDefault();
        const delta = e.shiftKey ? 30 : 5;
        onSeekRelative?.(delta);
        return;
      }

      // Arrow Up / Down: Local volume ±0.05
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        onAdjustVolume?.(0.05);
        return;
      }
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        onAdjustVolume?.(-0.05);
        return;
      }

      // F / f: Toggle Fullscreen
      if (e.key === 'f' || e.key === 'F') {
        e.preventDefault();
        onToggleFullscreen?.();
        return;
      }

      // M / m: Toggle Mute
      if (e.key === 'm' || e.key === 'M') {
        e.preventDefault();
        onToggleMute?.();
        return;
      }

      // Escape: Dismiss or exit fullscreen
      if (e.key === 'Escape') {
        e.preventDefault();
        onEscape?.();
        return;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [
    disabled,
    onTogglePlayPause,
    onSeekRelative,
    onToggleFullscreen,
    onToggleMute,
    onAdjustVolume,
    onOpenFile,
    onEscape,
  ]);
}
