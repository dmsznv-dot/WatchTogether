import { useState, useEffect, useCallback, useRef } from 'react';
import { MediaTrack, TracksChangePayload } from '../types/player';
import { playerApi } from '../services/ipc';

export function useTracks() {
  const revision = useRef(0);
  const [audioTracks, setAudioTracks] = useState<MediaTrack[]>([]);
  const [subtitleTracks, setSubtitleTracks] = useState<MediaTrack[]>([]);

  const refreshTracks = useCallback(async () => {
    const requestRevision = ++revision.current;
    try {
      const [audios, subs] = await Promise.all([
        playerApi.getAudioTracks(),
        playerApi.getSubtitleTracks(),
      ]);
      if (requestRevision !== revision.current) return;
      setAudioTracks(audios);
      setSubtitleTracks(subs);
    } catch {
      // Ignore if player not loaded yet
    }
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    playerApi
      .onTracksChange((payload: TracksChangePayload) => {
        if (disposed) return;
        ++revision.current;
        const audios = payload.tracks.filter((t) => t.track_type === 'audio');
        const subs = payload.tracks.filter((t) => t.track_type === 'subtitle');
        setAudioTracks(audios);
        setSubtitleTracks(subs);
      })
      .then((fn) => {
        if (disposed) fn();
        else { unlisten = fn; void refreshTracks(); }
      });

    return () => {
      disposed = true;
      ++revision.current;
      if (unlisten) unlisten();
    };
  }, [refreshTracks]);

  const selectAudio = useCallback(async (trackId: number) => {
    try {
      ++revision.current;
      await playerApi.selectAudio(trackId);
      ++revision.current;
      setAudioTracks((prev) =>
        prev.map((t) => ({ ...t, is_selected: t.id === trackId }))
      );
    } catch (err) {
      console.error('Failed to select audio track:', err);
    }
  }, []);

  const selectSubtitle = useCallback(async (trackId: number) => {
    try {
      ++revision.current;
      await playerApi.selectSubtitle(trackId);
      ++revision.current;
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
        ++revision.current;
        const track = await playerApi.loadExternalSubtitle(path);
        ++revision.current;
        setSubtitleTracks((prev) => [
          ...prev.filter((t) => t.id !== track.id).map((t) => ({ ...t, is_selected: false })),
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
