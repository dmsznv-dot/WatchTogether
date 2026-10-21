export type ParticipantRole = 'host' | 'guest';

export type PermissionMode = 'free_control' | 'host_only';

export type PlaybackStatus = 'playing' | 'paused' | 'buffering';

export interface Participant {
  id: string;
  display_name: string;
  role: ParticipantRole;
  is_connected: boolean;
  buffered_position_ms: number;
  current_position_ms: number;
  ping_ms: number;
  joined_at_ms: number;
}

export interface VideoMetadata {
  filename: string;
  file_size_bytes: number;
  duration_ms: number;
  mime_type: string;
  width?: number | null;
  height?: number | null;
}

export interface PlaybackState {
  revision: number;
  status: PlaybackStatus;
  position_ms: number;
  speed: number;
  updated_at_server_ms: number;
  duration_ms: number;
}

export interface RoomStateSnapshot {
  room_id: string;
  room_code: string;
  host_id: string;
  permission_mode: PermissionMode;
  participants: Participant[];
  playback: PlaybackState;
  video: VideoMetadata;
}

export type SessionType = 'none' | 'host' | 'guest';

export interface RoomUpdatePayload {
  session_type: SessionType;
  room_code?: string | null;
  snapshot?: RoomStateSnapshot | null;
}

export interface RoomErrorPayload {
  code: string;
  message: string;
}

export interface RoomCreateResponse {
  room_id: string;
  room_code: string;
  room_token: string;
}

export interface RoomJoinResponse {
  participant_id: string;
  room_code: string;
  room_token: string;
}
