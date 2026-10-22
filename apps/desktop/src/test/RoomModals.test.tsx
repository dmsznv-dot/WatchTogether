import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { CreateRoomModal } from '../features/rooms/CreateRoomModal';
import { JoinRoomModal } from '../features/rooms/JoinRoomModal';
import { ParticipantRosterPopover } from '../features/rooms/ParticipantRosterPopover';
import { RoomStateSnapshot } from '../types/room';

describe('Room Collaboration UI Modals', () => {
  describe('CreateRoomModal', () => {
    it('renders room code and handles copy button', async () => {
      const onClose = vi.fn();
      render(
        <CreateRoomModal
          isOpen={true}
          onClose={onClose}
          roomCode="K7M2P9"
          roomToken="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
          initialHostName="Alice"
          isLoading={false}
          onCreateRoom={vi.fn()}
          onEndRoom={vi.fn()}
        />
      );

      expect(screen.getByTestId('room-code-display')).toHaveTextContent('K7M2P9');

      fireEvent.change(screen.getByTestId('host-address-input'), {target:{value:'192.168.1.25:8999'}});
      const copyBtn = screen.getByTestId('copy-code-btn');
      await act(async () => {
        fireEvent.click(copyBtn);
      });

      expect(screen.getByText('Copied!')).toBeInTheDocument();
    });
  });

  describe('JoinRoomModal', () => {
    it('passes the host address from a complete invitation to the connection command', async () => {
      const join = vi.fn().mockResolvedValue({});
      render(<JoinRoomModal isOpen onClose={vi.fn()} initialDisplayName="Bob" isLoading={false} error={null} onJoinRoom={join} />);
      fireEvent.change(screen.getByTestId('room-code-input'), {target:{value:'watchtogether://join/K7M2P9?token='+'a'.repeat(64)+'&host=203.0.113.4%3A8999'}});
      expect(screen.getByTestId('join-host-address-input')).toHaveValue('203.0.113.4:8999');
      await act(async () => { fireEvent.click(screen.getByTestId('join-room-submit-btn')); });
      expect(join).toHaveBeenCalledWith('K7M2P9','a'.repeat(64),'Bob','203.0.113.4:8999');
    });
    it('normalizes room code input to uppercase', () => {
      render(
        <JoinRoomModal
          isOpen={true}
          onClose={vi.fn()}
          initialDisplayName="Bob"
          isLoading={false}
          error={null}
          onJoinRoom={vi.fn()}
        />
      );

      const codeInput = screen.getByTestId('room-code-input') as HTMLInputElement;
      fireEvent.change(codeInput, { target: { value: '  k7m-2p9  ' } });

      expect(codeInput.value).toBe('K7M2P9');
    });

    it('warns when forbidden Crockford characters are typed', () => {
      render(
        <JoinRoomModal
          isOpen={true}
          onClose={vi.fn()}
          initialDisplayName="Bob"
          isLoading={false}
          error={null}
          onJoinRoom={vi.fn()}
        />
      );

      const codeInput = screen.getByTestId('room-code-input');
      fireEvent.change(codeInput, { target: { value: 'K7M2P0' } });

      expect(screen.getByTestId('crockford-forbidden-hint')).toBeInTheDocument();
    });

    it('auto-extracts code and token from deep link paste', () => {
      render(
        <JoinRoomModal
          isOpen={true}
          onClose={vi.fn()}
          initialDisplayName="Bob"
          isLoading={false}
          error={null}
          onJoinRoom={vi.fn()}
        />
      );

      const codeInput = screen.getByTestId('room-code-input') as HTMLInputElement;
      const tokenInput = screen.getByTestId('room-token-input') as HTMLInputElement;

      fireEvent.change(codeInput, {
        target: {
          value:
            'watchtogether://join/K7M2P9?token=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
        },
      });

      expect(codeInput.value).toBe('K7M2P9');
      expect(tokenInput.value).toBe(
        '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef'
      );
    });
  });

  describe('ParticipantRosterPopover', () => {
    const mockSnapshot: RoomStateSnapshot = {
      room_id: 'room-K7M2P9',
      room_code: 'K7M2P9',
      host_id: 'part-host',
      permission_mode: 'free_control',
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
          id: 'part-guest-1',
          display_name: 'Guest Bob',
          role: 'guest',
          is_connected: true,
          buffered_position_ms: 80000,
          current_position_ms: 5000,
          ping_ms: 85,
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

    it('renders participant roster and host controls', () => {
      const onSetPermissions = vi.fn();
      const onKickParticipant = vi.fn();

      render(
        <ParticipantRosterPopover
          isOpen={true}
          onClose={vi.fn()}
          snapshot={mockSnapshot}
          roomCode="K7M2P9"
          isHost={true}
          permissionMode="free_control"
          onSetPermissions={onSetPermissions}
          onKickParticipant={onKickParticipant}
          onLeaveRoom={vi.fn()}
        />
      );

      expect(screen.getByText('Host Alice')).toBeInTheDocument();
      expect(screen.getByText('Guest Bob')).toBeInTheDocument();
      expect(screen.getByTestId('roster-capacity-badge')).toHaveTextContent('2/5');

      // Check kick button exists for guest
      expect(screen.getByTestId('kick-participant-part-guest-1')).toBeInTheDocument();

      // Check permission mode toggle
      const hostOnlyBtn = screen.getByTestId('toggle-perm-host');
      fireEvent.click(hostOnlyBtn);
      expect(onSetPermissions).toHaveBeenCalledWith('host_only');
    });
  });
});
