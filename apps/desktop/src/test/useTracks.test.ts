import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useTracks } from '../hooks/useTracks';
import { mockBackend } from '../services/mockBackend';

describe('useTracks Hook', () => {
  beforeEach(() => {
    mockBackend.reset();
  });

  it('loads audio and subtitle tracks from backend', async () => {
    const { result } = renderHook(() => useTracks());

    await act(async () => {
      await result.current.refreshTracks();
    });

    expect(result.current.audioTracks.length).toBeGreaterThan(0);
    expect(result.current.subtitleTracks.length).toBeGreaterThan(0);
  });

  it('selects active audio track', async () => {
    const { result } = renderHook(() => useTracks());
    await act(async () => {
      await result.current.refreshTracks();
    });

    await act(async () => {
      await result.current.selectAudio(2);
    });

    expect(
      result.current.audioTracks.find((t) => t.id === 2)?.is_selected
    ).toBe(true);
  });

  it('selects subtitle track or disables with -1', async () => {
    const { result } = renderHook(() => useTracks());
    await act(async () => {
      await result.current.refreshTracks();
    });

    await act(async () => {
      await result.current.selectSubtitle(1);
    });
    expect(
      result.current.subtitleTracks.find((t) => t.id === 1)?.is_selected
    ).toBe(true);

    await act(async () => {
      await result.current.selectSubtitle(-1);
    });
    expect(
      result.current.subtitleTracks.some((t) => t.is_selected)
    ).toBe(false);
  });

  it('loads external subtitle track', async () => {
    const { result } = renderHook(() => useTracks());

    let track: any;
    await act(async () => {
      track = await result.current.loadExternalSubtitle('/subs/spanish.srt');
    });

    expect(track).toBeDefined();
    expect(track.is_external).toBe(true);
    expect(track.is_selected).toBe(true);
  });
});
