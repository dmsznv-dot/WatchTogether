import { useState, useEffect, useCallback, useRef } from 'react';
import {
  PermissionMode,
  RoomCreateResponse,
  RoomJoinResponse,
  RoomStateSnapshot,
  RoomUpdatePayload,
  SessionType,
} from '../types/room';
import { roomApi } from '../services/ipc';
import { normalizeRoomCode } from '../utils/crockford';

export function useRoom() {
  const [sessionType, setSessionType] = useState<SessionType>('none');
  const [roomCode, setRoomCode] = useState<string | null>(null);
  const [roomToken, setRoomToken] = useState<string | null>(null);
  const [snapshot, setSnapshot] = useState<RoomStateSnapshot | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [authorityToast, setAuthorityToast] = useState<string | null>(null);

  const toastTimerRef = useRef<number | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    roomApi
      .onUpdate((payload: RoomUpdatePayload) => {
        setSessionType(payload.session_type);
        setRoomCode(payload.room_code || null);
        setSnapshot(payload.snapshot || null);
        if (payload.session_type === 'none') {
          setRoomToken(null);
        }
      })
      .then((fn) => {
        unlisten = fn;
      });

    return () => {
      if (unlisten) unlisten();
      if (toastTimerRef.current !== null) {
        window.clearTimeout(toastTimerRef.current);
      }
    };
  }, []);

  const isHost = sessionType === 'host';
  const isGuest = sessionType === 'guest';
  const isSolo = sessionType === 'none';
  const permissionMode: PermissionMode = snapshot?.permission_mode || 'free_control';

  // Can the current local user control playback (play, pause, seek, speed)?
  const canControlPlayback = isSolo || isHost || permissionMode === 'free_control';

  const notifyHostAuthority = useCallback((action = 'control playback') => {
    const hostName =
      snapshot?.participants.find((p) => p.role === 'host')?.display_name || 'Host';
    setAuthorityToast(`Playback is controlled by ${hostName} (Host-Only mode)`);

    if (toastTimerRef.current !== null) {
      window.clearTimeout(toastTimerRef.current);
    }
    toastTimerRef.current = window.setTimeout(() => {
      setAuthorityToast(null);
      toastTimerRef.current = null;
    }, 3000);
  }, [snapshot]);

  const createRoom = useCallback(
    async (
      hostName?: string,
      permMode: PermissionMode = 'free_control',
      videoPath?: string
    ): Promise<RoomCreateResponse> => {
      setIsLoading(true);
      setError(null);
      try {
        const res = await roomApi.create(hostName, videoPath, permMode);
        setSessionType('host');
        setRoomCode(res.room_code);
        setRoomToken(res.room_token);
        return res;
      } catch (err: any) {
        const msg = err?.message || String(err);
        setError(msg);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const joinRoom = useCallback(
    async (
      code: string,
      token: string,
      displayName?: string
    ): Promise<RoomJoinResponse> => {
      setIsLoading(true);
      setError(null);
      const normalizedCode = normalizeRoomCode(code);
      try {
        const res = await roomApi.join(normalizedCode, token, displayName);
        setSessionType('guest');
        setRoomCode(res.room_code);
        setRoomToken(res.room_token);
        return res;
      } catch (err: any) {
        const msg = err?.message || String(err);
        setError(msg);
        throw err;
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  const leaveRoom = useCallback(async () => {
    setIsLoading(true);
    try {
      await roomApi.leave();
      setSessionType('none');
      setRoomCode(null);
      setRoomToken(null);
      setSnapshot(null);
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const setPermissions = useCallback(
    async (mode: PermissionMode) => {
      if (!isHost) return;
      try {
        await roomApi.setPermissions(mode);
        if (snapshot) {
          setSnapshot({ ...snapshot, permission_mode: mode });
        }
      } catch (err: any) {
        setError(err?.message || String(err));
      }
    },
    [isHost, snapshot]
  );

  const kickParticipant = useCallback(
    async (targetId: string) => {
      if (!isHost) return;
      try {
        await roomApi.kickParticipant(targetId);
        if (snapshot) {
          setSnapshot({
            ...snapshot,
            participants: snapshot.participants.filter((p) => p.id !== targetId),
          });
        }
      } catch (err: any) {
        setError(err?.message || String(err));
      }
    },
    [isHost, snapshot]
  );

  return {
    sessionType,
    roomCode,
    roomToken,
    snapshot,
    isHost,
    isGuest,
    isSolo,
    permissionMode,
    canControlPlayback,
    authorityToast,
    isLoading,
    error,
    createRoom,
    joinRoom,
    leaveRoom,
    setPermissions,
    kickParticipant,
    notifyHostAuthority,
  };
}
