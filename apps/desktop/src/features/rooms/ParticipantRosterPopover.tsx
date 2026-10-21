import React, { useRef, useEffect, useState } from 'react';
import {
  Crown,
  Wifi,
  Shield,
  Users,
  UserX,
  LogOut,
  Trash2,
  Copy,
  Check,
} from 'lucide-react';
import { Participant, PermissionMode, RoomStateSnapshot } from '../../types/room';
import { Button } from '../../components/ui/Button';
import { cn } from '../../utils/cn';

export interface ParticipantRosterPopoverProps {
  isOpen: boolean;
  onClose: () => void;
  snapshot: RoomStateSnapshot | null;
  roomCode: string | null;
  isHost: boolean;
  permissionMode: PermissionMode;
  onSetPermissions: (mode: PermissionMode) => void;
  onKickParticipant: (targetId: string) => void;
  onLeaveRoom: () => void;
}

export function ParticipantRosterPopover({
  isOpen,
  onClose,
  snapshot,
  roomCode,
  isHost,
  permissionMode,
  onSetPermissions,
  onKickParticipant,
  onLeaveRoom,
}: ParticipantRosterPopoverProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [copied, setCopied] = useState(false);
  const [kickingId, setKickingId] = useState<string | null>(null);

  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const participants: Participant[] = snapshot?.participants || [];
  const participantCount = participants.length;

  const handleCopyCode = async () => {
    if (!roomCode) return;
    try {
      await navigator.clipboard.writeText(roomCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback
    }
  };

  const getPingColor = (ping: number) => {
    if (ping <= 100) return 'text-emerald-400';
    if (ping <= 250) return 'text-amber-400';
    return 'text-rose-400';
  };

  return (
    <div
      ref={containerRef}
      data-testid="participant-roster-popover"
      className="absolute top-12 left-4 w-80 rounded-2xl glass-dropdown p-4 z-50 shadow-2xl border border-zinc-700/80 animate-in fade-in zoom-in-95 duration-150 select-none text-zinc-100"
    >
      {/* Header Info */}
      <div className="flex items-center justify-between pb-3 border-b border-zinc-700/60">
        <div className="flex items-center gap-2">
          <span className="text-xs font-semibold uppercase tracking-wider text-zinc-400">
            Room Code:
          </span>
          <button
            onClick={handleCopyCode}
            data-testid="roster-copy-code-btn"
            className="flex items-center gap-1 font-mono font-bold text-sm text-indigo-400 hover:text-indigo-300"
          >
            <span>{roomCode}</span>
            {copied ? (
              <Check className="w-3.5 h-3.5 text-emerald-400" />
            ) : (
              <Copy className="w-3.5 h-3.5 text-zinc-400" />
            )}
          </button>
        </div>

        <div
          data-testid="roster-capacity-badge"
          className="text-[11px] font-mono px-2 py-0.5 rounded-full bg-zinc-800 text-zinc-300 border border-zinc-700"
        >
          {participantCount}/5
        </div>
      </div>

      {/* Host Permission Controls */}
      {isHost && (
        <div className="my-3 p-2.5 rounded-xl bg-zinc-900/80 border border-zinc-800">
          <div className="text-[10px] font-semibold text-zinc-400 uppercase tracking-wider mb-1.5 flex items-center justify-between">
            <span>Playback Authority</span>
            <span className="text-indigo-400 font-mono text-[9px]">HOST SETTING</span>
          </div>

          <div className="grid grid-cols-2 gap-1.5">
            <button
              onClick={() => onSetPermissions('free_control')}
              data-testid="toggle-perm-free"
              className={cn(
                'flex items-center justify-center gap-1.5 py-1.5 px-2 rounded-lg text-xs font-medium border transition-colors',
                permissionMode === 'free_control'
                  ? 'bg-indigo-600 text-white border-indigo-500 font-bold'
                  : 'bg-zinc-800 text-zinc-400 border-zinc-700 hover:text-white'
              )}
            >
              <Users className="w-3.5 h-3.5" />
              <span>Free Control</span>
            </button>

            <button
              onClick={() => onSetPermissions('host_only')}
              data-testid="toggle-perm-host"
              className={cn(
                'flex items-center justify-center gap-1.5 py-1.5 px-2 rounded-lg text-xs font-medium border transition-colors',
                permissionMode === 'host_only'
                  ? 'bg-indigo-600 text-white border-indigo-500 font-bold'
                  : 'bg-zinc-800 text-zinc-400 border-zinc-700 hover:text-white'
              )}
            >
              <Shield className="w-3.5 h-3.5" />
              <span>Host Only</span>
            </button>
          </div>
        </div>
      )}

      {/* Participant Roster List */}
      <div className="my-2">
        <div className="text-[10px] font-semibold text-zinc-400 uppercase tracking-wider mb-2">
          Connected Participants
        </div>

        <div className="flex flex-col gap-1 max-h-48 overflow-y-auto pr-0.5">
          {participants.length === 0 ? (
            <div className="text-xs text-zinc-500 py-2 italic text-center">
              No participants connected
            </div>
          ) : (
            participants.map((p) => {
              const isParticipantHost = p.role === 'host';
              return (
                <div
                  key={p.id}
                  data-testid={`participant-row-${p.id}`}
                  className="flex items-center justify-between p-2 rounded-xl bg-zinc-900/60 hover:bg-zinc-850 border border-zinc-800/80 text-xs"
                >
                  <div className="flex items-center gap-2 truncate mr-2">
                    {isParticipantHost ? (
                      <Crown className="w-4 h-4 text-amber-400 shrink-0" />
                    ) : (
                      <div className="w-2 h-2 rounded-full bg-emerald-500 shrink-0" />
                    )}
                    <span className="font-medium truncate text-zinc-200">
                      {p.display_name}
                    </span>
                    {isParticipantHost && (
                      <span className="text-[9px] uppercase tracking-wider text-amber-300 bg-amber-950/60 px-1 py-0.2 rounded border border-amber-800/40">
                        Host
                      </span>
                    )}
                  </div>

                  <div className="flex items-center gap-2 shrink-0">
                    {/* Ping Latency */}
                    <div
                      className={cn(
                        'flex items-center gap-1 font-mono text-[11px]',
                        getPingColor(p.ping_ms)
                      )}
                      title={`Ping: ${p.ping_ms}ms`}
                    >
                      <Wifi className="w-3 h-3" />
                      <span>{p.ping_ms}ms</span>
                    </div>

                    {/* Host Kick Action */}
                    {isHost && !isParticipantHost && (
                      <button
                        onClick={() => {
                          if (kickingId === p.id) {
                            onKickParticipant(p.id);
                            setKickingId(null);
                          } else {
                            setKickingId(p.id);
                            setTimeout(() => setKickingId(null), 3000);
                          }
                        }}
                        data-testid={`kick-participant-${p.id}`}
                        className={cn(
                          'p-1 rounded-md text-zinc-400 hover:text-rose-400 hover:bg-rose-950/50 transition-colors',
                          kickingId === p.id && 'text-rose-400 bg-rose-950/80 font-bold px-1.5'
                        )}
                        title={kickingId === p.id ? 'Click again to confirm kick' : 'Kick participant'}
                      >
                        {kickingId === p.id ? (
                          <span className="text-[10px]">Confirm?</span>
                        ) : (
                          <UserX className="w-3.5 h-3.5" />
                        )}
                      </button>
                    )}
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>

      {/* Footer Teardown / Leave Action */}
      <div className="mt-3 pt-3 border-t border-zinc-700/60 flex justify-end">
        <Button
          variant="danger"
          size="sm"
          onClick={() => {
            onLeaveRoom();
            onClose();
          }}
          data-testid="roster-leave-room-btn"
          className="w-full justify-center"
        >
          {isHost ? (
            <>
              <Trash2 className="w-3.5 h-3.5 mr-1.5" />
              <span>End Room For All</span>
            </>
          ) : (
            <>
              <LogOut className="w-3.5 h-3.5 mr-1.5" />
              <span>Leave Room</span>
            </>
          )}
        </Button>
      </div>
    </div>
  );
}
