import React, { ChangeEvent } from 'react';
import { Volume2, Volume1, VolumeX } from 'lucide-react';
import { cn } from '../../utils/cn';

export interface VolumeControlProps {
  volume: number;
  isMuted: boolean;
  onVolumeChange: (newVolume: number) => void;
  onMuteToggle: () => void;
}

export function VolumeControl({
  volume,
  isMuted,
  onVolumeChange,
  onMuteToggle,
}: VolumeControlProps) {
  const currentLevel = isMuted ? 0 : volume;

  const handleSliderChange = (e: ChangeEvent<HTMLInputElement>) => {
    const nextVal = parseFloat(e.target.value);
    onVolumeChange(nextVal);
  };

  const renderIcon = () => {
    if (isMuted || volume === 0) {
      return <VolumeX className="w-5 h-5 text-zinc-400 group-hover:text-white" />;
    }
    if (volume <= 0.5) {
      return <Volume1 className="w-5 h-5 text-zinc-300 group-hover:text-white" />;
    }
    return <Volume2 className="w-5 h-5 text-zinc-200 group-hover:text-white" />;
  };

  return (
    <div
      data-testid="volume-control"
      className="flex items-center gap-2 group/vol"
    >
      <button
        onClick={onMuteToggle}
        className="p-1.5 rounded-lg text-zinc-300 hover:text-white hover:bg-white/10 transition-colors"
        aria-label={isMuted ? 'Unmute' : 'Mute'}
        data-testid="mute-toggle-btn"
      >
        {renderIcon()}
      </button>

      <div className="w-20 sm:w-24 flex items-center">
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          value={currentLevel}
          onChange={handleSliderChange}
          aria-label="Volume slider"
          data-testid="volume-slider"
          style={{
            background: `linear-gradient(to right, #6366F1 ${currentLevel * 100}%, #374151 ${currentLevel * 100}%)`,
          }}
          className="w-full h-1.5 rounded-full appearance-none bg-zinc-700 cursor-pointer accent-indigo-500 focus:outline-none"
        />
      </div>
    </div>
  );
}
