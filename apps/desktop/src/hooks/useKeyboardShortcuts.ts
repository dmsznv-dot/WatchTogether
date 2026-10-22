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
      if (e.defaultPrevented || e.isComposing) return;
      const target = e.target;
      if (target instanceof Element && target.closest(
        'input, textarea, select, [contenteditable]:not([contenteditable="false"])'
      )) return;

      const isKey = (code: string, key: string) =>
        e.code === code || (!e.code && e.key.toLowerCase() === key);

      // Cmd+O (macOS) / Ctrl+O (Windows)
      if ((e.metaKey || e.ctrlKey) && isKey('KeyO', 'o') && !e.altKey) {
        e.preventDefault();
        if (!e.repeat) onOpenFile?.();
        return;
      }

      if (e.metaKey || e.ctrlKey || e.altKey) return;

      // Space: Play / Pause
      if (e.key === ' ' || e.code === 'Space') {
        e.preventDefault();
        if (!e.repeat) onTogglePlayPause?.();
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
      if (isKey('KeyF', 'f')) {
        e.preventDefault();
        if (!e.repeat) onToggleFullscreen?.();
        return;
      }

      // M / m: Toggle Mute
      if (isKey('KeyM', 'm')) {
        e.preventDefault();
        if (!e.repeat) onToggleMute?.();
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
