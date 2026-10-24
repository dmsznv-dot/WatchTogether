import { useCallback, useEffect, useRef, useState } from 'react';
import { hasTauri, isNativeDialogOpen, windowApi } from '../services/ipc';

export interface MiniPlayerOptions {
  /** User setting: shrink into the mini player when switching to another app. */
  enabled: boolean;
  isPlaying: boolean;
  hasVideo: boolean;
}

/**
 * Picture-in-picture: when the window loses focus (Alt+Tab, Cmd+Tab, clicking another
 * app) while a video plays, it becomes a small always-on-top player in the screen
 * corner. Focusing it again does not expand it, so it can be moved and resized;
 * `exit` (double-click, the expand button, Esc) restores the full window.
 */
export function useMiniPlayer({ enabled, isPlaying, hasVideo }: MiniPlayerOptions) {
  const [isMini, setIsMini] = useState(false);
  const latest = useRef({ enabled, isPlaying, hasVideo, isMini });
  latest.current = { enabled, isPlaying, hasVideo, isMini };

  const enter = useCallback(async () => {
    try {
      setIsMini(await windowApi.setMini(true));
    } catch (err) {
      console.warn('Mini player unavailable:', err);
    }
  }, []);

  const exit = useCallback(async () => {
    try {
      await windowApi.setMini(false);
    } finally {
      setIsMini(false);
    }
  }, []);

  useEffect(() => {
    if (!hasTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    import('@tauri-apps/api/webviewWindow')
      .then(({ getCurrentWebviewWindow }) =>
        getCurrentWebviewWindow().onFocusChanged(({ payload: focused }) => {
          const s = latest.current;
          if (!focused && s.enabled && s.isPlaying && s.hasVideo && !s.isMini && !isNativeDialogOpen()) {
            void enter();
          }
        })
      )
      .then((fn) => { if (disposed) fn(); else unlisten = fn; })
      .catch((err) => console.warn('Focus tracking unavailable:', err));
    return () => { disposed = true; unlisten?.(); };
  }, [enter]);

  return { isMini, enter, exit };
}
