import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useAutoHide } from '../hooks/useAutoHide';

describe('useAutoHide Hook', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('keeps controls visible on mount', () => {
    const { result } = renderHook(() =>
      useAutoHide({ isPlaying: false, timeoutMs: 2500 })
    );
    expect(result.current.isVisible).toBe(true);
  });

  it('hides controls after inactivity timeout when isPlaying is true', () => {
    const { result } = renderHook(() =>
      useAutoHide({ isPlaying: true, timeoutMs: 2500 })
    );

    expect(result.current.isVisible).toBe(true);

    act(() => {
      vi.advanceTimersByTime(2500);
    });

    expect(result.current.isVisible).toBe(false);
  });

  it('keeps controls visible if paused', () => {
    const { result } = renderHook(() =>
      useAutoHide({ isPlaying: false, timeoutMs: 2500 })
    );

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(result.current.isVisible).toBe(true);
  });

  it('keeps controls visible if suspended (e.g. modal open or scrubbing)', () => {
    const { result } = renderHook(() =>
      useAutoHide({ isPlaying: true, isSuspended: true, timeoutMs: 2500 })
    );

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(result.current.isVisible).toBe(true);
  });

  it('resets timer on mousemove activity', () => {
    const { result } = renderHook(() =>
      useAutoHide({ isPlaying: true, timeoutMs: 2500 })
    );

    act(() => {
      vi.advanceTimersByTime(2000);
    });

    expect(result.current.isVisible).toBe(true);

    // Activity occurs at 2000ms
    act(() => {
      window.dispatchEvent(new MouseEvent('mousemove'));
    });

    // Advance another 1000ms (total 3000ms from start, but 1000ms from reset)
    act(() => {
      vi.advanceTimersByTime(1000);
    });

    expect(result.current.isVisible).toBe(true);

    // Advance remaining 1500ms
    act(() => {
      vi.advanceTimersByTime(1500);
    });

    expect(result.current.isVisible).toBe(false);
  });

  it('keeps controls visible when hovered', () => {
    const { result } = renderHook(() =>
      useAutoHide({ isPlaying: true, timeoutMs: 2500 })
    );

    act(() => {
      result.current.bindHover.onMouseEnter();
    });

    act(() => {
      vi.advanceTimersByTime(5000);
    });

    expect(result.current.isVisible).toBe(true);

    act(() => {
      result.current.bindHover.onMouseLeave();
    });

    act(() => {
      vi.advanceTimersByTime(2500);
    });

    expect(result.current.isVisible).toBe(false);
  });
});
