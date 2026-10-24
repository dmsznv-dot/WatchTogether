import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act, renderHook } from '@testing-library/react';
import {
  normalizeRoomCode,
  isValidRoomCode,
  getForbiddenCrockfordChars,
  generateMockRoomCode,
  CROCKFORD_CHARSET,
} from '../utils/crockford';
import { parseJoinInput, createShareableLink, createConnectionCode } from '../utils/deepLink';
import { JoinRoomModal } from '../features/rooms/JoinRoomModal';
import { ParticipantRosterPopover } from '../features/rooms/ParticipantRosterPopover';
import { ControlsHUD } from '../features/player/ControlsHUD';
import { HeaderHUD } from '../features/player/HeaderHUD';
import { useRoom } from '../hooks/useRoom';
import { mockBackend } from '../services/mockBackend';
import { roomApi } from '../services/ipc';
import { RoomStateSnapshot, Participant } from '../types/room';

describe('Milestone 4 Empirical Adversarial Verification Suite', () => {
  beforeEach(() => {
    mockBackend.reset();
    vi.clearAllMocks();
  });

  // =========================================================================
  // 1. CROCKFORD ROOM CODE INPUT NORMALIZATION & ADVERSARIAL FUZZING
  // =========================================================================
  describe('1. Crockford Room Code Input Normalization & Fuzzing', () => {
    it('normalizes lowercase, mixed whitespace, tabs, and hyphens', () => {
      const cases = [
        { input: 'k7m2p9', expected: 'K7M2P9' },
        { input: '  k7m-2p9  ', expected: 'K7M2P9' },
        { input: '\t\n  k-7-m-2-p-9  \r\n', expected: 'K7M2P9' },
        { input: '2 3 4 5 6 7', expected: '234567' },
        { input: '---abcdef---', expected: 'ABCDEF' },
        { input: '   Z Y X W V U   ', expected: 'ZYXWVU' },
      ];

      for (const { input, expected } of cases) {
        const normalized = normalizeRoomCode(input);
        expect(normalized).toBe(expected);
        expect(isValidRoomCode(normalized)).toBe(true);
      }
    });

    it('rejects punctuation, special symbols, and emojis in room code validation', () => {
      const invalidSymbols = [
        'K7M.2P9',
        'K7M_2P9',
        'K7M!2P9',
        'K7M@2P9',
        'K7M#2P9',
        'K7M$2P9',
        'K7M%2P9',
        'K7M^2P9',
        'K7M&2P9',
        'K7M*2P9',
        'K7M(2P9',
        'K7M)2P9',
        'K7M+2P9',
        'K7M=2P9',
        'K7M/2P9',
        'K7M\\2P9',
        'K7M:2P9',
        'K7M;2P9',
        'K7M"2P9',
        'K7M\'2P9',
        'K7M<2P9',
        'K7M>2P9',
        'K7M,2P9',
        'K7M?2P9',
        'K7M`2P9',
        'K7M~2P9',
        'K7M|2P9',
        '👍🎉🔥🚀😀❤️',
        'К7М2Р9', // Cyrillic lookalikes (U+041A, U+041C, U+0420)
        'K7M2P9\u200B', // Zero-width space
      ];

      for (const symbolCode of invalidSymbols) {
        expect(isValidRoomCode(symbolCode)).toBe(false);
      }
    });

    it('rigorously detects all forbidden Crockford ambiguous characters (0, O, 1, I)', () => {
      const forbiddenTests = [
        { code: '0BCDEF', expectedForbidden: ['0'] },
        { code: 'OBCDEF', expectedForbidden: ['O'] },
        { code: '1BCDEF', expectedForbidden: ['1'] },
        { code: 'IBCDEF', expectedForbidden: ['I'] },
        { code: '0O1IBC', expectedForbidden: ['0', 'O', '1', 'I'] },
        { code: 'i1o0k7', expectedForbidden: ['I', '1', 'O', '0'] },
        { code: 'K7M2P9', expectedForbidden: [] },
      ];

      for (const { code, expectedForbidden } of forbiddenTests) {
        const forbidden = getForbiddenCrockfordChars(code);
        expect(forbidden.sort()).toEqual(expectedForbidden.sort());
        if (expectedForbidden.length > 0) {
          expect(isValidRoomCode(code)).toBe(false);
        } else {
          expect(isValidRoomCode(code)).toBe(true);
        }
      }
    });

    it('parses diverse deep link URL variations (with/without token, trailing slashes, path tokens)', () => {
      const validToken = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

      // 1. Standard query param
      const url1 = `watchtogether://join/k7m2p9?token=${validToken}`;
      const res1 = parseJoinInput(url1);
      expect(res1.isUrl).toBe(true);
      expect(res1.roomCode).toBe('K7M2P9');
      expect(res1.roomToken).toBe(validToken);

      // 2. Trailing slash on path before query
      const url2 = `watchtogether://join/k7m2p9/?token=${validToken}`;
      const res2 = parseJoinInput(url2);
      expect(res2.isUrl).toBe(true);
      expect(res2.roomCode).toBe('K7M2P9');
      expect(res2.roomToken).toBe(validToken);

      // 3. Deep link without token
      const url3 = 'watchtogether://join/ABCDEF';
      const res3 = parseJoinInput(url3);
      expect(res3.isUrl).toBe(true);
      expect(res3.roomCode).toBe('ABCDEF');
      expect(res3.roomToken).toBeUndefined();

      // 4. Token passed in path segment
      const url4 = `watchtogether://join/ABCDEF/${validToken}`;
      const res4 = parseJoinInput(url4);
      expect(res4.isUrl).toBe(true);
      expect(res4.roomCode).toBe('ABCDEF');
      expect(res4.roomToken).toBe(validToken);

      // 5. Hyphenated and lowercase code in deep link
      const url5 = `watchtogether://join/k-7-m-2-p-9?token=${validToken}`;
      const res5 = parseJoinInput(url5);
      expect(res5.isUrl).toBe(true);
      expect(res5.roomCode).toBe('K7M2P9');
      expect(isValidRoomCode(res5.roomCode)).toBe(true);
    });

    it('executes 500-iteration random string fuzz test against normalizeRoomCode and isValidRoomCode', () => {
      const allPossibleChars =
        'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*()_+-=[]{}|;:\',.<>/?~` \t\n\r';

      for (let i = 0; i < 500; i++) {
        // Generate random length string between 0 and 20 chars
        const len = Math.floor(Math.random() * 21);
        let randomStr = '';
        for (let j = 0; j < len; j++) {
          const charIdx = Math.floor(Math.random() * allPossibleChars.length);
          randomStr += allPossibleChars[charIdx];
        }

        // Must not throw exception
        expect(() => {
          const norm = normalizeRoomCode(randomStr);
          const valid = isValidRoomCode(randomStr);

          if (valid) {
            // Invariant: if valid, normalized length must be 6 and every char in CROCKFORD_CHARSET
            expect(norm.length).toBe(6);
            for (const c of norm) {
              expect(CROCKFORD_CHARSET).toContain(c);
            }
          }
        }).not.toThrow();
      }
    });

    it('auto-sanitizes input and populates code and token when pasting deep link in JoinRoomModal', () => {
      const onJoinRoom = vi.fn().mockResolvedValue({});
      render(
        <JoinRoomModal
          isOpen={true}
          onClose={vi.fn()}
          initialDisplayName="Tester"
          isLoading={false}
          error={null}
          onJoinRoom={onJoinRoom}
        />
      );

      const codeInput = screen.getByTestId('room-code-input') as HTMLInputElement;
      const tokenInput = screen.getByTestId('room-token-input') as HTMLInputElement;
      const submitBtn = screen.getByTestId('join-room-submit-btn');

      // Before input, submit button must be disabled
      expect(submitBtn).toBeDisabled();

      // Paste full deep link
      const fullLink =
        'watchtogether://join/k7m-2p9?token=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
      fireEvent.change(codeInput, { target: { value: fullLink } });

      // Code must be normalized and token populated
      expect(codeInput.value).toBe('K7M2P9');
      expect(tokenInput.value).toBe(
        '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'
      );
      expect(submitBtn).not.toBeDisabled();
    });
  });

  // =========================================================================
  // 2. HOST AUTHORITY BYPASS ATTEMPTS
  // =========================================================================
  describe('2. Host Authority Bypass Attempts', () => {
    it('prevents simulated guest from executing host-only setPermissions via useRoom', async () => {
      const { result } = renderHook(() => useRoom());

      // Join room as guest
      await act(async () => {
        await result.current.joinRoom(
          'K7M2P9',
          '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
          'Guest Mallory'
        );
      });

      expect(result.current.isGuest).toBe(true);
      expect(result.current.isHost).toBe(false);
      expect(result.current.permissionMode).toBe('free_control');

      // Guest attempts to switch permissions to host_only
      await act(async () => {
        await result.current.setPermissions('host_only');
      });

      // Hook guard (if (!isHost) return) must prevent permission alteration
      expect(result.current.permissionMode).toBe('free_control');
    });

    it('prevents simulated guest from kicking participants via useRoom', async () => {
      const { result } = renderHook(() => useRoom());

      // Join room as guest
      await act(async () => {
        await result.current.joinRoom(
          'K7M2P9',
          '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
          'Guest Mallory'
        );
      });

      const initialCount = result.current.snapshot?.participants.length || 0;
      expect(initialCount).toBeGreaterThanOrEqual(2);

      // Guest attempts to kick the host ('part-host-1')
      await act(async () => {
        await result.current.kickParticipant('part-host-1');
      });

      // Hook guard (if (!isHost) return) must block kick action
      expect(result.current.snapshot?.participants.length).toBe(initialCount);
      expect(
        result.current.snapshot?.participants.some((p) => p.id === 'part-host-1')
      ).toBe(true);
    });

    it('hides permission toggles and kick buttons from non-host in ParticipantRosterPopover', () => {
      const snapshot: RoomStateSnapshot = {
        room_id: 'room-K7M2P9',
        room_code: 'K7M2P9',
        host_id: 'part-host',
        permission_mode: 'host_only',
        participants: [
          {
            id: 'part-host',
            display_name: 'Host Alice',
            role: 'host',
            is_connected: true,
            buffered_position_ms: 100000,
            current_position_ms: 5000,
            ping_ms: 15,
            joined_at_ms: 1000,
          },
          {
            id: 'part-guest',
            display_name: 'Guest Bob',
            role: 'guest',
            is_connected: true,
            buffered_position_ms: 80000,
            current_position_ms: 5000,
            ping_ms: 45,
            joined_at_ms: 2000,
          },
        ],
        playback: {
          revision: 1,
          status: 'playing',
          position_ms: 5000,
          speed: 1.0,
          updated_at_server_ms: 3000,
          duration_ms: 100000,
        },
        video: {
          filename: 'movie.mp4',
          file_size_bytes: 10000,
          duration_ms: 100000,
          mime_type: 'video/mp4',
        },
      };

      render(
        <ParticipantRosterPopover
          isOpen={true}
          onClose={vi.fn()}
          snapshot={snapshot}
          roomCode="K7M2P9"
          isHost={false} // NON-HOST GUEST
          permissionMode="host_only"
          onSetPermissions={vi.fn()}
          onKickParticipant={vi.fn()}
          onLeaveRoom={vi.fn()}
        />
      );

      // Host controls must NOT be present in DOM
      expect(screen.queryByTestId('toggle-perm-free')).not.toBeInTheDocument();
      expect(screen.queryByTestId('toggle-perm-host')).not.toBeInTheDocument();
      expect(screen.queryByTestId('kick-participant-part-host')).not.toBeInTheDocument();
      expect(screen.queryByTestId('kick-participant-part-guest')).not.toBeInTheDocument();

      // Non-host only has "Leave Room" button, NOT "End Room For All"
      expect(screen.getByTestId('roster-leave-room-btn')).toHaveTextContent('Leave Room');
      expect(screen.queryByText('End Room For All')).not.toBeInTheDocument();
    });

    it('intercepts guest playback actions in HostOnly mode and triggers authority toast', () => {
      const onTogglePlayPause = vi.fn();
      const onSeekRelative = vi.fn();
      const onUnauthorizedAction = vi.fn();

      render(
        <ControlsHUD
          isVisible={true}
          isPlaying={true}
          positionSec={50}
          durationSec={200}
          speed={1.0}
          volume={0.8}
          isMuted={false}
          isFullscreen={false}
          isScrubbing={false}
          canControlPlayback={false} // Intercepted for non-host
          audioTracks={[]}
          subtitleTracks={[]}
          onTogglePlayPause={onTogglePlayPause}
          onSeekRelative={onSeekRelative}
          onStartScrub={vi.fn()}
          onUpdateScrub={vi.fn()}
          onCommitScrub={vi.fn()}
          onSelectSpeed={vi.fn()}
          onVolumeChange={vi.fn()}
          onMuteToggle={vi.fn()}
          onSelectAudio={vi.fn()}
          onSelectSubtitle={vi.fn()}
          onLoadExternalSubtitle={vi.fn()}
          onToggleFullscreen={vi.fn()}
          onUnauthorizedAction={onUnauthorizedAction}
        />
      );

      // Play/Pause button click
      const playPauseBtn = screen.getByTestId('play-pause-btn');
      fireEvent.click(playPauseBtn);
      expect(onTogglePlayPause).not.toHaveBeenCalled();
      expect(onUnauthorizedAction).toHaveBeenCalledTimes(1);

      // Seek buttons click
      const seekBackBtn = screen.getByTestId('seek-backward-btn');
      fireEvent.click(seekBackBtn);
      expect(onSeekRelative).not.toHaveBeenCalled();
      expect(onUnauthorizedAction).toHaveBeenCalledTimes(2);

      const seekFwdBtn = screen.getByTestId('seek-forward-btn');
      fireEvent.click(seekFwdBtn);
      expect(onSeekRelative).not.toHaveBeenCalled();
      expect(onUnauthorizedAction).toHaveBeenCalledTimes(3);
    });

    it('keeps local unsynchronized controls interactive for guest in HostOnly mode', () => {
      const onVolumeChange = vi.fn();
      const onMuteToggle = vi.fn();
      const onToggleFullscreen = vi.fn();

      render(
        <ControlsHUD
          isVisible={true}
          isPlaying={true}
          positionSec={50}
          durationSec={200}
          speed={1.0}
          volume={0.8}
          isMuted={false}
          isFullscreen={false}
          isScrubbing={false}
          canControlPlayback={false} // Host-only restriction active
          audioTracks={[]}
          subtitleTracks={[]}
          onTogglePlayPause={vi.fn()}
          onSeekRelative={vi.fn()}
          onStartScrub={vi.fn()}
          onUpdateScrub={vi.fn()}
          onCommitScrub={vi.fn()}
          onSelectSpeed={vi.fn()}
          onVolumeChange={onVolumeChange}
          onMuteToggle={onMuteToggle}
          onSelectAudio={vi.fn()}
          onSelectSubtitle={vi.fn()}
          onLoadExternalSubtitle={vi.fn()}
          onToggleFullscreen={onToggleFullscreen}
          onUnauthorizedAction={vi.fn()}
        />
      );

      // Mute toggle must be operable
      const muteBtn = screen.getByTestId('mute-toggle-btn');
      fireEvent.click(muteBtn);
      expect(onMuteToggle).toHaveBeenCalledTimes(1);

      // Fullscreen toggle must be operable
      const fullscreenBtn = screen.getByTestId('fullscreen-btn');
      fireEvent.click(fullscreenBtn);
      expect(onToggleFullscreen).toHaveBeenCalledTimes(1);
    });
  });

  // =========================================================================
  // 3. ROOM CAPACITY LIMITS (2-5 PARTICIPANTS)
  // =========================================================================
  describe('3. Room Capacity Limits & Overflow Handling', () => {
    const createParticipants = (count: number): Participant[] => {
      const list: Participant[] = [
        {
          id: 'part-host',
          display_name: 'Host Alice',
          role: 'host',
          is_connected: true,
          buffered_position_ms: 100000,
          current_position_ms: 1000,
          ping_ms: 12,
          joined_at_ms: 1000,
        },
      ];
      for (let i = 1; i < count; i++) {
        list.push({
          id: `part-guest-${i}`,
          display_name: `Guest ${i}`,
          role: 'guest',
          is_connected: true,
          buffered_position_ms: 90000,
          current_position_ms: 1000,
          ping_ms: 30 + i * 20,
          joined_at_ms: 1000 + i * 100,
        });
      }
      return list;
    };

    it('renders exact capacity badge across 1, 2, 3, 4, and 5 participants', () => {
      for (let count = 1; count <= 5; count++) {
        const { unmount } = render(
          <HeaderHUD
            isVisible={true}
            currentFile="/sample/movie.mp4"
            sessionType="host"
            roomCode="K7M2P9"
            snapshot={{
              room_id: 'room-1',
              room_code: 'K7M2P9',
              host_id: 'part-host',
              permission_mode: 'free_control',
              participants: createParticipants(count),
              playback: {
                revision: 1,
                status: 'paused',
                position_ms: 0,
                speed: 1.0,
                updated_at_server_ms: 0,
                duration_ms: 100000,
              },
              video: {
                filename: 'movie.mp4',
                file_size_bytes: 1000,
                duration_ms: 100000,
                mime_type: 'video/mp4',
              },
            }}
            permissionMode="free_control"
            isHost={true}
            onOpenFileDialog={vi.fn()}
            onOpenCreateModal={vi.fn()}
            onOpenJoinModal={vi.fn()}
            onOpenSettingsModal={vi.fn()}
            onSetPermissions={vi.fn()}
            onKickParticipant={vi.fn()}
            onLeaveRoom={vi.fn()}
          />
        );

        const badge = screen.getByTestId('room-status-badge');
        expect(badge).toHaveTextContent(`(${count}/5)`);
        unmount();
      }
    });

    it('gracefully handles participant roster overflow (6+ participants) without crashing', () => {
      const overflowCount = 7;
      const overflowParticipants = createParticipants(overflowCount);

      const snapshot: RoomStateSnapshot = {
        room_id: 'room-overflow',
        room_code: 'K7M2P9',
        host_id: 'part-host',
        permission_mode: 'free_control',
        participants: overflowParticipants,
        playback: {
          revision: 1,
          status: 'playing',
          position_ms: 2000,
          speed: 1.0,
          updated_at_server_ms: 1000,
          duration_ms: 100000,
        },
        video: {
          filename: 'movie.mp4',
          file_size_bytes: 100000,
          duration_ms: 100000,
          mime_type: 'video/mp4',
        },
      };

      render(
        <ParticipantRosterPopover
          isOpen={true}
          onClose={vi.fn()}
          snapshot={snapshot}
          roomCode="K7M2P9"
          isHost={true}
          permissionMode="free_control"
          onSetPermissions={vi.fn()}
          onKickParticipant={vi.fn()}
          onLeaveRoom={vi.fn()}
        />
      );

      // Verify badge indicates 7/5
      expect(screen.getByTestId('roster-capacity-badge')).toHaveTextContent(`${overflowCount}/5`);

      // Verify all 7 participant rows rendered
      for (const p of overflowParticipants) {
        expect(screen.getByTestId(`participant-row-${p.id}`)).toBeInTheDocument();
      }
    });

    it('captures RoomFull rejection in JoinRoomModal and renders error banner', async () => {
      const rejectWithRoomFull = vi.fn().mockRejectedValue(new Error('Room is full (maximum 5 participants)'));

      render(
        <JoinRoomModal
          isOpen={true}
          onClose={vi.fn()}
          initialDisplayName="LateGuest"
          isLoading={false}
          error={null}
          onJoinRoom={rejectWithRoomFull}
        />
      );

      const codeInput = screen.getByTestId('room-code-input');
      const tokenInput = screen.getByTestId('room-token-input');
      const submitBtn = screen.getByTestId('join-room-submit-btn');

      fireEvent.change(codeInput, {
        target: {
          value: createConnectionCode('K7M2P9', '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef', 'eyJpIjoiaG9zdCJ9'),
        },
      });

      await act(async () => {
        fireEvent.click(submitBtn);
      });

      const banner = screen.getByTestId('join-error-banner');
      expect(banner).toBeInTheDocument();
      expect(banner).toHaveTextContent('Room is full (maximum 5 participants)');
    });
  });

  // =========================================================================
  // 4. ROOM TEARDOWN CASCADE & STATE CLEANUP
  // =========================================================================
  describe('4. Room Teardown Cascade', () => {
    it('cascades host session reset: resets sessionType, roomCode, roomToken, and snapshot', async () => {
      const { result } = renderHook(() => useRoom());

      // Host creates room
      await act(async () => {
        await result.current.createRoom('Host Alice', 'free_control', '/movie.mp4');
      });

      expect(result.current.sessionType).toBe('host');
      expect(result.current.roomCode).not.toBeNull();
      expect(result.current.roomToken).not.toBeNull();
      expect(result.current.snapshot).not.toBeNull();
      expect(result.current.isHost).toBe(true);

      // Host ends room
      await act(async () => {
        await result.current.leaveRoom();
      });

      // Complete state cleanup verification
      expect(result.current.sessionType).toBe('none');
      expect(result.current.roomCode).toBeNull();
      expect(result.current.roomToken).toBeNull();
      expect(result.current.snapshot).toBeNull();
      expect(result.current.isHost).toBe(false);
      expect(result.current.isGuest).toBe(false);
      expect(result.current.isSolo).toBe(true);
      expect(result.current.canControlPlayback).toBe(true);
    });

    it('cascades guest session reset when leaving room', async () => {
      const { result } = renderHook(() => useRoom());

      // Guest joins room
      await act(async () => {
        await result.current.joinRoom(
          'K7M2P9',
          '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
          'Guest Bob'
        );
      });

      expect(result.current.sessionType).toBe('guest');
      expect(result.current.isGuest).toBe(true);

      // Guest leaves room
      await act(async () => {
        await result.current.leaveRoom();
      });

      expect(result.current.sessionType).toBe('none');
      expect(result.current.roomCode).toBeNull();
      expect(result.current.roomToken).toBeNull();
      expect(result.current.snapshot).toBeNull();
      expect(result.current.isGuest).toBe(false);
      expect(result.current.isSolo).toBe(true);
    });

    it('cascades teardown when remote room:update event arrives with session_type: none', async () => {
      const { result } = renderHook(() => useRoom());

      // Guest joins room
      await act(async () => {
        await result.current.joinRoom(
          'K7M2P9',
          '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
          'Guest Charlie'
        );
      });

      expect(result.current.sessionType).toBe('guest');

      // Simulate host closing room -> backend broadcasts room://update with session_type none
      act(() => {
        mockBackend.emit('room://update', {
          session_type: 'none',
          room_code: null,
          snapshot: null,
        });
      });

      // Verification that guest hook cleans up immediately
      expect(result.current.sessionType).toBe('none');
      expect(result.current.roomCode).toBeNull();
      expect(result.current.roomToken).toBeNull();
      expect(result.current.snapshot).toBeNull();
      expect(result.current.isSolo).toBe(true);
      expect(result.current.canControlPlayback).toBe(true);
    });

    it('restores solo HUD state after teardown in HeaderHUD', () => {
      const onOpenCreateModal = vi.fn();
      const onOpenJoinModal = vi.fn();

      const { rerender } = render(
        <HeaderHUD
          isVisible={true}
          currentFile="/movie.mp4"
          sessionType="host"
          roomCode="K7M2P9"
          snapshot={{
            room_id: 'room-1',
            room_code: 'K7M2P9',
            host_id: 'part-host',
            permission_mode: 'free_control',
            participants: [
              {
                id: 'part-host',
                display_name: 'Host Alice',
                role: 'host',
                is_connected: true,
                buffered_position_ms: 1000,
                current_position_ms: 1000,
                ping_ms: 10,
                joined_at_ms: 1000,
              },
            ],
            playback: {
              revision: 1,
              status: 'playing',
              position_ms: 1000,
              speed: 1.0,
              updated_at_server_ms: 0,
              duration_ms: 10000,
            },
            video: {
              filename: 'movie.mp4',
              file_size_bytes: 1000,
              duration_ms: 10000,
              mime_type: 'video/mp4',
            },
          }}
          permissionMode="free_control"
          isHost={true}
          onOpenFileDialog={vi.fn()}
          onOpenCreateModal={onOpenCreateModal}
          onOpenJoinModal={onOpenJoinModal}
          onOpenSettingsModal={vi.fn()}
          onSetPermissions={vi.fn()}
          onKickParticipant={vi.fn()}
          onLeaveRoom={vi.fn()}
        />
      );

      // Room status badge is active
      expect(screen.getByTestId('room-status-badge')).toBeInTheDocument();
      expect(screen.queryByTestId('watch-together-btn')).not.toBeInTheDocument();

      // Teardown occurs: sessionType becomes 'none', roomCode null, snapshot null
      rerender(
        <HeaderHUD
          isVisible={true}
          currentFile="/movie.mp4"
          sessionType="none"
          roomCode={null}
          snapshot={null}
          permissionMode="free_control"
          isHost={false}
          onOpenFileDialog={vi.fn()}
          onOpenCreateModal={onOpenCreateModal}
          onOpenJoinModal={onOpenJoinModal}
          onOpenSettingsModal={vi.fn()}
          onSetPermissions={vi.fn()}
          onKickParticipant={vi.fn()}
          onLeaveRoom={vi.fn()}
        />
      );

      // Room status badge must be removed; Watch Together and Join Room buttons must return
      expect(screen.queryByTestId('room-status-badge')).not.toBeInTheDocument();
      expect(screen.getByTestId('watch-together-btn')).toBeInTheDocument();
      expect(screen.getByTestId('join-room-btn')).toBeInTheDocument();
    });
  });
});
