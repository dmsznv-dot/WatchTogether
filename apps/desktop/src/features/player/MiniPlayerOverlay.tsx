import React from 'react';
import { Maximize2 } from 'lucide-react';

/** Expand button shown over the video in the mini player; appears on hover. */
export function MiniPlayerOverlay({ onExpand }: { onExpand: () => void }) {
  return (
    <div className="absolute inset-0 z-40 pointer-events-none group">
      <button
        onClick={onExpand}
        aria-label="Expand player"
        title="Expand (double-click or Esc)"
        data-testid="mini-expand-btn"
        className="pointer-events-auto absolute top-2 right-2 p-1.5 rounded-lg bg-black/60 text-zinc-100 opacity-0 group-hover:opacity-100 focus:opacity-100 hover:bg-black/80 transition-opacity"
      >
        <Maximize2 className="w-4 h-4" />
      </button>
    </div>
  );
}
