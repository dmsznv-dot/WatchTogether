import React, { useState, useRef, useCallback, useEffect, MouseEvent } from 'react';
import { formatTimecode } from '../../utils/timecode';
import { cn } from '../../utils/cn';

export interface ScrubberProps {
  positionSec: number;
  durationSec: number;
  bufferedSec?: number;
  isScrubbing: boolean;
  disabled?: boolean;
  onStartScrub: (initialSec: number) => void;
  onUpdateScrub: (sec: number) => void;
  onCommitScrub: () => void;
}

export function Scrubber({
  positionSec,
  durationSec,
  bufferedSec,
  isScrubbing,
  disabled = false,
  onStartScrub,
  onUpdateScrub,
  onCommitScrub,
}: ScrubberProps) {
  const barRef = useRef<HTMLDivElement>(null);
  const [hoverX, setHoverX] = useState<number | null>(null);
  const [hoverSec, setHoverSec] = useState<number>(0);

  const playedPercent = durationSec > 0 ? Math.min(100, Math.max(0, (positionSec / durationSec) * 100)) : 0;
  const bufferedPercent =
    durationSec > 0
      ? Math.min(100, Math.max(0, ((bufferedSec ?? durationSec) / durationSec) * 100))
      : 0;

  const calculatePositionFromClientX = useCallback(
    (clientX: number): number => {
      if (!barRef.current || durationSec <= 0) return 0;
      const rect = barRef.current.getBoundingClientRect();
      const clickX = Math.max(0, Math.min(rect.width, clientX - rect.left));
      const ratio = clickX / rect.width;
      return ratio * durationSec;
    },
    [durationSec]
  );

  const handleMouseMove = (e: MouseEvent<HTMLDivElement>) => {
    if (disabled || durationSec <= 0 || !barRef.current) return;
    const rect = barRef.current.getBoundingClientRect();
    const x = Math.max(0, Math.min(rect.width, e.clientX - rect.left));
    setHoverX(x);
    setHoverSec((x / rect.width) * durationSec);
  };

  const handleMouseLeave = () => {
    setHoverX(null);
  };

  const handleMouseDown = (e: MouseEvent<HTMLDivElement>) => {
    if (disabled || durationSec <= 0) return;
    e.preventDefault();
    const targetSec = calculatePositionFromClientX(e.clientX);
    onStartScrub(targetSec);
  };

  useEffect(() => {
    if (!isScrubbing) return;

    const handleWindowMouseMove = (e: globalThis.MouseEvent) => {
      const targetSec = calculatePositionFromClientX(e.clientX);
      onUpdateScrub(targetSec);
      if (barRef.current) {
        const rect = barRef.current.getBoundingClientRect();
        const x = Math.max(0, Math.min(rect.width, e.clientX - rect.left));
        setHoverX(x);
        setHoverSec(targetSec);
      }
    };

    const handleWindowMouseUp = () => {
      onCommitScrub();
    };

    window.addEventListener('mousemove', handleWindowMouseMove);
    window.addEventListener('mouseup', handleWindowMouseUp);

    return () => {
      window.removeEventListener('mousemove', handleWindowMouseMove);
      window.removeEventListener('mouseup', handleWindowMouseUp);
    };
  }, [isScrubbing, calculatePositionFromClientX, onUpdateScrub, onCommitScrub]);

  return (
    <div
      data-testid="timeline-scrubber"
      className={cn(
        'relative w-full py-2 group cursor-pointer select-none',
        disabled && 'opacity-50 cursor-not-allowed'
      )}
      onMouseMove={handleMouseMove}
      onMouseLeave={handleMouseLeave}
      onMouseDown={handleMouseDown}
    >
      {/* Track Container */}
      <div
        ref={barRef}
        className="relative h-1.5 group-hover:h-2.5 transition-all w-full bg-zinc-800/80 rounded-full overflow-hidden"
      >
        {/* Buffered Progress */}
        <div
          data-testid="scrubber-buffered"
          style={{ width: `${bufferedPercent}%` }}
          className="absolute inset-y-0 left-0 bg-zinc-600/60 rounded-full transition-all"
        />

        {/* Played Progress */}
        <div
          data-testid="scrubber-played"
          style={{ width: `${playedPercent}%` }}
          className="absolute inset-y-0 left-0 bg-indigo-500 rounded-full transition-all"
        />
      </div>

      {/* Scrubber Thumb */}
      <div
        data-testid="scrubber-thumb"
        style={{ left: `${playedPercent}%` }}
        className={cn(
          'absolute top-1/2 -translate-y-1/2 -translate-x-1/2 w-3.5 h-3.5 rounded-full bg-white shadow-md border border-indigo-300 pointer-events-none transition-transform',
          isScrubbing || hoverX !== null ? 'scale-100 opacity-100' : 'scale-0 opacity-0 group-hover:scale-100 group-hover:opacity-100'
        )}
      />

      {/* Hover Timecode Preview Bubble */}
      {hoverX !== null && durationSec > 0 && barRef.current && (
        <div
          data-testid="scrubber-hover-preview"
          style={{
            left: `${Math.max(30, Math.min((barRef.current?.getBoundingClientRect().width || 100) - 30, hoverX))}px`,
          }}
          className="absolute bottom-6 -translate-x-1/2 px-2 py-0.5 rounded-md bg-zinc-900/90 text-zinc-100 text-[11px] font-mono tabular-nums border border-zinc-700 shadow-xl pointer-events-none z-30"
        >
          {formatTimecode(hoverSec, durationSec >= 3600)}
        </div>
      )}
    </div>
  );
}
