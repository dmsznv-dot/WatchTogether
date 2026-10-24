import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { VideoCanvas } from '../features/player/VideoCanvas';

describe('VideoCanvas Component', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('renders hero screen when no video is loaded', () => {
    render(
      <VideoCanvas
        currentFile={null}
        isPlaying={false}
        onOpenFile={vi.fn()}
        onTogglePlayPause={vi.fn()}
        onToggleFullscreen={vi.fn()}
        onSelectFileClick={vi.fn()}
      />
    );

    expect(screen.getByText('No Video Loaded')).toBeInTheDocument();
    expect(screen.getByText('Open Video File')).toBeInTheDocument();
  });

  it('renders media surface when currentFile is loaded', () => {
    render(
      <VideoCanvas
        currentFile="/path/to/movie.mp4"
        isPlaying={false}
        onOpenFile={vi.fn()}
        onTogglePlayPause={vi.fn()}
        onToggleFullscreen={vi.fn()}
        onSelectFileClick={vi.fn()}
      />
    );

    expect(screen.getByTestId('media-surface')).toBeInTheDocument();
    expect(screen.getByText('movie.mp4')).toBeInTheDocument();
  });

  it('handles video file drop', () => {
    const onOpenFile = vi.fn();
    render(
      <VideoCanvas
        currentFile={null}
        isPlaying={false}
        onOpenFile={onOpenFile}
        onTogglePlayPause={vi.fn()}
        onToggleFullscreen={vi.fn()}
        onSelectFileClick={vi.fn()}
      />
    );

    const container = screen.getByTestId('video-canvas-container');
    const file = new File(['dummy content'], 'cool_film.mp4', {
      type: 'video/mp4',
    });

    fireEvent.drop(container, {
      dataTransfer: {
        files: [file],
      },
    });

    expect(onOpenFile).toHaveBeenCalledWith('cool_film.mp4');
  });

  it('routes dropped audio and subtitle files to their tracks, not to the video', () => {
    const onOpenFile = vi.fn();
    const onLoadAudio = vi.fn();
    const onLoadSubtitle = vi.fn();
    render(
      <VideoCanvas
        currentFile="/path/to/movie.mkv"
        isPlaying={false}
        onOpenFile={onOpenFile}
        onLoadAudio={onLoadAudio}
        onLoadSubtitle={onLoadSubtitle}
        onTogglePlayPause={vi.fn()}
        onToggleFullscreen={vi.fn()}
        onSelectFileClick={vi.fn()}
      />
    );
    const container = screen.getByTestId('video-canvas-container');
    const drop = (name: string) =>
      fireEvent.drop(container, { dataTransfer: { files: [new File(['x'], name)] } });
    drop('Russian Dub.MKA');
    drop('commentary.mp3');
    drop('movie.srt');
    expect(onLoadAudio).toHaveBeenCalledWith('Russian Dub.MKA');
    expect(onLoadAudio).toHaveBeenCalledWith('commentary.mp3');
    expect(onLoadSubtitle).toHaveBeenCalledWith('movie.srt');
    expect(onOpenFile).not.toHaveBeenCalled();
  });

  it('handles single click play/pause vs double click fullscreen', () => {
    const onTogglePlayPause = vi.fn();
    const onToggleFullscreen = vi.fn();

    render(
      <VideoCanvas
        currentFile="/path/to/movie.mp4"
        isPlaying={false}
        onOpenFile={vi.fn()}
        onTogglePlayPause={onTogglePlayPause}
        onToggleFullscreen={onToggleFullscreen}
        onSelectFileClick={vi.fn()}
      />
    );

    const viewport = screen.getByTestId('aspect-ratio-viewport');

    // Single click
    fireEvent.click(viewport);
    act(() => {
      vi.advanceTimersByTime(250);
    });
    expect(onTogglePlayPause).toHaveBeenCalledTimes(1);
    expect(onToggleFullscreen).not.toHaveBeenCalled();

    // Double click in rapid succession (< 230ms)
    fireEvent.click(viewport);
    fireEvent.click(viewport);
    expect(onToggleFullscreen).toHaveBeenCalledTimes(1);
  });
});
