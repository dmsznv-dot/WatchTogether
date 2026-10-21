import {
  MediaTrack,
  PlaybackSpeed,
  PlayerState,
  StateChangePayload,
  TimePosPayload,
  TracksChangePayload,
} from '../types/player';
import {
  PermissionMode,
  RoomCreateResponse,
  RoomJoinResponse,
  RoomStateSnapshot,
  RoomUpdatePayload,
} from '../types/room';
import { mockBackend } from './mockBackend';

export const hasTauri = (): boolean => {
  return (
    typeof window !== 'undefined' &&
    ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)
  );
};

export async function invokeCommand<T>(
  cmd: string,
  args: Record<string, unknown> = {}
): Promise<T> {
  if (hasTauri()) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      return await invoke<T>(cmd, args);
    } catch (err) {
      console.error(`[Tauri IPC] Failed invoking ${cmd}:`, err);
      throw err;
    }
  }
  return mockBackend.handleInvoke<T>(cmd, args);
}

export async function listenEvent<T>(
  event: string,
  handler: (payload: T) => void
): Promise<() => void> {
  if (hasTauri()) {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      return await listen<T>(event, (e) => handler(e.payload));
    } catch (err) {
      console.error(`[Tauri IPC] Failed subscribing to ${event}:`, err);
      return () => {};
    }
  }
  return mockBackend.subscribe<T>(event, handler);
}

// Typed Player APIs
export const playerApi = {
  openFile: (path: string): Promise<PlayerState> =>
    invokeCommand<PlayerState>('cmd_player_open_file', { path }),

  play: (): Promise<void> => invokeCommand<void>('cmd_player_play'),

  pause: (): Promise<void> => invokeCommand<void>('cmd_player_pause'),

  seek: (positionSeconds: number, exact = true): Promise<void> =>
    invokeCommand<void>('cmd_player_seek', { positionSeconds, exact }),

  setSpeed: (speed: PlaybackSpeed | number): Promise<void> =>
    invokeCommand<void>('cmd_player_set_speed', { speed }),

  setVolume: (volume: number): Promise<void> =>
    invokeCommand<void>('cmd_player_set_volume', { volume }),

  setMute: (muted: boolean): Promise<void> =>
    invokeCommand<void>('cmd_player_set_mute', { muted }),

  getState: (): Promise<PlayerState> => invokeCommand<PlayerState>('cmd_player_get_state'),

  getAudioTracks: (): Promise<MediaTrack[]> =>
    invokeCommand<MediaTrack[]>('cmd_player_get_audio_tracks'),

  selectAudio: (trackId: number): Promise<void> =>
    invokeCommand<void>('cmd_player_select_audio', { trackId }),

  getSubtitleTracks: (): Promise<MediaTrack[]> =>
    invokeCommand<MediaTrack[]>('cmd_player_get_subtitle_tracks'),

  selectSubtitle: (trackId: number): Promise<void> =>
    invokeCommand<void>('cmd_player_select_subtitle', { trackId }),

  loadExternalSubtitle: (path: string): Promise<MediaTrack> =>
    invokeCommand<MediaTrack>('cmd_player_load_external_subtitle', { path }),

  addSubtitle: (path: string): Promise<void> =>
    invokeCommand<void>('cmd_player_add_subtitle', { path }),

  onTimePos: (callback: (payload: TimePosPayload) => void): Promise<() => void> =>
    listenEvent<TimePosPayload>('player://time-pos', callback),

  onStateChange: (callback: (payload: StateChangePayload) => void): Promise<() => void> =>
    listenEvent<StateChangePayload>('player://state-change', callback),

  onTracksChange: (callback: (payload: TracksChangePayload) => void): Promise<() => void> =>
    listenEvent<TracksChangePayload>('player://tracks-change', callback),
};

// Typed Room APIs
export const roomApi = {
  create: (
    hostName?: string,
    videoPath?: string,
    permissionMode: PermissionMode = 'free_control'
  ): Promise<RoomCreateResponse> =>
    invokeCommand<RoomCreateResponse>('cmd_room_create', {
      hostName,
      videoPath,
      permissionMode,
    }),

  join: (
    roomCode: string,
    roomToken: string,
    displayName?: string
  ): Promise<RoomJoinResponse> =>
    invokeCommand<RoomJoinResponse>('cmd_room_join', {
      roomCode,
      roomToken,
      displayName,
    }),

  leave: (): Promise<void> => invokeCommand<void>('cmd_room_leave'),

  getState: (): Promise<RoomStateSnapshot> =>
    invokeCommand<RoomStateSnapshot>('cmd_room_get_state'),

  setPermissions: (permissionMode: PermissionMode): Promise<void> =>
    invokeCommand<void>('cmd_room_set_permissions', { permissionMode }),

  kickParticipant: (targetId: string): Promise<void> =>
    invokeCommand<void>('cmd_room_kick_participant', { targetId }),

  onUpdate: (callback: (payload: RoomUpdatePayload) => void): Promise<() => void> =>
    listenEvent<RoomUpdatePayload>('room://update', callback),
};

// Window & Dialog APIs
export const windowApi = {
  toggleFullscreen: async (): Promise<boolean> => {
    if (hasTauri()) {
      return invokeCommand<boolean>('cmd_window_toggle_fullscreen');
    }
    // Browser fallback
    if (!document.fullscreenElement) {
      await document.documentElement.requestFullscreen().catch(() => {});
      return true;
    } else {
      await document.exitFullscreen().catch(() => {});
      return false;
    }
  },

  openFileDialog: async (): Promise<string | null> => {
    if (hasTauri()) {
      try {
        const { open } = await import('@tauri-apps/plugin-dialog');
        const selected = await open({
          multiple: false,
          filters: [
            {
              name: 'Video Files',
              extensions: ['mp4', 'mov', 'mkv'],
            },
            {
              name: 'Subtitles',
              extensions: ['srt', 'ass'],
            },
          ],
        });
        if (typeof selected === 'string') return selected;
        return null;
      } catch {
        return invokeCommand<string | null>('cmd_dialog_open_file');
      }
    }
    return invokeCommand<string | null>('cmd_dialog_open_file');
  },
};
