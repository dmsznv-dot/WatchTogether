import {
  MediaTrack,
  PlayerState,
  StateChangePayload,
  TimePosPayload,
  TracksChangePayload,
} from '../types/player';
import {
  Participant,
  PermissionMode,
  RoomCreateResponse,
  RoomJoinResponse,
  RoomStateSnapshot,
  RoomUpdatePayload,
  SessionType,
} from '../types/room';
import { generateMockRoomCode, generateMockRoomToken, isValidRoomCode } from '../utils/crockford';

export class MockIpcBackend {
  private playerState: PlayerState = {
    current_file: null,
    is_playing: false,
    position_ms: 0,
    duration_ms: 0,
    speed: 1.0,
    volume: 1.0,
    is_muted: false,
  };

  private audioTracks: MediaTrack[] = [
    {
      id: 1,
      track_type: 'audio',
      title: 'English (Stereo AAC)',
      lang: 'eng',
      is_selected: true,
      is_external: false,
      codec: 'aac',
    },
    {
      id: 2,
      track_type: 'audio',
      title: 'Commentary (AC3)',
      lang: 'eng',
      is_selected: false,
      is_external: false,
      codec: 'ac3',
    },
  ];

  private subtitleTracks: MediaTrack[] = [
    {
      id: 1,
      track_type: 'subtitle',
      title: 'English [SDH]',
      lang: 'eng',
      is_selected: false,
      is_external: false,
      codec: 'subrip',
    },
  ];

  private sessionType: SessionType = 'none';
  private roomCode: string | null = null;
  private roomToken: string | null = null;
  private snapshot: RoomStateSnapshot | null = null;
  private isFullscreen = false;

  private listeners: Map<string, Set<(payload: any) => void>> = new Map();
  private tickerInterval: any = null;

  constructor() {
    this.startTicker();
  }

  private startTicker() {
    if (this.tickerInterval) return;
    this.tickerInterval = setInterval(() => {
      if (this.playerState.is_playing && this.playerState.duration_ms > 0) {
        const delta = Math.round(250 * this.playerState.speed);
        let nextPos = this.playerState.position_ms + delta;
        if (nextPos >= this.playerState.duration_ms) {
          nextPos = this.playerState.duration_ms;
          this.playerState.is_playing = false;
          this.emitStateChange();
        }
        this.playerState.position_ms = nextPos;
        this.emitTimePos();
      }
    }, 250);
  }

  public reset(): void {
    this.playerState = {
      current_file: null,
      is_playing: false,
      position_ms: 0,
      duration_ms: 0,
      speed: 1.0,
      volume: 1.0,
      is_muted: false,
    };
    this.sessionType = 'none';
    this.roomCode = null;
    this.roomToken = null;
    this.snapshot = null;
    this.isFullscreen = false;
    this.audioTracks = [
      {
        id: 1,
        track_type: 'audio',
        title: 'English (Stereo AAC)',
        lang: 'eng',
        is_selected: true,
        is_external: false,
        codec: 'aac',
      },
      {
        id: 2,
        track_type: 'audio',
        title: 'Commentary (AC3)',
        lang: 'eng',
        is_selected: false,
        is_external: false,
        codec: 'ac3',
      },
    ];
    this.subtitleTracks = [
      {
        id: 1,
        track_type: 'subtitle',
        title: 'English [SDH]',
        lang: 'eng',
        is_selected: false,
        is_external: false,
        codec: 'subrip',
      },
    ];
  }

  public subscribe<T>(event: string, handler: (payload: T) => void): () => void {
    if (!this.listeners.has(event)) {
      this.listeners.set(event, new Set());
    }
    this.listeners.get(event)!.add(handler);
    return () => {
      this.listeners.get(event)?.delete(handler);
    };
  }

  public emit<T>(event: string, payload: T): void {
    const handlers = this.listeners.get(event);
    if (handlers) {
      handlers.forEach((h) => {
        try {
          h(payload);
        } catch (err) {
          console.error(`Error in mock event handler for ${event}:`, err);
        }
      });
    }
  }

  private emitTimePos(): void {
    const payload: TimePosPayload = {
      position_ms: this.playerState.position_ms,
      duration_ms: this.playerState.duration_ms,
      position_sec: this.playerState.position_ms / 1000,
      duration_sec: this.playerState.duration_ms / 1000,
    };
    this.emit('player://time-pos', payload);
  }

  private emitStateChange(): void {
    const payload: StateChangePayload = {
      is_playing: this.playerState.is_playing,
      is_paused: !this.playerState.is_playing,
      speed: this.playerState.speed,
      volume: this.playerState.volume,
      is_muted: this.playerState.is_muted,
    };
    this.emit('player://state-change', payload);
  }

  private emitTracksChange(): void {
    const payload: TracksChangePayload = {
      tracks: [...this.audioTracks, ...this.subtitleTracks],
    };
    this.emit('player://tracks-change', payload);
  }

  private emitRoomUpdate(): void {
    const payload: RoomUpdatePayload = {
      session_type: this.sessionType,
      room_code: this.roomCode,
      snapshot: this.snapshot,
    };
    this.emit('room://update', payload);
  }

  public async handleInvoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
    switch (cmd) {
      case 'cmd_player_open_file':
      case 'player_open_file': {
        const path = (args.path as string) || '/Users/demo/Videos/sample_movie.mp4';
        this.playerState.current_file = path;
        this.playerState.duration_ms = 142000; // 2m 22s
        this.playerState.position_ms = 0;
        this.playerState.is_playing = false;
        this.emitStateChange();
        this.emitTimePos();
        this.emitTracksChange();
        return { ...this.playerState } as unknown as T;
      }

      case 'cmd_player_play':
      case 'player_play': {
        this.playerState.is_playing = true;
        this.emitStateChange();
        return undefined as unknown as T;
      }

      case 'cmd_player_pause':
      case 'player_pause': {
        this.playerState.is_playing = false;
        this.emitStateChange();
        return undefined as unknown as T;
      }

      case 'cmd_player_seek':
      case 'player_seek': {
        const posSec = (args.positionSeconds as number) ?? (args.position_seconds as number) ?? 0;
        this.playerState.position_ms = Math.max(
          0,
          Math.min(this.playerState.duration_ms, Math.round(posSec * 1000))
        );
        this.emitTimePos();
        return undefined as unknown as T;
      }

      case 'cmd_player_set_speed':
      case 'player_set_speed': {
        const speed = (args.speed as number) || 1.0;
        this.playerState.speed = speed;
        this.emitStateChange();
        return undefined as unknown as T;
      }

      case 'cmd_player_set_volume':
      case 'player_set_volume': {
        const volume = (args.volume as number) ?? 1.0;
        this.playerState.volume = Math.max(0, Math.min(1, volume));
        this.emitStateChange();
        return undefined as unknown as T;
      }

      case 'cmd_player_set_mute':
      case 'player_set_mute': {
        const muted = !!args.muted;
        this.playerState.is_muted = muted;
        this.emitStateChange();
        return undefined as unknown as T;
      }

      case 'cmd_player_get_state':
      case 'player_get_state': {
        return { ...this.playerState } as unknown as T;
      }

      case 'cmd_player_get_audio_tracks':
      case 'player_get_audio_tracks': {
        return [...this.audioTracks] as unknown as T;
      }

      case 'cmd_player_select_audio':
      case 'player_select_audio': {
        const trackId = (args.trackId as number) ?? (args.track_id as number);
        this.audioTracks = this.audioTracks.map((t) => ({
          ...t,
          is_selected: t.id === trackId,
        }));
        this.emitTracksChange();
        return undefined as unknown as T;
      }

      case 'cmd_player_get_subtitle_tracks':
      case 'player_get_subtitle_tracks': {
        return [...this.subtitleTracks] as unknown as T;
      }

      case 'cmd_player_select_subtitle':
      case 'player_select_subtitle': {
        const trackId = (args.trackId as number) ?? (args.track_id as number);
        this.subtitleTracks = this.subtitleTracks.map((t) => ({
          ...t,
          is_selected: t.id === trackId,
        }));
        this.emitTracksChange();
        return undefined as unknown as T;
      }

      case 'cmd_player_toggle_stats':
        return undefined as unknown as T;

      case 'cmd_player_load_external_subtitle':
      case 'cmd_player_add_subtitle':
      case 'player_add_subtitle': {
        const path = (args.path as string) || 'subtitle.srt';
        const filename = path.split('/').pop() || 'External Subtitle';
        const newTrack: MediaTrack = {
          id: this.subtitleTracks.length + 10,
          track_type: 'subtitle',
          title: filename,
          lang: null,
          is_selected: true,
          is_external: true,
          codec: path.endsWith('.ass') ? 'ass' : 'subrip',
        };
        this.subtitleTracks = this.subtitleTracks.map((t) => ({ ...t, is_selected: false }));
        this.subtitleTracks.push(newTrack);
        this.emitTracksChange();
        return (cmd === 'cmd_player_load_external_subtitle' ? newTrack : undefined) as unknown as T;
      }

      case 'cmd_window_toggle_fullscreen': {
        this.isFullscreen = !this.isFullscreen;
        return this.isFullscreen as unknown as T;
      }

      case 'cmd_dialog_open_file': {
        return '/Users/demo/Videos/sample_movie.mp4' as unknown as T;
      }

      case 'cmd_room_create':
      case 'room_create': {
        const hostName = (args.hostName as string) ?? (args.host_name as string) ?? 'Host User';
        const permMode = ((args.permissionMode as PermissionMode) ??
          (args.permission_mode as PermissionMode) ??
          'free_control') as PermissionMode;
        const code = generateMockRoomCode();
        const token = generateMockRoomToken();
        const roomId = `room-${code}`;
        const hostId = 'part-host-1';

        const hostPart: Participant = {
          id: hostId,
          display_name: hostName,
          role: 'host',
          is_connected: true,
          buffered_position_ms: 142000,
          current_position_ms: this.playerState.position_ms,
          ping_ms: 12,
          joined_at_ms: Date.now(),
        };

        this.sessionType = 'host';
        this.roomCode = code;
        this.roomToken = token;
        this.snapshot = {
          room_id: roomId,
          room_code: code,
          host_id: hostId,
          permission_mode: permMode,
          participants: [hostPart],
          playback: {
            revision: 1,
            status: this.playerState.is_playing ? 'playing' : 'paused',
            position_ms: this.playerState.position_ms,
            speed: this.playerState.speed,
            updated_at_server_ms: Date.now(),
            duration_ms: this.playerState.duration_ms,
          },
          video: {
            filename: this.playerState.current_file?.split('/').pop() || 'sample_movie.mp4',
            file_size_bytes: 524288000,
            duration_ms: this.playerState.duration_ms,
            mime_type: 'video/mp4',
            width: 1920,
            height: 1080,
          },
        };

        this.emitRoomUpdate();

        const response: RoomCreateResponse = {
          room_id: roomId,
          room_code: code,
          room_token: token,
          host_addr: `mock-iroh-${code}`,
        };
        return response as unknown as T;
      }

      case 'cmd_room_join':
      case 'room_join': {
        const rawCode = (args.roomCode as string) ?? (args.room_code as string) ?? '';
        const rawToken = (args.roomToken as string) ?? (args.room_token as string) ?? '';
        const displayName =
          (args.displayName as string) ?? (args.display_name as string) ?? 'Guest User';

        const normalizedCode = rawCode.trim().toUpperCase().replace(/[\s-]/g, '');
        if (!isValidRoomCode(normalizedCode)) {
          throw new Error(`Invalid room code: ${normalizedCode}`);
        }
        if (!rawToken || rawToken.trim().length < 32) {
          throw new Error('Invalid room token: token must be at least 32 characters');
        }

        const participantId = `part-guest-${Date.now()}`;
        this.sessionType = 'guest';
        this.roomCode = normalizedCode;
        this.roomToken = rawToken.trim();

        const hostPart: Participant = {
          id: 'part-host-1',
          display_name: 'Room Host',
          role: 'host',
          is_connected: true,
          buffered_position_ms: 142000,
          current_position_ms: 0,
          ping_ms: 24,
          joined_at_ms: Date.now() - 30000,
        };

        const guestPart: Participant = {
          id: participantId,
          display_name: displayName,
          role: 'guest',
          is_connected: true,
          buffered_position_ms: 100000,
          current_position_ms: 0,
          ping_ms: 45,
          joined_at_ms: Date.now(),
        };

        this.snapshot = {
          room_id: `room-${normalizedCode}`,
          room_code: normalizedCode,
          host_id: 'part-host-1',
          permission_mode: 'free_control',
          participants: [hostPart, guestPart],
          playback: {
            revision: 2,
            status: 'paused',
            position_ms: 0,
            speed: 1.0,
            updated_at_server_ms: Date.now(),
            duration_ms: 142000,
          },
          video: {
            filename: 'sample_movie.mp4',
            file_size_bytes: 524288000,
            duration_ms: 142000,
            mime_type: 'video/mp4',
            width: 1920,
            height: 1080,
          },
        };

        this.emitRoomUpdate();

        const response: RoomJoinResponse = {
          participant_id: participantId,
          room_code: normalizedCode,
          room_token: rawToken.trim(),
        };
        return response as unknown as T;
      }

      case 'cmd_room_leave':
      case 'room_leave': {
        this.sessionType = 'none';
        this.roomCode = null;
        this.roomToken = null;
        this.snapshot = null;
        this.emitRoomUpdate();
        return undefined as unknown as T;
      }

      case 'cmd_room_set_permissions':
      case 'room_set_permissions': {
        const mode = ((args.permissionMode as PermissionMode) ??
          (args.permission_mode as PermissionMode) ??
          'free_control') as PermissionMode;
        if (this.snapshot) {
          this.snapshot = {
            ...this.snapshot,
            permission_mode: mode,
          };
          this.emitRoomUpdate();
        }
        return undefined as unknown as T;
      }

      case 'cmd_room_kick_participant':
      case 'room_kick_participant': {
        const targetId = (args.targetId as string) ?? (args.target_id as string);
        if (this.snapshot && targetId) {
          this.snapshot = {
            ...this.snapshot,
            participants: this.snapshot.participants.filter((p) => p.id !== targetId),
          };
          this.emitRoomUpdate();
        }
        return undefined as unknown as T;
      }

      case 'cmd_room_get_state':
      case 'room_get_state': {
        if (!this.snapshot) {
          throw new Error('No active room session');
        }
        return { ...this.snapshot } as unknown as T;
      }

      default:
        console.warn(`[MockIpcBackend] Unhandled command: ${cmd}`, args);
        return undefined as unknown as T;
    }
  }
}

export const mockBackend = new MockIpcBackend();
