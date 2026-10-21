import React, { useState } from 'react';
import {
  Users,
  FolderOpen,
  Settings,
  Lock,
  Unlock,
  PlusCircle,
  LogIn,
} from 'lucide-react';
import { Button } from '../../components/ui/Button';
import { ParticipantRosterPopover } from '../rooms/ParticipantRosterPopover';
import { PermissionMode, RoomStateSnapshot, SessionType } from '../../types/room';
import { cn } from '../../utils/cn';

export interface HeaderHUDProps {
  isVisible: boolean;
  currentFile: string | null;
  sessionType: SessionType;
  roomCode: string | null;
  snapshot: RoomStateSnapshot | null;
  permissionMode: PermissionMode;
  isHost: boolean;
  onOpenFileDialog: () => void;
  onOpenCreateModal: () => void;
  onOpenJoinModal: () => void;
  onOpenSettingsModal: () => void;
  onSetPermissions: (mode: PermissionMode) => void;
  onKickParticipant: (targetId: string) => void;
  onLeaveRoom: () => void;
  bindHover?: {
    onMouseEnter: () => void;
    onMouseLeave: () => void;
  };
}

export function HeaderHUD({
  isVisible,
  currentFile,
  sessionType,
  roomCode,
  snapshot,
  permissionMode,
  isHost,
  onOpenFileDialog,
  onOpenCreateModal,
  onOpenJoinModal,
  onOpenSettingsModal,
  onSetPermissions,
  onKickParticipant,
  onLeaveRoom,
  bindHover,
}: HeaderHUDProps) {
  const [isRosterOpen, setIsRosterOpen] = useState(false);

  const filename = currentFile ? currentFile.split('/').pop() || currentFile : null;
  const participantCount = snapshot?.participants.length || (sessionType === 'host' ? 1 : 0);

  return (
    <div
      {...bindHover}
      data-testid="header-hud"
      className={cn(
        'absolute top-0 inset-x-0 p-4 transition-all duration-300 z-30 select-none pointer-events-auto',
        isVisible || isRosterOpen ? 'opacity-100 translate-y-0' : 'opacity-0 -translate-y-3 pointer-events-none'
      )}
    >
      <div className="max-w-6xl mx-auto flex items-center justify-between gap-4">
        {/* Left: Room Status Badge or Start/Join Buttons */}
        <div className="flex items-center gap-2 relative">
          {sessionType !== 'none' && roomCode ? (
            <>
              {/* Room Status Badge */}
              <button
                onClick={() => setIsRosterOpen(!isRosterOpen)}
                data-testid="room-status-badge"
                className={cn(
                  'flex items-center gap-2 px-3 py-1.5 rounded-xl glass-panel border transition-all text-xs font-medium',
                  isRosterOpen
                    ? 'border-indigo-400 bg-indigo-950/70 text-white'
                    : 'border-white/10 hover:border-white/20 text-zinc-200'
                )}
              >
                <div className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                <span className="font-mono font-bold tracking-wider text-indigo-300">
                  {roomCode}
                </span>
                <span className="text-[11px] text-zinc-400 font-mono">
                  ({participantCount}/5)
                </span>
                {permissionMode === 'host_only' ? (
                  <Lock className="w-3.5 h-3.5 text-amber-400 ml-0.5" />
                ) : (
                  <Unlock className="w-3.5 h-3.5 text-indigo-400 ml-0.5" />
                )}
              </button>

              {/* Roster Popover */}
              <ParticipantRosterPopover
                isOpen={isRosterOpen}
                onClose={() => setIsRosterOpen(false)}
                snapshot={snapshot}
                roomCode={roomCode}
                isHost={isHost}
                permissionMode={permissionMode}
                onSetPermissions={onSetPermissions}
                onKickParticipant={onKickParticipant}
                onLeaveRoom={onLeaveRoom}
              />
            </>
          ) : (
            <>
              {/* Watch Together Button (Disabled if no media is loaded) */}
              <Button
                variant="primary"
                size="sm"
                disabled={!currentFile}
                onClick={onOpenCreateModal}
                data-testid="watch-together-btn"
                title={!currentFile ? 'Open a video file first to create a room' : 'Create Room'}
              >
                <PlusCircle className="w-4 h-4" />
                <span>Watch Together</span>
              </Button>

              {/* Join Room Button */}
              <Button
                variant="secondary"
                size="sm"
                onClick={onOpenJoinModal}
                data-testid="join-room-btn"
              >
                <LogIn className="w-4 h-4" />
                <span>Join Room</span>
              </Button>
            </>
          )}
        </div>

        {/* Center: Video Title */}
        <div
          data-testid="video-title"
          className="text-xs sm:text-sm font-medium text-zinc-300 truncate max-w-md px-3 py-1 rounded-lg bg-black/40 backdrop-blur border border-white/5"
        >
          {filename || 'Watch Together'}
        </div>

        {/* Right: Open File & Settings */}
        <div className="flex items-center gap-2">
          <Button
            variant="secondary"
            size="sm"
            onClick={onOpenFileDialog}
            data-testid="open-file-btn"
            title="Open Video File (Cmd+O)"
          >
            <FolderOpen className="w-4 h-4" />
            <span className="hidden sm:inline">Open</span>
          </Button>

          <Button
            variant="ghost"
            size="icon"
            onClick={onOpenSettingsModal}
            data-testid="settings-btn"
            title="Settings & Shortcuts"
          >
            <Settings className="w-4 h-4" />
          </Button>
        </div>
      </div>
    </div>
  );
}
