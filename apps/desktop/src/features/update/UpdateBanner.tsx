import React from 'react';
import { Download, X } from 'lucide-react';
import { UpdateStatus } from '../../hooks/useUpdater';

export interface UpdateBannerProps {
  status: UpdateStatus;
  onInstall: () => void;
  onDismiss: () => void;
}

/** Small pill at the top of the window; stays visible while the HUD auto-hides. */
export function UpdateBanner({ status, onInstall, onDismiss }: UpdateBannerProps) {
  if (status.state === 'idle') return null;
  return (
    <div
      role="status"
      data-testid="update-banner"
      className="absolute top-3 left-1/2 -translate-x-1/2 z-50 flex items-center gap-3 px-4 py-2 rounded-full bg-indigo-950/95 border border-indigo-500/60 text-sm text-indigo-100 shadow-xl"
    >
      <Download className="w-4 h-4 text-indigo-300 shrink-0" />
      {status.state === 'available' && (
        <>
          <span>Update available: version {status.version}</span>
          <button
            onClick={onInstall}
            data-testid="update-install-btn"
            className="px-3 py-1 rounded-full bg-indigo-500 hover:bg-indigo-400 text-white text-xs font-semibold"
          >
            Update & restart
          </button>
        </>
      )}
      {status.state === 'downloading' && (
        <span>Downloading update{status.percent !== null ? ` ${status.percent}%` : '…'} The app restarts when it is done.</span>
      )}
      {status.state === 'error' && (
        <>
          <span className="text-rose-200">Update failed: {status.message}</span>
          <button onClick={onInstall} className="px-3 py-1 rounded-full bg-indigo-500 hover:bg-indigo-400 text-white text-xs font-semibold">
            Retry
          </button>
        </>
      )}
      {status.state !== 'downloading' && (
        <button onClick={onDismiss} aria-label="Dismiss update" data-testid="update-dismiss-btn" className="text-indigo-300 hover:text-white">
          <X className="w-4 h-4" />
        </button>
      )}
    </div>
  );
}
