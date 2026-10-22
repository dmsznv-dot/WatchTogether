import React, { useState } from 'react';
import { Copy, Check, Users, Shield, Link2, Trash2 } from 'lucide-react';
import { Modal } from '../../components/ui/Modal';
import { Button } from '../../components/ui/Button';
import { PermissionMode } from '../../types/room';
import { createShareableLink, createConnectionCode } from '../../utils/deepLink';

export interface CreateRoomModalProps {
  isOpen: boolean;
  onClose: () => void;
  roomCode: string | null;
  roomToken: string | null;
  initialHostName: string;
  isLoading: boolean;
  error?: string | null;
  onCreateRoom: (hostName: string, permissionMode: PermissionMode) => Promise<unknown>;
  onEndRoom: () => Promise<unknown>;
}

export function CreateRoomModal({
  isOpen,
  onClose,
  roomCode,
  roomToken,
  initialHostName,
  isLoading,
  error,
  onCreateRoom,
  onEndRoom,
}: CreateRoomModalProps) {
  const [hostAddr, setHostAddr] = useState('');
  const [localError, setLocalError] = useState<string | null>(null);
  const [hostName, setHostName] = useState(initialHostName || 'Host');
  const [permissionMode, setPermissionMode] = useState<PermissionMode>('free_control');
  const [copiedCode, setCopiedCode] = useState(false);
  const [copiedLink, setCopiedLink] = useState(false);

  const shareableLink =
    roomCode && roomToken && hostAddr.trim() ? createShareableLink(roomCode, roomToken, hostAddr) : '';

  const handleCopyCode = async () => {
    if (!roomCode || !roomToken || !hostAddr.trim()) return;
    try {
      await navigator.clipboard.writeText(createConnectionCode(roomCode, roomToken, hostAddr));
      setCopiedCode(true);
      setTimeout(() => setCopiedCode(false), 2000);
    } catch {
      // Fallback
    }
  };

  const handleCopyLink = async () => {
    if (!shareableLink) return;
    try {
      await navigator.clipboard.writeText(shareableLink);
      setCopiedLink(true);
      setTimeout(() => setCopiedLink(false), 2000);
    } catch {
      // Fallback
    }
  };

  const handleCreate = async () => {
    setLocalError(null);
    try { await onCreateRoom(hostName.trim() || 'Host', permissionMode); } catch (err) { setLocalError(String(err)); }
  };

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title={roomCode ? 'Room Created' : 'Start Watch Together'}
      description={
        roomCode
          ? 'Share your room code or link with friends to watch in sync.'
          : 'Create a local room server to watch this video together.'
      }
    >
      <div className="flex flex-col gap-5 pt-2">
        {(error || localError) && <div role="alert" className="text-sm text-rose-300">{localError || error}</div>}
        <p className="text-xs text-zinc-400">Each person opens their own copy of the movie. This computer runs the sync server while the room is open.</p>
        <label className="text-xs text-zinc-400">Host address for invitations (IP or hostname, port 8999)
          <input data-testid="host-address-input" value={hostAddr} onChange={e => setHostAddr(e.target.value)} placeholder="203.0.113.4:8999" className="mt-1 w-full p-2 rounded-xl bg-zinc-900 border border-zinc-700 text-zinc-100" />
        </label>
        <p className="text-xs text-zinc-400">Internet: allow TCP 8999 through the host firewall and forward it on the router, or use a shared VPN. The short room code alone does not locate a computer.</p>
        {!roomCode ? (
          // Pre-creation form
          <>
            <div>
              <label className="block text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-1.5">
                Your Display Name
              </label>
              <input
                type="text"
                value={hostName}
                onChange={(e) => setHostName(e.target.value)}
                placeholder="Host name"
                className="w-full px-3.5 py-2 rounded-xl bg-zinc-900 border border-zinc-700 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                data-testid="host-name-input"
              />
            </div>

            <div>
              <label className="block text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-1.5">
                Playback Permissions
              </label>
              <div className="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  onClick={() => setPermissionMode('free_control')}
                  className={`flex flex-col items-center justify-center p-3 rounded-xl border text-center transition-all ${
                    permissionMode === 'free_control'
                      ? 'bg-indigo-950/60 border-indigo-500 text-white shadow-md'
                      : 'bg-zinc-900/60 border-zinc-800 text-zinc-400 hover:text-zinc-200'
                  }`}
                  data-testid="perm-free-btn"
                >
                  <Users className="w-5 h-5 mb-1 text-indigo-400" />
                  <span className="text-xs font-semibold">Free Control</span>
                  <span className="text-[10px] text-zinc-400 mt-0.5">
                    Anyone can play/pause/seek
                  </span>
                </button>

                <button
                  type="button"
                  onClick={() => setPermissionMode('host_only')}
                  className={`flex flex-col items-center justify-center p-3 rounded-xl border text-center transition-all ${
                    permissionMode === 'host_only'
                      ? 'bg-indigo-950/60 border-indigo-500 text-white shadow-md'
                      : 'bg-zinc-900/60 border-zinc-800 text-zinc-400 hover:text-zinc-200'
                  }`}
                  data-testid="perm-host-btn"
                >
                  <Shield className="w-5 h-5 mb-1 text-amber-400" />
                  <span className="text-xs font-semibold">Host Only</span>
                  <span className="text-[10px] text-zinc-400 mt-0.5">
                    Only host controls playback
                  </span>
                </button>
              </div>
            </div>

            <Button
              variant="primary"
              size="lg"
              disabled={isLoading}
              onClick={handleCreate}
              className="w-full mt-2"
              data-testid="create-room-submit-btn"
            >
              {isLoading ? 'Creating Room...' : 'Create Room'}
            </Button>
          </>
        ) : (
          // Active Room Details
          <>
            <div className="flex flex-col items-center justify-center p-4 rounded-2xl bg-zinc-950/90 border border-zinc-800">
              <span className="text-[11px] font-semibold uppercase tracking-wider text-zinc-400 mb-1">
                Room Code (Crockford Base32)
              </span>
              <div
                className="font-mono text-3xl font-bold tracking-widest text-indigo-400 select-all"
                data-testid="room-code-display"
              >
                {roomCode}
              </div>
              <Button
                variant="secondary"
                size="sm"
                onClick={handleCopyCode}
                disabled={!hostAddr.trim()}
                className="mt-3"
                data-testid="copy-code-btn"
              >
                {copiedCode ? (
                  <>
                    <Check className="w-3.5 h-3.5 text-emerald-400" />
                    <span>Copied!</span>
                  </>
                ) : (
                  <>
                    <Copy className="w-3.5 h-3.5" />
                    <span>Copy Connection Code</span>
                  </>
                )}
              </Button>
            </div>

            <div>
              <label className="block text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-1.5">
                Shareable Link
              </label>
              <div className="flex items-center gap-2">
                <input
                  type="text"
                  readOnly
                  value={shareableLink}
                  className="w-full px-3 py-2 rounded-xl bg-zinc-900 border border-zinc-800 text-xs font-mono text-zinc-300 truncate select-all focus:outline-none"
                  data-testid="shareable-link-input"
                />
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={handleCopyLink}
                  disabled={!shareableLink}
                  className="shrink-0"
                  data-testid="copy-link-btn"
                >
                  {copiedLink ? (
                    <Check className="w-4 h-4 text-emerald-400" />
                  ) : (
                    <Link2 className="w-4 h-4" />
                  )}
                </Button>
              </div>
            </div>

            <div className="flex items-center justify-between pt-2 border-t border-zinc-800">
              <Button
                variant="danger"
                size="sm"
                onClick={async () => {
                  await onEndRoom();
                  onClose();
                }}
                data-testid="end-room-btn"
              >
                <Trash2 className="w-3.5 h-3.5 mr-1" />
                End Room
              </Button>
              <Button variant="primary" size="sm" onClick={onClose}>
                Done
              </Button>
            </div>
          </>
        )}
      </div>
    </Modal>
  );
}
