import { useCallback, useEffect, useRef, useState } from 'react';
import { hasTauri } from '../services/ipc';

export type UpdateStatus =
  | { state: 'idle' }
  | { state: 'available'; version: string }
  | { state: 'downloading'; percent: number | null }
  | { state: 'error'; message: string };

const FIRST_CHECK_DELAY_MS = 5_000;
const CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000;

type PendingUpdate = Awaited<ReturnType<typeof import('@tauri-apps/plugin-updater').check>>;

/**
 * Checks GitHub Releases (the `updates/latest.json` manifest) for a newer, signed build.
 * Offline or failed checks stay silent; only an available update is shown.
 */
export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus>({ state: 'idle' });
  const pending = useRef<PendingUpdate>(null);

  const checkNow = useCallback(async () => {
    if (!hasTauri()) return;
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const update = await check();
      if (!update) return;
      pending.current = update;
      setStatus((current) => (current.state === 'downloading' ? current : { state: 'available', version: update.version }));
    } catch (err) {
      console.warn('Update check failed:', err);
    }
  }, []);

  useEffect(() => {
    const first = window.setTimeout(checkNow, FIRST_CHECK_DELAY_MS);
    const every = window.setInterval(checkNow, CHECK_INTERVAL_MS);
    return () => { window.clearTimeout(first); window.clearInterval(every); };
  }, [checkNow]);

  const install = useCallback(async () => {
    const update = pending.current;
    if (!update) return;
    let total = 0;
    let received = 0;
    setStatus({ state: 'downloading', percent: null });
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') total = event.data.contentLength ?? 0;
        if (event.event === 'Progress') {
          received += event.data.chunkLength;
          setStatus({ state: 'downloading', percent: total ? Math.min(100, Math.round((received / total) * 100)) : null });
        }
      });
      const { relaunch } = await import('@tauri-apps/plugin-process');
      await relaunch();
    } catch (err) {
      setStatus({ state: 'error', message: String(err) });
    }
  }, []);

  const dismiss = useCallback(() => setStatus({ state: 'idle' }), []);

  return { status, install, dismiss, checkNow };
}
