import { useState, useEffect, useCallback, useRef } from 'react';
import { PlaybackSpeed, PlayerState, StateChangePayload, TimePosPayload } from '../types/player';
import { playerApi } from '../services/ipc';

export interface UsePlayerOptions {
  initialVolume?: number;
  initialMuted?: boolean;
}

export function usePlayer({ initialVolume = 1.0, initialMuted = false }: UsePlayerOptions = {}) {
  const [currentFile, setCurrentFile] = useState<string | null>(null);
  const [isPlaying, setIsPlaying] = useState<boolean>(false);
  const [positionSec, setPositionSec] = useState<number>(0);
  const [durationSec, setDurationSec] = useState<number>(0);
  const [speed, setSpeedState] = useState<PlaybackSpeed>(1.0);
  const [volume, setVolumeState] = useState<number>(initialVolume);
  const [isMuted, setIsMutedState] = useState<boolean>(initialMuted);

  // Scrubber state
  const [isScrubbing, setIsScrubbing] = useState<boolean>(false);
  const [scrubPositionSec, setScrubPositionSec] = useState<number>(0);

  const isScrubbingRef = useRef<boolean>(false);
  isScrubbingRef.current = isScrubbing;

  const hasInitializedRef = useRef(false);

  // Load initial player state and subscribe to events
  useEffect(() => {
    let isMounted = true;

    if (!hasInitializedRef.current) {
      hasInitializedRef.current = true;
      if (initialVolume !== 1.0) {
        playerApi.setVolume(initialVolume).catch(() => {});
      }
      if (initialMuted) {
        playerApi.setMute(initialMuted).catch(() => {});
      }
    }

    playerApi
      .getState()
      .then((state: PlayerState) => {
        if (!isMounted) return;
        setCurrentFile(state.current_file);
        setIsPlaying(state.is_playing);
        setPositionSec(state.position_ms / 1000);
        setDurationSec(state.duration_ms / 1000);
        setSpeedState(state.speed as PlaybackSpeed);
        setVolumeState((prev) => (prev !== initialVolume ? prev : state.volume));
        setIsMutedState((prev) => (prev !== initialMuted ? prev : state.is_muted));
      })
      .catch(() => {});

    // Subscribe to time-pos events
    let unlistenTime: (() => void) | undefined;
    playerApi
      .onTimePos((payload: TimePosPayload) => {
        if (!isMounted) return;
        if (!isScrubbingRef.current) {
          setPositionSec(payload.position_sec);
        }
        setDurationSec(payload.duration_sec);
      })
      .then((fn) => {
        unlistenTime = fn;
      });

    // Subscribe to state-change events
    let unlistenState: (() => void) | undefined;
    playerApi
      .onStateChange((payload: StateChangePayload) => {
        if (!isMounted) return;
        setIsPlaying(payload.is_playing);
        setSpeedState(payload.speed as PlaybackSpeed);
        setVolumeState(payload.volume);
        setIsMutedState(payload.is_muted);
      })
      .then((fn) => {
        unlistenState = fn;
      });

    return () => {
      isMounted = false;
      if (unlistenTime) unlistenTime();
      if (unlistenState) unlistenState();
    };
  }, []);

  const openFile = useCallback(async (path: string) => {
    try {
      const state = await playerApi.openFile(path);
      setCurrentFile(state.current_file);
      setIsPlaying(state.is_playing);
      setPositionSec(state.position_ms / 1000);
      setDurationSec(state.duration_ms / 1000);
      setSpeedState(state.speed as PlaybackSpeed);
      setVolumeState(state.volume);
      setIsMutedState(state.is_muted);
    } catch (err) {
      console.error('Failed to open file:', err);
      throw err;
    }
  }, []);

  const play = useCallback(async () => {
    try {
      await playerApi.play();
      setIsPlaying(true);
    } catch (err) {
      console.error('Failed to play:', err);
    }
  }, []);

  const pause = useCallback(async () => {
    try {
      await playerApi.pause();
      setIsPlaying(false);
    } catch (err) {
      console.error('Failed to pause:', err);
    }
  }, []);

  const togglePlayPause = useCallback(() => {
    if (isPlaying) {
      pause();
    } else {
      play();
    }
  }, [isPlaying, play, pause]);

  const seek = useCallback(
    async (targetSeconds: number, exact = true) => {
      const clamped = Math.max(0, Math.min(durationSec || targetSeconds, targetSeconds));
      setPositionSec(clamped);
      try {
        await playerApi.seek(clamped, exact);
      } catch (err) {
        console.error('Failed to seek:', err);
      }
    },
    [durationSec]
  );

  const startScrub = useCallback((initialSec: number) => {
    setIsScrubbing(true);
    setScrubPositionSec(initialSec);
  }, []);

  const updateScrub = useCallback(
    (sec: number) => {
      const clamped = Math.max(0, Math.min(durationSec || sec, sec));
      setScrubPositionSec(clamped);
    },
    [durationSec]
  );

  const commitScrub = useCallback(async () => {
    setIsScrubbing(false);
    await seek(scrubPositionSec, true);
  }, [seek, scrubPositionSec]);

  const setSpeed = useCallback(async (newSpeed: PlaybackSpeed) => {
    try {
      await playerApi.setSpeed(newSpeed);
      setSpeedState(newSpeed);
    } catch (err) {
      console.error('Failed to set speed:', err);
    }
  }, []);

  const setVolume = useCallback(async (newVolume: number) => {
    const clamped = Math.max(0, Math.min(1, newVolume));
    setVolumeState(clamped);
    try {
      await playerApi.setVolume(clamped);
      if (clamped > 0 && isMuted) {
        setIsMutedState(false);
        await playerApi.setMute(false);
      }
    } catch (err) {
      console.error('Failed to set volume:', err);
    }
  }, [isMuted]);

  const toggleMute = useCallback(async () => {
    const nextMuted = !isMuted;
    setIsMutedState(nextMuted);
    try {
      await playerApi.setMute(nextMuted);
    } catch (err) {
      console.error('Failed to toggle mute:', err);
    }
  }, [isMuted]);

  return {
    currentFile,
    isPlaying,
    positionSec: isScrubbing ? scrubPositionSec : positionSec,
    durationSec,
    speed,
    volume,
    isMuted,
    isScrubbing,
    scrubPositionSec,
    openFile,
    play,
    pause,
    togglePlayPause,
    seek,
    startScrub,
    updateScrub,
    commitScrub,
    setSpeed,
    setVolume,
    toggleMute,
  };
}
