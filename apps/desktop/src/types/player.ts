export type TrackType = 'audio' | 'subtitle' | 'video';

export interface MediaTrack {
  id: number;
  track_type: TrackType;
  title: string | null;
  lang: string | null;
  is_selected: boolean;
  is_external: boolean;
  codec?: string | null;
}

export type PlaybackSpeed = 0.5 | 0.75 | 1.0 | 1.25 | 1.5 | 2.0;

export const PLAYBACK_SPEEDS: readonly PlaybackSpeed[] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] as const;

export interface PlayerState {
  current_file: string | null;
  is_playing: boolean;
  position_ms: number;
  duration_ms: number;
  speed: number;
  volume: number;
  is_muted: boolean;
}

export interface TimePosPayload {
  position_ms: number;
  duration_ms: number;
  position_sec: number;
  duration_sec: number;
}

export interface StateChangePayload {
  is_playing: boolean;
  is_paused: boolean;
  speed: number;
  volume: number;
  is_muted: boolean;
}

export interface TracksChangePayload {
  tracks: MediaTrack[];
}

export interface VideoDimensions {
  width: number;
  height: number;
}
