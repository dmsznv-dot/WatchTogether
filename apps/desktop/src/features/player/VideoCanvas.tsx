import React, { useState, useRef, useCallback, useEffect, DragEvent } from 'react';
import { Film, UploadCloud } from 'lucide-react';
import { Button } from '../../components/ui/Button';
import { PlayPauseRipple } from './PlayPauseRipple';
import { hasTauri } from '../../services/ipc';
import { cn } from '../../utils/cn';

export interface VideoCanvasProps {
  currentFile: string | null;
  isPlaying: boolean;
  aspectRatio?: number; // e.g. 16/9
  hideCursor?: boolean;
  onOpenFile: (path: string) => void;
  onLoadSubtitle?: (path: string) => void;
  onTogglePlayPause: () => void;
  onToggleFullscreen: () => void;
  onSelectFileClick: () => void;
}

export function VideoCanvas({
  currentFile,
  isPlaying,
  aspectRatio = 16 / 9,
  hideCursor = false,
  onOpenFile,
  onLoadSubtitle,
  onTogglePlayPause,
  onToggleFullscreen,
  onSelectFileClick,
}: VideoCanvasProps) {
  const native = hasTauri();
  const [isDragOver, setIsDragOver] = useState(false);
  const [rippleType, setRippleType] = useState<'play' | 'pause' | null>(null);
  const clickTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (!native) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    import('@tauri-apps/api/webviewWindow').then(({ getCurrentWebviewWindow }) =>
      getCurrentWebviewWindow().onDragDropEvent(({ payload }) => {
        if (disposed) return;
        setIsDragOver(payload.type === 'over' || payload.type === 'enter');
        if (payload.type === 'drop') {
          const path = payload.paths[0];
          if (!path) return;
          if (/\.(srt|ass)$/i.test(path)) onLoadSubtitle?.(path);
          else if (/\.(mp4|mov|mkv)$/i.test(path)) onOpenFile(path);
        }
      })
    ).then((fn) => { if (disposed) fn(); else unlisten = fn; })
      .catch((err) => console.error('Native drag and drop failed:', err));
    return () => { disposed = true; unlisten?.(); };
  }, [native, onOpenFile, onLoadSubtitle]);

  useEffect(() => () => {
    if (clickTimerRef.current !== null) window.clearTimeout(clickTimerRef.current);
  }, []);

  const triggerRipple = useCallback((type: 'play' | 'pause') => {
    setRippleType(type);
    window.setTimeout(() => setRippleType(null), 500);
  }, []);

  const handleClick = useCallback(() => {
    if (clickTimerRef.current !== null) {
      // Second click in quick succession -> double click = fullscreen
      window.clearTimeout(clickTimerRef.current);
      clickTimerRef.current = null;
      onToggleFullscreen();
    } else {
      // First click: wait 230ms to distinguish from double click
      clickTimerRef.current = window.setTimeout(() => {
        onTogglePlayPause();
        triggerRipple(isPlaying ? 'pause' : 'play');
        clickTimerRef.current = null;
      }, 230);
    }
  }, [isPlaying, onToggleFullscreen, onTogglePlayPause, triggerRipple]);

  const handleDragOver = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(true);
  };

  const handleDragLeave = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);
  };

  const handleDrop = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    setIsDragOver(false);

    if (e.dataTransfer.files && e.dataTransfer.files.length > 0) {
      const file = e.dataTransfer.files[0];
      const name = file.name.toLowerCase();
      // On web/mock, use name or file.name; in Tauri with drag-drop, path is available
      const filePath = (file as any).path || file.name;

      if (name.endsWith('.mp4') || name.endsWith('.mov') || name.endsWith('.mkv')) {
        onOpenFile(filePath);
      } else if (name.endsWith('.srt') || name.endsWith('.ass')) {
        if (onLoadSubtitle) {
          onLoadSubtitle(filePath);
        }
      }
    }
  };

  return (
    <div
      data-testid="video-canvas-container"
      onDragOver={handleDragOver}
      onDragLeave={handleDragLeave}
      onDrop={handleDrop}
      className={cn(
        'relative w-full h-full bg-black overflow-hidden flex items-center justify-center',
        native && currentFile && 'bg-transparent',
        hideCursor && 'cursor-none'
      )}
    >
      {/* Aspect Ratio Container (Letterbox / Pillarbox) */}
      <div
        data-testid="aspect-ratio-viewport"
        onClick={currentFile ? handleClick : undefined}
        style={{ aspectRatio }}
        className={cn("relative max-w-full max-h-full w-full h-full flex items-center justify-center select-none", native && currentFile ? "bg-transparent" : "bg-black")}
      >
        {!currentFile ? (
          // Empty State Hero
          <div className="flex flex-col items-center justify-center p-8 text-center max-w-md z-10">
            <div className="w-20 h-20 rounded-3xl bg-zinc-900/80 border border-zinc-700/60 flex items-center justify-center mb-6 shadow-2xl text-indigo-400">
              <Film className="w-10 h-10" />
            </div>
            <h1 className="text-2xl font-bold text-zinc-100 mb-2">No Video Loaded</h1>
            <p className="text-sm text-zinc-400 mb-6">
              Drag and drop an MP4, MOV, or MKV video file here, or click below to select a file.
            </p>
            <Button
              variant="primary"
              size="lg"
              onClick={(e) => {
                e.stopPropagation();
                onSelectFileClick();
              }}
            >
              Open Video File
            </Button>
          </div>
        ) : native ? (
          <div data-testid="media-surface" className="absolute inset-0 bg-transparent" aria-label="Video" />
        ) : (
          // Browser preview only
          <div
            data-testid="media-surface"
            className="w-full h-full relative flex items-center justify-center bg-zinc-950 overflow-hidden"
          >
            {/* Visual simulation of media content in mock/webview */}
            <div className="absolute inset-0 bg-gradient-to-tr from-zinc-950 via-zinc-900 to-indigo-950/40 opacity-70" />
            <div className="relative text-center p-6 pointer-events-none select-none max-w-lg">
              <div className="w-16 h-16 mx-auto mb-3 rounded-2xl bg-indigo-500/10 border border-indigo-500/20 flex items-center justify-center text-indigo-400">
                <Film className="w-8 h-8 animate-pulse" />
              </div>
              <p className="text-base font-semibold text-zinc-200 mb-1 truncate">
                {currentFile.split('/').pop() || currentFile}
              </p>
              <p className="text-xs text-zinc-400 mb-4">
                Playing in hardware-accelerated video player
              </p>
              <div className="inline-flex items-center justify-center gap-2 text-[11px] text-zinc-400 bg-zinc-900/80 py-1.5 px-3 rounded-lg border border-zinc-800">
                <span>Space: Play/Pause</span>
                <span>•</span>
                <span>←/→: Seek 5s</span>
                <span>•</span>
                <span>F: Fullscreen</span>
                <span>•</span>
                <span>M: Mute</span>
              </div>
            </div>
          </div>
        )}

        {/* Transient Play/Pause Center Ripple */}
        <PlayPauseRipple type={rippleType} />
      </div>

      {/* Drag & Drop Overlay */}
      {isDragOver && (
        <div
          data-testid="drag-drop-overlay"
          className="pointer-events-none absolute inset-0 z-30 flex flex-col items-center justify-center bg-indigo-950/80 backdrop-blur-md border-4 border-dashed border-indigo-400/80 m-4 rounded-3xl animate-in fade-in duration-200"
        >
          <UploadCloud className="w-16 h-16 text-indigo-300 mb-4 animate-bounce" />
          <h2 className="text-xl font-bold text-white mb-1">Drop Media File Here</h2>
          <p className="text-sm text-indigo-200">
            Supports .mp4, .mov, .mkv video and .srt, .ass subtitle files
          </p>
        </div>
      )}
    </div>
  );
}
