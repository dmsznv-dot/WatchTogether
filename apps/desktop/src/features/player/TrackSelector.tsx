import React, { useState, useRef, useEffect } from 'react';
import { Headphones, Subtitles, Check, Plus } from 'lucide-react';
import { MediaTrack } from '../../types/player';
import { windowApi } from '../../services/ipc';
import { cn } from '../../utils/cn';

export interface TrackSelectorProps {
  audioTracks: MediaTrack[];
  subtitleTracks: MediaTrack[];
  onSelectAudio: (trackId: number) => void;
  onSelectSubtitle: (trackId: number) => void;
  onLoadExternalSubtitle: (path: string) => void;
}

export function TrackSelector({
  audioTracks,
  subtitleTracks,
  onSelectAudio,
  onSelectSubtitle,
  onLoadExternalSubtitle,
}: TrackSelectorProps) {
  const [activeMenu, setActiveMenu] = useState<'audio' | 'subtitle' | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setActiveMenu(null);
      }
    };
    if (activeMenu) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [activeMenu]);

  const handleOpenExternalSubtitle = async () => {
    const selected = await windowApi.openFileDialog('subtitle');
    if (selected) {
      onLoadExternalSubtitle(selected);
      setActiveMenu(null);
    }
  };

  const selectedSubtitle = subtitleTracks.find((t) => t.is_selected);
  const isSubtitleOff = !selectedSubtitle || selectedSubtitle.id === -1;

  return (
    <div ref={containerRef} className="flex items-center gap-1.5 relative">
      {/* Audio Selector Button */}
      <button
        onClick={() => setActiveMenu(activeMenu === 'audio' ? null : 'audio')}
        aria-label="Audio tracks"
        data-testid="audio-tracks-btn"
        className={cn(
          'p-1.5 rounded-lg border transition-colors',
          activeMenu === 'audio'
            ? 'bg-indigo-600 text-white border-indigo-500'
            : 'bg-zinc-800/80 hover:bg-zinc-700/80 text-zinc-300 hover:text-white border-zinc-700/60'
        )}
      >
        <Headphones className="w-4 h-4" />
      </button>

      {/* Subtitle Selector Button */}
      <button
        onClick={() => setActiveMenu(activeMenu === 'subtitle' ? null : 'subtitle')}
        aria-label="Subtitle tracks"
        data-testid="subtitle-tracks-btn"
        className={cn(
          'p-1.5 rounded-lg border transition-colors relative',
          activeMenu === 'subtitle'
            ? 'bg-indigo-600 text-white border-indigo-500'
            : 'bg-zinc-800/80 hover:bg-zinc-700/80 text-zinc-300 hover:text-white border-zinc-700/60',
          !isSubtitleOff && 'text-indigo-400'
        )}
      >
        <Subtitles className="w-4 h-4" />
        {!isSubtitleOff && (
          <span className="absolute bottom-1 right-1 w-1.5 h-1.5 rounded-full bg-indigo-400" />
        )}
      </button>

      {/* Audio Menu Dropdown */}
      {activeMenu === 'audio' && (
        <div
          data-testid="audio-dropdown"
          className="absolute bottom-full mb-2 right-10 w-56 rounded-xl glass-dropdown p-1.5 z-40 animate-in fade-in zoom-in-95 duration-150"
        >
          <div className="text-[10px] font-semibold text-zinc-400 uppercase tracking-wider px-2 py-1">
            Audio Tracks
          </div>
          <div className="flex flex-col gap-0.5 max-h-52 overflow-y-auto">
            {audioTracks.length === 0 ? (
              <div className="text-xs text-zinc-500 px-2 py-1.5 italic">
                No audio tracks
              </div>
            ) : (
              audioTracks.map((track) => {
                const label =
                  track.title ||
                  (track.lang ? track.lang.toUpperCase() : `Audio Track ${track.id}`);
                return (
                  <button
                    key={track.id}
                    onClick={() => {
                      onSelectAudio(track.id);
                      setActiveMenu(null);
                    }}
                    data-testid={`audio-track-${track.id}`}
                    className={cn(
                      'w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-xs transition-colors text-left',
                      track.is_selected
                        ? 'bg-indigo-600 text-white font-medium'
                        : 'text-zinc-300 hover:text-white hover:bg-zinc-700/60'
                    )}
                  >
                    <div className="truncate mr-2">
                      <span>{label}</span>
                      {track.codec && (
                        <span className="ml-1.5 text-[10px] uppercase text-zinc-400 bg-zinc-800 px-1 py-0.5 rounded">
                          {track.codec}
                        </span>
                      )}
                    </div>
                    {track.is_selected && <Check className="w-3.5 h-3.5 shrink-0" />}
                  </button>
                );
              })
            )}
          </div>
        </div>
      )}

      {/* Subtitle Menu Dropdown */}
      {activeMenu === 'subtitle' && (
        <div
          data-testid="subtitle-dropdown"
          className="absolute bottom-full mb-2 right-0 w-60 rounded-xl glass-dropdown p-1.5 z-40 animate-in fade-in zoom-in-95 duration-150"
        >
          <div className="text-[10px] font-semibold text-zinc-400 uppercase tracking-wider px-2 py-1">
            Subtitles
          </div>

          <div className="flex flex-col gap-0.5 max-h-56 overflow-y-auto">
            {/* Off Option */}
            <button
              onClick={() => {
                onSelectSubtitle(-1);
                setActiveMenu(null);
              }}
              data-testid="subtitle-track-off"
              className={cn(
                'w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-xs transition-colors text-left',
                isSubtitleOff
                  ? 'bg-indigo-600 text-white font-medium'
                  : 'text-zinc-300 hover:text-white hover:bg-zinc-700/60'
              )}
            >
              <span>Off</span>
              {isSubtitleOff && <Check className="w-3.5 h-3.5" />}
            </button>

            {subtitleTracks.map((track) => {
              const label =
                track.title ||
                (track.lang ? track.lang.toUpperCase() : `Subtitle ${track.id}`);
              return (
                <button
                  key={track.id}
                  onClick={() => {
                    onSelectSubtitle(track.id);
                    setActiveMenu(null);
                  }}
                  data-testid={`subtitle-track-${track.id}`}
                  className={cn(
                    'w-full flex items-center justify-between px-2.5 py-1.5 rounded-lg text-xs transition-colors text-left',
                    track.is_selected
                      ? 'bg-indigo-600 text-white font-medium'
                      : 'text-zinc-300 hover:text-white hover:bg-zinc-700/60'
                  )}
                >
                  <div className="truncate mr-2 flex items-center gap-1.5">
                    <span>{label}</span>
                    {track.is_external && (
                      <span className="text-[9px] uppercase tracking-wider text-amber-300 bg-amber-950/80 px-1 py-0.2 rounded border border-amber-800/40">
                        External
                      </span>
                    )}
                  </div>
                  {track.is_selected && <Check className="w-3.5 h-3.5 shrink-0" />}
                </button>
              );
            })}

            <div className="my-1 border-t border-zinc-700/50" />

            {/* Add External Subtitle Option */}
            <button
              onClick={handleOpenExternalSubtitle}
              data-testid="add-external-subtitle-btn"
              className="w-full flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-xs text-indigo-400 hover:text-indigo-300 hover:bg-indigo-950/40 transition-colors text-left font-medium"
            >
              <Plus className="w-3.5 h-3.5" />
              <span>Add External Subtitle...</span>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
