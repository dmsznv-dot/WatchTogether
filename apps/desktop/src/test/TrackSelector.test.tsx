import React from 'react';
import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { TrackSelector } from '../features/player/TrackSelector';
import { windowApi } from '../services/ipc';
import { MediaTrack } from '../types/player';

const track = (over: Partial<MediaTrack>): MediaTrack => ({
  id: 1, track_type: 'audio', title: 'English', lang: 'eng', is_selected: true, is_external: false, codec: 'aac', ...over,
});

describe('TrackSelector external audio', () => {
  afterEach(() => vi.restoreAllMocks());

  it('adds an audio file picked in the dialog', async () => {
    const pick = vi.spyOn(windowApi, 'openFileDialog').mockResolvedValue('/dubs/russian.ac3');
    const onLoadExternalAudio = vi.fn();
    render(
      <TrackSelector
        audioTracks={[track({})]}
        subtitleTracks={[]}
        onSelectAudio={vi.fn()}
        onSelectSubtitle={vi.fn()}
        onLoadExternalSubtitle={vi.fn()}
        onLoadExternalAudio={onLoadExternalAudio}
      />
    );
    fireEvent.click(screen.getByTestId('audio-tracks-btn'));
    await act(async () => { fireEvent.click(screen.getByTestId('add-external-audio-btn')); });
    expect(pick).toHaveBeenCalledWith('audio');
    expect(onLoadExternalAudio).toHaveBeenCalledWith('/dubs/russian.ac3');
  });

  it('marks added audio tracks as External', () => {
    render(
      <TrackSelector
        audioTracks={[track({ is_selected: false }), track({ id: 2, title: 'russian.ac3', is_external: true, codec: 'ac3' })]}
        subtitleTracks={[]}
        onSelectAudio={vi.fn()}
        onSelectSubtitle={vi.fn()}
        onLoadExternalSubtitle={vi.fn()}
        onLoadExternalAudio={vi.fn()}
      />
    );
    fireEvent.click(screen.getByTestId('audio-tracks-btn'));
    expect(screen.getByTestId('audio-track-2')).toHaveTextContent('External');
    expect(screen.getByTestId('audio-track-1')).not.toHaveTextContent('External');
  });
});
