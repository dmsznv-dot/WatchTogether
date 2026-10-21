import React, { useState, useRef, useEffect } from 'react';
import { Check, Gauge, Lock } from 'lucide-react';
import { PlaybackSpeed, PLAYBACK_SPEEDS } from '../../types/player';
import { cn } from '../../utils/cn';

export interface SpeedSelectorProps {
  speed: PlaybackSpeed;
  disabled?: boolean;
  isHostControlled?: boolean;
  onSelectSpeed: (speed: PlaybackSpeed) => void;
  onUnauthorizedClick?: () => void;
}

export function SpeedSelector({
  speed,
  disabled = false,
  isHostControlled = false,
  onSelectSpeed,
  onUnauthorizedClick,
}: SpeedSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setIsOpen(false);
      }
    };
    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen]);

  const handleToggle = () => {
    if (isHostControlled) {
      onUnauthorizedClick?.();
      return;
    }
    if (disabled) return;
    setIsOpen(!isOpen);
  };

  return (
    <div ref={containerRef} className="relative inline-block text-left" data-testid="speed-selector">
      <button
        onClick={handleToggle}
        aria-label="Playback speed"
        data-testid="speed-selector-btn"
        className={cn(
          'flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-mono font-medium border transition-colors',
          isOpen
            ? 'bg-indigo-600 text-white border-indigo-500'
            : 'bg-zinc-800/80 hover:bg-zinc-700/80 text-zinc-300 hover:text-white border-zinc-700/60',
          isHostControlled && 'opacity-60 cursor-not-allowed'
        )}
      >
        {isHostControlled ? (
          <Lock className="w-3.5 h-3.5 text-amber-400" />
        ) : (
          <Gauge className="w-3.5 h-3.5 text-zinc-400" />
        )}
        <span>{speed}×</span>
      </button>

      {/* Speed Dropdown Menu */}
      {isOpen && (
        <div
          data-testid="speed-dropdown"
          className="absolute bottom-full mb-2 right-0 w-32 rounded-xl glass-dropdown p-1.5 z-40 animate-in fade-in zoom-in-95 duration-150"
        >
          <div className="text-[10px] font-semibold text-zinc-400 uppercase tracking-wider px-2 py-1">
            Speed
          </div>
          <div className="flex flex-col gap-0.5">
            {PLAYBACK_SPEEDS.map((s) => {
              const isSelected = s === speed;
              return (
                <button
                  key={s}
                  onClick={() => {
                    onSelectSpeed(s);
                    setIsOpen(false);
                  }}
                  data-testid={`speed-option-${s}`}
                  className={cn(
                    'w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-xs font-mono transition-colors text-left',
                    isSelected
                      ? 'bg-indigo-600 text-white font-bold'
                      : 'text-zinc-300 hover:text-white hover:bg-zinc-700/60'
                  )}
                >
                  <span>{s}×</span>
                  {isSelected && <Check className="w-3.5 h-3.5" />}
                </button>
              );
            })}
          </div>
        </div>
      )}
    </div>
  );
}
