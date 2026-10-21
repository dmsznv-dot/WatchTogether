import { useState, useEffect, useCallback } from 'react';
import { MediaTrack, TracksChangePayload } from '../types/player';
import { playerApi } from '../services/ipc';

export function useTracks() {
  const [audioTracks, setAudioTracks] = useState<MediaTrack[]>([]);
  const [subtitleTracks, setSubtitleTracks] = useState<MediaTrack[]>([]);

  const refreshTracks = useCallback(async () => {
    try {
      const [audios, subs] = await Promise.all([
        playerApi.getAudioTracks(),
        playerApi.getSubtitleTracks(),
      ]);
      setAudioTracks(audios);
      setSubtitleTracks(subs);
    } catch {
      // Ignore if player not loaded yet
    }
  }, []);

  useEffect(() => {
    refreshTracks();

    let unlisten: (() => void) | undefined;
    playerApi
      .onTracksChange((payload: TracksChangePayload) => {
        const audios = payload.tracks.filter((t) => t.track_type === 'audio');
        const subs = payload.tracks.filter((t) => t.track_type === 'subtitle');
        setAudioTracks(audios);
        setSubtitleTracks(subs);
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      if (unlisten) unlisten();
    };
  }, [refreshTracks]);

  const selectAudio = useCallback(async (trackId: number) => {
    try {
      await playerApi.selectAudio(trackId);
      setAudioTracks((prev) =>
        prev.map((t) => ({ ...t, is_selected: t.id === trackId }))
      );
    } catch (err) {
      console.error('Failed to select audio track:', err);
    }
  }, []);

  const selectSubtitle = useCallback(async (trackId: number) => {
    try {
      await playerApi.selectSubtitle(trackId);
      setSubtitleTracks((prev) =>
        prev.map((t) => ({ ...t, is_selected: t.id === trackId }))
      );
    } catch (err) {
      console.error('Failed to select subtitle track:', err);
    }
  }, []);

  const loadExternalSubtitle = useCallback(
    async (path: string): Promise<MediaTrack | null> => {
      try {
        const track = await playerApi.loadExternalSubtitle(path);
        setSubtitleTracks((prev) => [
          ...prev.map((t) => ({ ...t, is_selected: false })),
          track,
        ]);
        return track;
      } catch (err) {
        console.error('Failed to load external subtitle:', err);
        return null;
      }
    },
    []
  );

  return {
    audioTracks,
    subtitleTracks,
    selectAudio,
    selectSubtitle,
    loadExternalSubtitle,
    refreshTracks,
  };
}
