import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act, renderHook } from '@testing-library/react';
import { usePlayer } from '../hooks/usePlayer';
import { useAutoHide } from '../hooks/useAutoHide';
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts';
import { VideoCanvas } from '../features/player/VideoCanvas';
import { Scrubber } from '../features/player/Scrubber';
import { mockBackend } from '../services/mockBackend';
import { VideoDimensions } from '../types/player';

describe('Milestone 4 Empirical Stress Tests: Player HUD & Media Controls', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    mockBackend.reset();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  // ==========================================================================
  // AREA 1: Rapid Concurrent Seek Events & Boundary Seeks
  // ==========================================================================
  describe('Area 1: Rapid Concurrent Seek Events & Boundary Seeks', () => {
    it('handles boundary seeks: 0s, exact duration, past duration, and negative seeks', async () => {
      const { result } = renderHook(() => usePlayer());

      // Open a video file (mock duration = 142s)
      await act(async () => {
        await result.current.openFile('/path/to/test.mp4');
      });

      expect(result.current.durationSec).toBe(142);

      // Boundary 1: 0s seek
      await act(async () => {
        await result.current.seek(0);
      });
      expect(result.current.positionSec).toBe(0);

      // Boundary 2: exact duration seek (142s)
      await act(async () => {
        await result.current.seek(142);
      });
      expect(result.current.positionSec).toBe(142);

      // Boundary 3: past duration seek (e.g. 500s, 99999s) -> clamped to durationSec (142s)
      await act(async () => {
        await result.current.seek(500);
      });
      expect(result.current.positionSec).toBe(142);

      await act(async () => {
        await result.current.seek(99999);
      });
      expect(result.current.positionSec).toBe(142);

      // Boundary 4: negative seeks (-10s, -999s) -> clamped to 0s
      await act(async () => {
        await result.current.seek(-10);
      });
      expect(result.current.positionSec).toBe(0);

      await act(async () => {
        await result.current.seek(-999);
      });
      expect(result.current.positionSec).toBe(0);
    });

    it('handles seek when durationSec is 0 (unloaded / live stream)', async () => {
      const { result } = renderHook(() => usePlayer());
      expect(result.current.durationSec).toBe(0);

      // Seeks should not crash and clamp negative to 0
      await act(async () => {
        await result.current.seek(-5);
      });
      expect(result.current.positionSec).toBe(0);

      await act(async () => {
        await result.current.seek(10);
      });
      // When duration is 0 (no media loaded), position is clamped to 0
      expect(result.current.positionSec).toBe(0);
    });

    it('handles 100 rapid concurrent seek calls without race condition crash', async () => {
      const { result } = renderHook(() => usePlayer());

      await act(async () => {
        await result.current.openFile('/path/to/test.mp4');
      });

      // Launch 100 concurrent seek promises targeting random valid and boundary positions
      const targets = [0, 142, -50, 200, 30, 45, 90, 120, -5, 142];
      await act(async () => {
        const promises = Array.from({ length: 100 }, (_, i) => {
          const target = targets[i % targets.length];
          return result.current.seek(target);
        });
        await Promise.all(promises);
      });

      // Final state should be a valid clamped number within [0, 142]
      expect(result.current.positionSec).toBeGreaterThanOrEqual(0);
      expect(result.current.positionSec).toBeLessThanOrEqual(142);
      expect(Number.isFinite(result.current.positionSec)).toBe(true);
    });

    it('manages scrubber drag lifecycle under rapid updates and commits cleanly', async () => {
      const { result } = renderHook(() => usePlayer());

      await act(async () => {
        await result.current.openFile('/path/to/test.mp4');
      });

      act(() => {
        result.current.startScrub(10);
      });
      expect(result.current.isScrubbing).toBe(true);
      expect(result.current.positionSec).toBe(10);

      // 50 rapid scrub moves
      for (let i = 0; i < 50; i++) {
        act(() => {
          result.current.updateScrub(i * 3);
        });
        expect(result.current.positionSec).toBe(Math.min(142, i * 3));
      }

      // Commit scrub
      await act(async () => {
        await result.current.commitScrub();
      });
      expect(result.current.isScrubbing).toBe(false);
      expect(result.current.positionSec).toBe(142);
    });

    it('renders Scrubber component resiliently against extreme and boundary positions', () => {
      const onStartScrub = vi.fn();
      const onUpdateScrub = vi.fn();
      const onCommitScrub = vi.fn();

      const { rerender } = render(
        <Scrubber
          positionSec={0}
          durationSec={100}
          isScrubbing={false}
          onStartScrub={onStartScrub}
          onUpdateScrub={onUpdateScrub}
          onCommitScrub={onCommitScrub}
        />
      );

      const playedBar = screen.getByTestId('scrubber-played');
      expect(playedBar.style.width).toBe('0%');

      // Past duration: 150 / 100 -> clamped to 100%
      rerender(
        <Scrubber
          positionSec={150}
          durationSec={100}
          isScrubbing={false}
          onStartScrub={onStartScrub}
          onUpdateScrub={onUpdateScrub}
          onCommitScrub={onCommitScrub}
        />
      );
      expect(playedBar.style.width).toBe('100%');

      // Negative position: -20 / 100 -> clamped to 0%
      rerender(
        <Scrubber
          positionSec={-20}
          durationSec={100}
          isScrubbing={false}
          onStartScrub={onStartScrub}
          onUpdateScrub={onUpdateScrub}
          onCommitScrub={onCommitScrub}
        />
      );
      expect(playedBar.style.width).toBe('0%');

      // Zero duration: 0 / 0 -> 0% without NaN crash
      rerender(
        <Scrubber
          positionSec={0}
          durationSec={0}
          isScrubbing={false}
          onStartScrub={onStartScrub}
          onUpdateScrub={onUpdateScrub}
          onCommitScrub={onCommitScrub}
        />
      );
      expect(playedBar.style.width).toBe('0%');
    });
  });

  // ==========================================================================
  // AREA 2: Auto-Hide Timer Race Conditions
  // ==========================================================================
  describe('Area 2: Auto-Hide Timer Race Conditions', () => {
    it('keeps HUD visible during rapid burst of mouse movements', () => {
      const { result } = renderHook(() =>
        useAutoHide({
          isPlaying: true,
          isSuspended: false,
          timeoutMs: 2500,
        })
      );

      expect(result.current.isVisible).toBe(true);

      // Simulate 50 mouse moves occurring every 100ms (total 5000ms > timeoutMs)
      for (let i = 0; i < 50; i++) {
        act(() => {
          vi.advanceTimersByTime(100);
          window.dispatchEvent(new MouseEvent('mousemove'));
        });
        expect(result.current.isVisible).toBe(true);
      }

      // After mouse moves cease, wait 2490ms -> still visible
      act(() => {
        vi.advanceTimersByTime(2490);
      });
      expect(result.current.isVisible).toBe(true);

      // Cross 2500ms mark -> hides
      act(() => {
        vi.advanceTimersByTime(20);
      });
      expect(result.current.isVisible).toBe(false);
    });

    it('NEVER hides controls when playback is paused, regardless of timeout or inactivity', () => {
      const { result } = renderHook(() =>
        useAutoHide({
          isPlaying: false, // Paused
          isSuspended: false,
          timeoutMs: 2500,
        })
      );

      expect(result.current.isVisible).toBe(true);

      // Advance 10 seconds of complete idle time while paused
      act(() => {
        vi.advanceTimersByTime(10000);
      });

      expect(result.current.isVisible).toBe(true);

      // Mouse movements while paused should not cause timer to arm
      act(() => {
        window.dispatchEvent(new MouseEvent('mousemove'));
        vi.advanceTimersByTime(5000);
      });
      expect(result.current.isVisible).toBe(true);
    });

    it('NEVER hides controls when scrubbing or when suspended (modal open)', () => {
      const { result, rerender } = renderHook(
        (props: { isPlaying: boolean; isSuspended: boolean }) =>
          useAutoHide({
            isPlaying: props.isPlaying,
            isSuspended: props.isSuspended,
            timeoutMs: 2500,
          }),
        {
          initialProps: { isPlaying: true, isSuspended: true }, // Scrubbing or Modal Open
        }
      );

      expect(result.current.isVisible).toBe(true);

      // Idle 15 seconds while scrubbing/suspended
      act(() => {
        vi.advanceTimersByTime(15000);
      });
      expect(result.current.isVisible).toBe(true);

      // Unsuspend (user releases scrubber)
      rerender({ isPlaying: true, isSuspended: false });

      // After unsuspending, timer starts: 2490ms -> still visible
      act(() => {
        vi.advanceTimersByTime(2490);
      });
      expect(result.current.isVisible).toBe(true);

      // 2500ms -> hides
      act(() => {
        vi.advanceTimersByTime(20);
      });
      expect(result.current.isVisible).toBe(false);
    });

    it('keeps controls visible indefinitely when hovered, and hides 2.5s after mouse leaves', () => {
      const { result } = renderHook(() =>
        useAutoHide({
          isPlaying: true,
          isSuspended: false,
          timeoutMs: 2500,
        })
      );

      // Hover over controls HUD
      act(() => {
        result.current.bindHover.onMouseEnter();
      });
      expect(result.current.isHovered).toBe(true);

      // Idle 20 seconds while hovered
      act(() => {
        vi.advanceTimersByTime(20000);
      });
      expect(result.current.isVisible).toBe(true);

      // Mouse leaves HUD
      act(() => {
        result.current.bindHover.onMouseLeave();
      });
      expect(result.current.isHovered).toBe(false);

      // 2400ms after leaving -> still visible
      act(() => {
        vi.advanceTimersByTime(2400);
      });
      expect(result.current.isVisible).toBe(true);

      // 2600ms total after leaving -> hidden
      act(() => {
        vi.advanceTimersByTime(200);
      });
      expect(result.current.isVisible).toBe(false);
    });

    it('survives rapid play/pause state transitions without dangling timers', () => {
      const { result, rerender } = renderHook(
        (props: { isPlaying: boolean }) =>
          useAutoHide({
            isPlaying: props.isPlaying,
            isSuspended: false,
            timeoutMs: 2500,
          }),
        {
          initialProps: { isPlaying: true },
        }
      );

      // Rapidly flip play -> pause -> play -> pause 20 times
      for (let i = 0; i < 20; i++) {
        act(() => {
          rerender({ isPlaying: i % 2 === 0 });
          vi.advanceTimersByTime(100);
        });
      }

      // End on paused
      rerender({ isPlaying: false });
      act(() => {
        vi.advanceTimersByTime(5000);
      });
      expect(result.current.isVisible).toBe(true);
    });
  });

  // ==========================================================================
  // AREA 3: Keyboard Shortcuts Stress
  // ==========================================================================
  describe('Area 3: Keyboard Shortcuts Stress', () => {
    it('STRICTLY IGNORES keyboard shortcuts when events originate inside form inputs or editable elements', () => {
      const onTogglePlayPause = vi.fn();
      const onSeekRelative = vi.fn();
      const onToggleFullscreen = vi.fn();
      const onToggleMute = vi.fn();
      const onAdjustVolume = vi.fn();

      renderHook(() =>
        useKeyboardShortcuts({
          onTogglePlayPause,
          onSeekRelative,
          onToggleFullscreen,
          onToggleMute,
          onAdjustVolume,
        })
      );

      // 1. Create native HTML Input element (e.g. Room Code input)
      const inputEl = document.createElement('input');
      document.body.appendChild(inputEl);
      inputEl.focus();

      // Press Space, Arrows, F, M inside input
      fireEvent.keyDown(inputEl, { key: ' ', code: 'Space' });
      fireEvent.keyDown(inputEl, { key: 'ArrowLeft' });
      fireEvent.keyDown(inputEl, { key: 'ArrowRight', shiftKey: true });
      fireEvent.keyDown(inputEl, { key: 'ArrowUp' });
      fireEvent.keyDown(inputEl, { key: 'f' });
      fireEvent.keyDown(inputEl, { key: 'm' });

      expect(onTogglePlayPause).not.toHaveBeenCalled();
      expect(onSeekRelative).not.toHaveBeenCalled();
      expect(onToggleFullscreen).not.toHaveBeenCalled();
      expect(onToggleMute).not.toHaveBeenCalled();
      expect(onAdjustVolume).not.toHaveBeenCalled();

      // 2. Create native HTML Textarea element
      const textareaEl = document.createElement('textarea');
      document.body.appendChild(textareaEl);
      textareaEl.focus();

      fireEvent.keyDown(textareaEl, { key: ' ', code: 'Space' });
      fireEvent.keyDown(textareaEl, { key: 'f' });

      expect(onTogglePlayPause).not.toHaveBeenCalled();
      expect(onToggleFullscreen).not.toHaveBeenCalled();

      // 3. Create contenteditable div
      const editableDiv = document.createElement('div');
      editableDiv.setAttribute('contenteditable', 'true');
      document.body.appendChild(editableDiv);
      editableDiv.focus();

      fireEvent.keyDown(editableDiv, { key: ' ', code: 'Space' });
      fireEvent.keyDown(editableDiv, { key: 'm' });

      expect(onTogglePlayPause).not.toHaveBeenCalled();
      expect(onToggleMute).not.toHaveBeenCalled();

      // Cleanup
      document.body.removeChild(inputEl);
      document.body.removeChild(textareaEl);
      document.body.removeChild(editableDiv);
    });

    it('correctly discriminates Shift modifier combinations (±5s vs ±30s seek, uppercase F/M)', () => {
      const onSeekRelative = vi.fn();
      const onToggleFullscreen = vi.fn();
      const onToggleMute = vi.fn();
      const onTogglePlayPause = vi.fn();

      renderHook(() =>
        useKeyboardShortcuts({
          onSeekRelative,
          onToggleFullscreen,
          onToggleMute,
          onTogglePlayPause,
        })
      );

      // ArrowLeft without shift: -5s
      fireEvent.keyDown(window, { key: 'ArrowLeft', shiftKey: false });
      expect(onSeekRelative).toHaveBeenLastCalledWith(-5);

      // ArrowLeft with shift: -30s
      fireEvent.keyDown(window, { key: 'ArrowLeft', shiftKey: true });
      expect(onSeekRelative).toHaveBeenLastCalledWith(-30);

      // ArrowRight without shift: +5s
      fireEvent.keyDown(window, { key: 'ArrowRight', shiftKey: false });
      expect(onSeekRelative).toHaveBeenLastCalledWith(5);

      // ArrowRight with shift: +30s
      fireEvent.keyDown(window, { key: 'ArrowRight', shiftKey: true });
      expect(onSeekRelative).toHaveBeenLastCalledWith(30);

      // Uppercase 'F' (with Shift) and lowercase 'f'
      fireEvent.keyDown(window, { key: 'f' });
      expect(onToggleFullscreen).toHaveBeenCalledTimes(1);
      fireEvent.keyDown(window, { key: 'F', shiftKey: true });
      expect(onToggleFullscreen).toHaveBeenCalledTimes(2);

      // Uppercase 'M' (with Shift) and lowercase 'm'
      fireEvent.keyDown(window, { key: 'm' });
      expect(onToggleMute).toHaveBeenCalledTimes(1);
      fireEvent.keyDown(window, { key: 'M', shiftKey: true });
      expect(onToggleMute).toHaveBeenCalledTimes(2);

      // Space with Shift
      fireEvent.keyDown(window, { key: ' ', shiftKey: true });
      expect(onTogglePlayPause).toHaveBeenCalledTimes(1);
    });

    it('handles 100 rapid alternating key presses without error', () => {
      const onTogglePlayPause = vi.fn();
      const onSeekRelative = vi.fn();
      const onAdjustVolume = vi.fn();

      renderHook(() =>
        useKeyboardShortcuts({
          onTogglePlayPause,
          onSeekRelative,
          onAdjustVolume,
        })
      );

      const keys = [
        { key: ' ', code: 'Space' },
        { key: 'ArrowLeft' },
        { key: 'ArrowRight', shiftKey: true },
        { key: 'ArrowUp' },
        { key: 'ArrowDown' },
      ];

      for (let i = 0; i < 100; i++) {
        const k = keys[i % keys.length];
        fireEvent.keyDown(window, k);
      }

      expect(onTogglePlayPause).toHaveBeenCalledTimes(20);
      expect(onSeekRelative).toHaveBeenCalledTimes(40);
      expect(onAdjustVolume).toHaveBeenCalledTimes(40);
    });

    it('completely ignores all shortcuts when disabled is true', () => {
      const onTogglePlayPause = vi.fn();
      const onSeekRelative = vi.fn();

      renderHook(() =>
        useKeyboardShortcuts({
          disabled: true,
          onTogglePlayPause,
          onSeekRelative,
        })
      );

      fireEvent.keyDown(window, { key: ' ' });
      fireEvent.keyDown(window, { key: 'ArrowLeft' });
      fireEvent.keyDown(window, { key: 'ArrowRight' });

      expect(onTogglePlayPause).not.toHaveBeenCalled();
      expect(onSeekRelative).not.toHaveBeenCalled();
    });
  });

  // ==========================================================================
  // AREA 4: Extreme Video Dimensions & Aspect Ratio Calculation
  // ==========================================================================
  describe('Area 4: Extreme Video Dimensions in Aspect Ratio Calculator & VideoCanvas', () => {
    // Aspect ratio contract validator based on PROJECT.md & media-engine dimensions.rs
    const calculateAspectRatio = (
      dims?: VideoDimensions | { width?: number | null; height?: number | null } | null
    ): number => {
      if (!dims || !dims.width || !dims.height || dims.width <= 0 || dims.height <= 0) {
        return 16 / 9; // Fallback default
      }
      return dims.width / dims.height;
    };

    it('calculates aspect ratios accurately across extreme dimensions', () => {
      // 1. Standard 16:9 (1920x1080)
      const ar16_9 = calculateAspectRatio({ width: 1920, height: 1080 });
      expect(Math.abs(ar16_9 - 16 / 9)).toBeLessThan(1e-4);

      // 2. Square 1:1 (1x1 and 1000x1000)
      const ar1_1_min = calculateAspectRatio({ width: 1, height: 1 });
      expect(ar1_1_min).toBe(1.0);
      const ar1_1_norm = calculateAspectRatio({ width: 1000, height: 1000 });
      expect(ar1_1_norm).toBe(1.0);

      // 3. Ultra-wide 32:9 (5120x1440)
      const ar32_9 = calculateAspectRatio({ width: 5120, height: 1440 });
      expect(Math.abs(ar32_9 - 32 / 9)).toBeLessThan(1e-4);

      // 4. Vertical / Smartphone 9:16 (1080x1920)
      const ar9_16 = calculateAspectRatio({ width: 1080, height: 1920 });
      expect(Math.abs(ar9_16 - 9 / 16)).toBeLessThan(1e-4);

      // 5. Extreme Zero Dimensions (0x0, 0x1080, 1920x0) -> Safe fallback to 16/9 without NaN or Infinity
      expect(calculateAspectRatio({ width: 0, height: 0 })).toBe(16 / 9);
      expect(calculateAspectRatio({ width: 0, height: 1080 })).toBe(16 / 9);
      expect(calculateAspectRatio({ width: 1920, height: 0 })).toBe(16 / 9);
      expect(calculateAspectRatio(null)).toBe(16 / 9);
      expect(calculateAspectRatio(undefined)).toBe(16 / 9);

      // 6. Negative Dimensions -> Safe fallback to 16/9
      expect(calculateAspectRatio({ width: -1920, height: 1080 })).toBe(16 / 9);
      expect(calculateAspectRatio({ width: 1920, height: -1080 })).toBe(16 / 9);
    });

    it('renders VideoCanvas safely with extreme aspect ratio numbers', () => {
      const onOpenFile = vi.fn();
      const onTogglePlayPause = vi.fn();
      const onToggleFullscreen = vi.fn();
      const onSelectFileClick = vi.fn();

      // Render with 32:9 Ultra-Wide aspect ratio
      const { rerender } = render(
        <VideoCanvas
          currentFile="/path/to/movie.mp4"
          isPlaying={true}
          aspectRatio={32 / 9}
          onOpenFile={onOpenFile}
          onTogglePlayPause={onTogglePlayPause}
          onToggleFullscreen={onToggleFullscreen}
          onSelectFileClick={onSelectFileClick}
        />
      );

      const viewport = screen.getByTestId('aspect-ratio-viewport');
      expect(viewport).toBeInTheDocument();
      expect(viewport.style.aspectRatio).toBe('3.5555555555555554');

      // Render with 9:16 Vertical aspect ratio
      rerender(
        <VideoCanvas
          currentFile="/path/to/movie.mp4"
          isPlaying={true}
          aspectRatio={9 / 16}
          onOpenFile={onOpenFile}
          onTogglePlayPause={onTogglePlayPause}
          onToggleFullscreen={onToggleFullscreen}
          onSelectFileClick={onSelectFileClick}
        />
      );
      expect(viewport.style.aspectRatio).toBe('0.5625');

      // Render with 1:1 Square aspect ratio
      rerender(
        <VideoCanvas
          currentFile="/path/to/movie.mp4"
          isPlaying={true}
          aspectRatio={1}
          onOpenFile={onOpenFile}
          onTogglePlayPause={onTogglePlayPause}
          onToggleFullscreen={onToggleFullscreen}
          onSelectFileClick={onSelectFileClick}
        />
      );
      expect(viewport.style.aspectRatio).toBe('1');
    });
  });
});
