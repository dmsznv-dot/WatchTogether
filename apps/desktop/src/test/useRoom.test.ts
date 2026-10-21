import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useRoom } from '../hooks/useRoom';
import { mockBackend } from '../services/mockBackend';
import { isValidRoomCode } from '../utils/crockford';

describe('useRoom Hook', () => {
  beforeEach(() => {
    mockBackend.reset();
  });

  it('starts in solo mode', () => {
    const { result } = renderHook(() => useRoom());
    expect(result.current.sessionType).toBe('none');
    expect(result.current.isSolo).toBe(true);
    expect(result.current.isHost).toBe(false);
    expect(result.current.isGuest).toBe(false);
    expect(result.current.canControlPlayback).toBe(true);
  });

  it('creates room as host with valid Crockford code', async () => {
    const { result } = renderHook(() => useRoom());

    let res: any;
    await act(async () => {
      res = await result.current.createRoom('Host Bob', 'free_control');
    });

    expect(result.current.sessionType).toBe('host');
    expect(result.current.isHost).toBe(true);
    expect(result.current.roomCode).toBeDefined();
    expect(isValidRoomCode(result.current.roomCode!)).toBe(true);
    expect(result.current.roomToken).toBeDefined();
    expect(result.current.roomToken!.length).toBeGreaterThanOrEqual(32);
    expect(result.current.snapshot?.participants).toHaveLength(1);
    expect(result.current.canControlPlayback).toBe(true);
  });

  it('joins room as guest with normalized Crockford code', async () => {
    const { result } = renderHook(() => useRoom());

    await act(async () => {
      await result.current.joinRoom(
        '  k7m-2p9  ',
        '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
        'Guest Charlie'
      );
    });

    expect(result.current.sessionType).toBe('guest');
    expect(result.current.isGuest).toBe(true);
    expect(result.current.roomCode).toBe('K7M2P9');
    expect(result.current.snapshot?.participants.length).toBeGreaterThanOrEqual(2);
  });

  it('toggles permission mode and calculates canControlPlayback', async () => {
    const { result } = renderHook(() => useRoom());
    await act(async () => {
      await result.current.createRoom('Host Bob', 'free_control');
    });

    expect(result.current.permissionMode).toBe('free_control');

    await act(async () => {
      await result.current.setPermissions('host_only');
    });
    expect(result.current.permissionMode).toBe('host_only');
    expect(result.current.canControlPlayback).toBe(true); // Still true for host
  });

  it('notifies host authority in host-only mode', () => {
    const { result } = renderHook(() => useRoom());
    act(() => {
      result.current.notifyHostAuthority();
    });

    expect(result.current.authorityToast).toContain('Host-Only mode');
  });

  it('leaves room and cascades reset', async () => {
    const { result } = renderHook(() => useRoom());
    await act(async () => {
      await result.current.createRoom('Host Bob');
    });

    await act(async () => {
      await result.current.leaveRoom();
    });

    expect(result.current.sessionType).toBe('none');
    expect(result.current.roomCode).toBeNull();
    expect(result.current.roomToken).toBeNull();
    expect(result.current.snapshot).toBeNull();
  });
});
