import React, { useEffect, useState } from 'react';
import { Maximize2, Pause, Play, RotateCcw, RotateCw } from 'lucide-react';
import { hasTauri, windowApi } from '../../services/ipc';

export interface MiniPlayerOverlayProps {
  isPlaying: boolean;
  onTogglePlayPause: () => void;
  onSeekRelative: (seconds: number) => void;
  onExpand: () => void;
}

const SEEK_SECONDS = 10;
const HOVER_POLL_MS = 150;

/**
 * Mini player surface: the borderless window is just the picture. Drag anywhere to move
 * it, double-click to expand; controls appear while the pointer is over the window.
 */
export function MiniPlayerOverlay({ isPlaying, onTogglePlayPause, onSeekRelative, onExpand }: MiniPlayerOverlayProps) {
  const [hovered, setHovered] = useState(false);

  // While another app is active, macOS sends the page no hover events: ask the window.
  useEffect(() => {
    if (!hasTauri()) return;
    let stopped = false;
    const timer = window.setInterval(() => {
      windowApi.isCursorInside().then((inside) => { if (!stopped) setHovered(inside); }).catch(() => {});
    }, HOVER_POLL_MS);
    return () => { stopped = true; window.clearInterval(timer); };
  }, []);

  const onMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest('button')) return;
    if (e.detail >= 2) {
      onExpand();
      return;
    }
    if (hasTauri()) {
      void import('@tauri-apps/api/webviewWindow')
        .then(({ getCurrentWebviewWindow }) => getCurrentWebviewWindow().startDragging())
        .catch(() => {});
    }
  };

  const button =
    'pointer-events-auto p-2 rounded-full bg-black/55 hover:bg-black/80 text-white transition-colors';
  return (
    <div
      data-testid="mini-player-overlay"
      className="absolute inset-0 z-40 select-none cursor-default"
      onMouseDown={onMouseDown}
      onMouseEnter={() => setHovered(true)}
      onMouseMove={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <div className={`absolute inset-0 transition-opacity duration-150 ${hovered ? 'opacity-100' : 'opacity-0 pointer-events-none'}`}>
        <div className="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-black/60 to-transparent pointer-events-none" />
        <button onClick={onExpand} aria-label="Expand player" title="Expand (double-click or Esc)" data-testid="mini-expand-btn" className={`${button} absolute top-2 right-2`}>
          <Maximize2 className="w-4 h-4" />
        </button>
        <div className="absolute bottom-2 inset-x-0 flex items-center justify-center gap-3">
          <button onClick={() => onSeekRelative(-SEEK_SECONDS)} aria-label={`Back ${SEEK_SECONDS} seconds`} data-testid="mini-back-btn" className={button}>
            <RotateCcw className="w-4 h-4" />
          </button>
          <button onClick={onTogglePlayPause} aria-label={isPlaying ? 'Pause' : 'Play'} data-testid="mini-play-btn" className={`${button} p-2.5`}>
            {isPlaying ? <Pause className="w-5 h-5" /> : <Play className="w-5 h-5" />}
          </button>
          <button onClick={() => onSeekRelative(SEEK_SECONDS)} aria-label={`Forward ${SEEK_SECONDS} seconds`} data-testid="mini-forward-btn" className={button}>
            <RotateCw className="w-4 h-4" />
          </button>
        </div>
      </div>
    </div>
  );
}
