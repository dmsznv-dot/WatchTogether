import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';

let focusListener: ((event: { payload: boolean }) => void) | undefined;
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    onFocusChanged: async (cb: (event: { payload: boolean }) => void) => { focusListener = cb; return () => {}; },
  }),
}));
const setMini = vi.fn(async (enabled: boolean) => enabled);
let dialogOpen = false;
vi.mock('../services/ipc', async (original) => ({
  ...(await original<object>()),
  hasTauri: () => true,
  isNativeDialogOpen: () => dialogOpen,
  windowApi: { setMini: (enabled: boolean) => setMini(enabled) },
}));

import { useMiniPlayer } from '../hooks/useMiniPlayer';

const blur = async () => { await act(async () => { focusListener?.({ payload: false }); }); };

describe('useMiniPlayer', () => {
  beforeEach(() => { focusListener = undefined; setMini.mockClear(); dialogOpen = false; });

  it('turns into the mini player when the window loses focus during playback', async () => {
    const { result } = renderHook(() => useMiniPlayer({ enabled: true, isPlaying: true, hasVideo: true }));
    await waitFor(() => expect(focusListener).toBeDefined());
    await blur();
    expect(setMini).toHaveBeenCalledWith(true);
    expect(result.current.isMini).toBe(true);
    // Coming back to the mini window keeps it small so it can be moved and resized.
    await act(async () => { focusListener?.({ payload: true }); });
    expect(result.current.isMini).toBe(true);
    await act(async () => { await result.current.exit(); });
    expect(setMini).toHaveBeenLastCalledWith(false);
    expect(result.current.isMini).toBe(false);
  });

  it('stays a normal window when paused, disabled, without a video or for a file dialog', async () => {
    for (const options of [
      { enabled: true, isPlaying: false, hasVideo: true },
      { enabled: false, isPlaying: true, hasVideo: true },
      { enabled: true, isPlaying: true, hasVideo: false },
    ]) {
      focusListener = undefined;
      renderHook(() => useMiniPlayer(options));
      await waitFor(() => expect(focusListener).toBeDefined());
      await blur();
    }
    focusListener = undefined;
    dialogOpen = true;
    renderHook(() => useMiniPlayer({ enabled: true, isPlaying: true, hasVideo: true }));
    await waitFor(() => expect(focusListener).toBeDefined());
    await blur();
    expect(setMini).not.toHaveBeenCalled();
  });
});
