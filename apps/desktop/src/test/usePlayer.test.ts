import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { usePlayer } from '../hooks/usePlayer';
import { mockBackend } from '../services/mockBackend';

describe('usePlayer Hook', () => {
  beforeEach(() => {
    mockBackend.reset();
  });

  it('initializes with default player state', () => {
    const { result } = renderHook(() => usePlayer());
    expect(result.current.currentFile).toBeNull();
    expect(result.current.isPlaying).toBe(false);
    expect(result.current.positionSec).toBe(0);
    expect(result.current.speed).toBe(1.0);
    expect(result.current.volume).toBe(1.0);
    expect(result.current.isMuted).toBe(false);
  });

  it('opens video file and populates metadata', async () => {
    const { result } = renderHook(() => usePlayer());

    await act(async () => {
      await result.current.openFile('/movies/sample.mp4');
    });

    expect(result.current.currentFile).toBe('/movies/sample.mp4');
    expect(result.current.durationSec).toBe(142);
    expect(result.current.positionSec).toBe(0);
    expect(result.current.isPlaying).toBe(false);
  });

  it('toggles play and pause states', async () => {
    const { result } = renderHook(() => usePlayer());

    await act(async () => {
      await result.current.play();
    });
    expect(result.current.isPlaying).toBe(true);

    await act(async () => {
      await result.current.pause();
    });
    expect(result.current.isPlaying).toBe(false);
  });

  it('seeks to target position', async () => {
    const { result } = renderHook(() => usePlayer());
    await act(async () => {
      await result.current.openFile('/movies/sample.mp4');
    });

    await act(async () => {
      await result.current.seek(45);
    });

    expect(result.current.positionSec).toBe(45);
  });

  it('manages scrubber dragging lifecycle', async () => {
    const { result } = renderHook(() => usePlayer());
    await act(async () => {
      await result.current.openFile('/movies/sample.mp4');
    });

    act(() => {
      result.current.startScrub(10);
    });
    expect(result.current.isScrubbing).toBe(true);
    expect(result.current.positionSec).toBe(10);

    act(() => {
      result.current.updateScrub(55);
    });
    expect(result.current.positionSec).toBe(55);

    await act(async () => {
      await result.current.commitScrub();
    });
    expect(result.current.isScrubbing).toBe(false);
    expect(result.current.positionSec).toBe(55);
  });

  it('adjusts discrete playback speed', async () => {
    const { result } = renderHook(() => usePlayer());

    await act(async () => {
      await result.current.setSpeed(1.5);
    });

    expect(result.current.speed).toBe(1.5);
  });

  it('adjusts volume and mute toggles', async () => {
    const { result } = renderHook(() => usePlayer());

    await act(async () => {
      await result.current.setVolume(0.4);
    });
    expect(result.current.volume).toBe(0.4);

    await act(async () => {
      await result.current.toggleMute();
    });
    expect(result.current.isMuted).toBe(true);
  });
});
