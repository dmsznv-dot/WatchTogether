import React from 'react';
import { Play, Pause } from 'lucide-react';

export interface PlayPauseRippleProps {
  type: 'play' | 'pause' | null;
}

export function PlayPauseRipple({ type }: PlayPauseRippleProps) {
  if (!type) return null;

  return (
    <div
      key={`${type}-${Date.now()}`}
      className="pointer-events-none absolute inset-0 flex items-center justify-center z-20"
      data-testid="play-pause-ripple"
    >
      <div className="flex items-center justify-center w-24 h-24 rounded-full bg-black/60 backdrop-blur-md border border-white/20 text-white shadow-2xl animate-ripple">
        {type === 'play' ? (
          <Play className="w-12 h-12 fill-white ml-1.5" />
        ) : (
          <Pause className="w-12 h-12 fill-white" />
        )}
      </div>
    </div>
  );
}
