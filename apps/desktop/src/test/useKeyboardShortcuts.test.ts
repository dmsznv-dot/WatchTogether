import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';

describe('useKeyboardShortcuts Hook', () => {
  it('uses physical keys in the Russian layout', () => {
    const onToggleFullscreen = vi.fn();
    const onToggleMute = vi.fn();
    const onOpenFile = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onToggleFullscreen, onToggleMute, onOpenFile }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'а', code: 'KeyF' }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ь', code: 'KeyM' }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'щ', code: 'KeyO', ctrlKey: true }));
    expect(onToggleFullscreen).toHaveBeenCalledOnce();
    expect(onToggleMute).toHaveBeenCalledOnce();
    expect(onOpenFile).toHaveBeenCalledOnce();
  });

  it('does not hijack modified shortcuts or repeat toggle actions', () => {
    const onToggleFullscreen = vi.fn();
    const onTogglePlayPause = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onToggleFullscreen, onTogglePlayPause }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'f', ctrlKey: true }));
    window.dispatchEvent(new KeyboardEvent('keydown', { key: ' ', repeat: true }));
    expect(onToggleFullscreen).not.toHaveBeenCalled();
    expect(onTogglePlayPause).not.toHaveBeenCalled();
  });

  it('leaves nested editable content alone', () => {
    const onToggleMute = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onToggleMute }));
    const editor = document.createElement('div');
    editor.setAttribute('contenteditable', '');
    const child = document.createElement('span');
    editor.appendChild(child);
    document.body.appendChild(editor);
    child.dispatchEvent(new KeyboardEvent('keydown', { key: 'm', bubbles: true }));
    expect(onToggleMute).not.toHaveBeenCalled();
    editor.remove();
  });
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('triggers play/pause on Space key', () => {
    const onTogglePlayPause = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onTogglePlayPause }));

    window.dispatchEvent(new KeyboardEvent('keydown', { key: ' ' }));
    expect(onTogglePlayPause).toHaveBeenCalledTimes(1);
  });

  it('triggers relative seeking on arrow keys', () => {
    const onSeekRelative = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onSeekRelative }));

    // Arrow Left (-5s)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowLeft' }));
    expect(onSeekRelative).toHaveBeenCalledWith(-5);

    // Arrow Right (+5s)
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight' }));
    expect(onSeekRelative).toHaveBeenCalledWith(5);

    // Shift + Arrow Left (-30s)
    window.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowLeft', shiftKey: true })
    );
    expect(onSeekRelative).toHaveBeenCalledWith(-30);

    // Shift + Arrow Right (+30s)
    window.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'ArrowRight', shiftKey: true })
    );
    expect(onSeekRelative).toHaveBeenCalledWith(30);
  });

  it('triggers fullscreen toggle on F key', () => {
    const onToggleFullscreen = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onToggleFullscreen }));

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'f' }));
    expect(onToggleFullscreen).toHaveBeenCalledTimes(1);
  });

  it('triggers mute toggle on M key', () => {
    const onToggleMute = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onToggleMute }));

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'm' }));
    expect(onToggleMute).toHaveBeenCalledTimes(1);
  });

  it('triggers file picker on Cmd/Ctrl+O', () => {
    const onOpenFile = vi.fn();
    renderHook(() => useKeyboardShortcuts({ onOpenFile }));

    window.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'o', metaKey: true })
    );
    expect(onOpenFile).toHaveBeenCalledTimes(1);
  });

  it('guards against active text inputs', () => {
    const onTogglePlayPause = vi.fn();
    const onSeekRelative = vi.fn();
    renderHook(() =>
      useKeyboardShortcuts({ onTogglePlayPause, onSeekRelative })
    );

    const input = document.createElement('input');
    document.body.appendChild(input);
    input.focus();

    // Dispatch event targeted at input
    const event = new KeyboardEvent('keydown', { key: ' ', bubbles: true });
    input.dispatchEvent(event);

    expect(onTogglePlayPause).not.toHaveBeenCalled();

    document.body.removeChild(input);
  });
});
