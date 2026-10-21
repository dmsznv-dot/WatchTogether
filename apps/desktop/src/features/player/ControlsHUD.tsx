import React from 'react';
import {
  Play,
  Pause,
  RotateCcw,
  RotateCw,
  Maximize2,
  Minimize2,
  Lock,
} from 'lucide-react';
import { Scrubber } from './Scrubber';
import { VolumeControl } from './VolumeControl';
import { SpeedSelector } from './SpeedSelector';
import { TrackSelector } from './TrackSelector';
import { MediaTrack, PlaybackSpeed } from '../../types/player';
import { formatTimecode } from '../../utils/timecode';
import { cn } from '../../utils/cn';

export interface ControlsHUDProps {
  isVisible: boolean;
  isPlaying: boolean;
  positionSec: number;
  durationSec: number;
  bufferedSec?: number;
  speed: PlaybackSpeed;
  volume: number;
  isMuted: boolean;
  isFullscreen: boolean;
  isScrubbing: boolean;
  canControlPlayback: boolean;
  audioTracks: MediaTrack[];
  subtitleTracks: MediaTrack[];
  onTogglePlayPause: () => void;
  onSeekRelative: (deltaSeconds: number) => void;
  onStartScrub: (initialSec: number) => void;
  onUpdateScrub: (sec: number) => void;
  onCommitScrub: () => void;
  onSelectSpeed: (speed: PlaybackSpeed) => void;
  onVolumeChange: (newVolume: number) => void;
  onMuteToggle: () => void;
  onSelectAudio: (trackId: number) => void;
  onSelectSubtitle: (trackId: number) => void;
  onLoadExternalSubtitle: (path: string) => void;
  onToggleFullscreen: () => void;
  onUnauthorizedAction?: () => void;
  bindHover?: {
    onMouseEnter: () => void;
    onMouseLeave: () => void;
  };
}

export function ControlsHUD({
  isVisible,
  isPlaying,
  positionSec,
  durationSec,
  bufferedSec,
  speed,
  volume,
  isMuted,
  isFullscreen,
  isScrubbing,
  canControlPlayback,
  audioTracks,
  subtitleTracks,
  onTogglePlayPause,
  onSeekRelative,
  onStartScrub,
  onUpdateScrub,
  onCommitScrub,
  onSelectSpeed,
  onVolumeChange,
  onMuteToggle,
  onSelectAudio,
  onSelectSubtitle,
  onLoadExternalSubtitle,
  onToggleFullscreen,
  onUnauthorizedAction,
  bindHover,
}: ControlsHUDProps) {
  const needsHours = durationSec >= 3600;
  const formattedPosition = formatTimecode(positionSec, needsHours);
  const formattedDuration = formatTimecode(durationSec, needsHours);

  const handlePlayPauseClick = () => {
    if (!canControlPlayback) {
      onUnauthorizedAction?.();
      return;
    }
    onTogglePlayPause();
  };

  const handleSeekClick = (delta: number) => {
    if (!canControlPlayback) {
      onUnauthorizedAction?.();
      return;
    }
    onSeekRelative(delta);
  };

  return (
    <div
      {...bindHover}
      data-testid="controls-hud"
      className={cn(
        'absolute bottom-0 inset-x-0 p-4 transition-all duration-300 z-30 select-none pointer-events-auto',
        isVisible ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-3 pointer-events-none'
      )}
    >
      <div className="max-w-5xl mx-auto rounded-2xl glass-panel p-3 shadow-2xl border border-white/10 flex flex-col gap-2">
        {/* Row 1: Interactive Scrubber Seekbar */}
        <div className="w-full px-1">
          <Scrubber
            positionSec={positionSec}
            durationSec={durationSec}
            bufferedSec={bufferedSec}
            isScrubbing={isScrubbing}
            disabled={!canControlPlayback}
            onStartScrub={(sec) => {
              if (!canControlPlayback) {
                onUnauthorizedAction?.();
                return;
              }
              onStartScrub(sec);
            }}
            onUpdateScrub={onUpdateScrub}
            onCommitScrub={onCommitScrub}
          />
        </div>

        {/* Row 2: Media Controls Bar */}
        <div className="flex items-center justify-between gap-3 text-zinc-100">
          {/* Left: Playback Controls & Timecode */}
          <div className="flex items-center gap-2">
            {/* Play/Pause Button */}
            <button
              onClick={handlePlayPauseClick}
              aria-label={isPlaying ? 'Pause' : 'Play'}
              data-testid="play-pause-btn"
              className={cn(
                'p-2 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white shadow-lg transition-transform active:scale-95 flex items-center justify-center',
                !canControlPlayback && 'opacity-60 cursor-not-allowed'
              )}
            >
              {!canControlPlayback ? (
                <Lock className="w-5 h-5 text-amber-300" />
              ) : isPlaying ? (
                <Pause className="w-5 h-5 fill-white" />
              ) : (
                <Play className="w-5 h-5 fill-white ml-0.5" />
              )}
            </button>

            {/* Seek -5s Button */}
            <button
              onClick={() => handleSeekClick(-5)}
              aria-label="Seek back 5 seconds"
              data-testid="seek-backward-btn"
              className="p-1.5 rounded-lg text-zinc-300 hover:text-white hover:bg-white/10 transition-colors"
            >
              <RotateCcw className="w-4 h-4" />
            </button>

            {/* Seek +5s Button */}
            <button
              onClick={() => handleSeekClick(5)}
              aria-label="Seek forward 5 seconds"
              data-testid="seek-forward-btn"
              className="p-1.5 rounded-lg text-zinc-300 hover:text-white hover:bg-white/10 transition-colors"
            >
              <RotateCw className="w-4 h-4" />
            </button>

            {/* Timecode Display */}
            <div
              data-testid="timecode-display"
              className="font-mono tabular-nums text-xs sm:text-sm text-zinc-300 ml-2"
            >
              <span className="text-zinc-100 font-medium">{formattedPosition}</span>
              <span className="text-zinc-500 mx-1">/</span>
              <span className="text-zinc-400">{formattedDuration}</span>
            </div>
          </div>

          {/* Right: Tools, Tracks, Volume & Fullscreen */}
          <div className="flex items-center gap-2.5">
            {/* Speed Selector */}
            <SpeedSelector
              speed={speed}
              isHostControlled={!canControlPlayback}
              onSelectSpeed={onSelectSpeed}
              onUnauthorizedClick={onUnauthorizedAction}
            />

            {/* Audio & Subtitle Selectors */}
            <TrackSelector
              audioTracks={audioTracks}
              subtitleTracks={subtitleTracks}
              onSelectAudio={onSelectAudio}
              onSelectSubtitle={onSelectSubtitle}
              onLoadExternalSubtitle={onLoadExternalSubtitle}
            />

            {/* Local Unsynchronized Volume */}
            <VolumeControl
              volume={volume}
              isMuted={isMuted}
              onVolumeChange={onVolumeChange}
              onMuteToggle={onMuteToggle}
            />

            {/* Fullscreen Button */}
            <button
              onClick={onToggleFullscreen}
              aria-label={isFullscreen ? 'Exit Fullscreen' : 'Enter Fullscreen'}
              data-testid="fullscreen-btn"
              className="p-1.5 rounded-lg text-zinc-300 hover:text-white hover:bg-white/10 transition-colors"
            >
              {isFullscreen ? (
                <Minimize2 className="w-4 h-4" />
              ) : (
                <Maximize2 className="w-4 h-4" />
              )}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
