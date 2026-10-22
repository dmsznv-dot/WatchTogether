import React, { useState, useEffect, ChangeEvent } from 'react';
import { AlertCircle, LogIn } from 'lucide-react';
import { Modal } from '../../components/ui/Modal';
import { Button } from '../../components/ui/Button';
import {
  getForbiddenCrockfordChars,
  isValidRoomCode,
  normalizeRoomCode,
} from '../../utils/crockford';
import { parseJoinInput } from '../../utils/deepLink';

export interface JoinRoomModalProps {
  isOpen: boolean;
  onClose: () => void;
  initialDisplayName: string;
  isLoading: boolean;
  error: string | null;
  initialInvitation?: string;
  onJoinRoom: (code: string, token: string, displayName: string, hostAddr?: string) => Promise<unknown>;
}

export function JoinRoomModal({
  isOpen,
  onClose,
  initialDisplayName,
  isLoading,
  error,
  initialInvitation,
  onJoinRoom,
}: JoinRoomModalProps) {
  const [codeOrUrl, setCodeOrUrl] = useState('');
  const [hostAddr, setHostAddr] = useState('');
  const [token, setToken] = useState('');
  const [displayName, setDisplayName] = useState(initialDisplayName || 'Guest');
  const [localError, setLocalError] = useState<string | null>(null);

  useEffect(() => {
    if (!initialInvitation) return;
    const parsed = parseJoinInput(initialInvitation);
    setCodeOrUrl(parsed.roomCode);
    setToken(parsed.roomToken || '');
    setHostAddr(parsed.hostAddr || '');
    setLocalError(null);
  }, [initialInvitation]);

  const forbiddenChars = getForbiddenCrockfordChars(codeOrUrl);

  const handleCodeOrUrlChange = (e: ChangeEvent<HTMLInputElement>) => {
    const raw = e.target.value;
    setLocalError(null);

    // If a full deep link is pasted:
    if (raw.trim().startsWith('watchtogether://') || raw.trim().startsWith('WT1-')) {
      const parsed = parseJoinInput(raw);
      setHostAddr(parsed.hostAddr || '');
      setCodeOrUrl(parsed.roomCode);
      if (parsed.roomToken) {
        setToken(parsed.roomToken);
      }
      return;
    }

    // Otherwise apply uppercase Crockford sanitization
    setCodeOrUrl(normalizeRoomCode(raw));
  };

  const handleJoin = async () => {
    setLocalError(null);
    const sanitizedCode = normalizeRoomCode(codeOrUrl);
    if (!isValidRoomCode(sanitizedCode)) {
      setLocalError('Please enter a valid 6-character room code (e.g. K7M2P9)');
      return;
    }
    if (!token.trim() || token.trim().length < 32) {
      setLocalError('Please provide a valid room authorization token (32+ characters)');
      return;
    }

    if (!hostAddr.trim()) { setLocalError('Provide the host address or paste a complete invitation.'); return; }
    try {
      await onJoinRoom(sanitizedCode, token.trim(), displayName.trim() || 'Guest', hostAddr.trim());
      onClose();
    } catch (err: any) {
      setLocalError(err?.message || 'Failed to join room');
    }
  };

  const isCodeValid = isValidRoomCode(normalizeRoomCode(codeOrUrl));

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Join Watch Together Room"
      description="Open your own copy of the movie first, then paste the complete invitation link or connection code."
    >
      <div className="flex flex-col gap-4 pt-1">
        {(error || localError) && (
          <div
            data-testid="join-error-banner"
            className="flex items-center gap-2 p-3 rounded-xl bg-rose-950/80 border border-rose-800 text-xs text-rose-200"
          >
            <AlertCircle className="w-4 h-4 text-rose-400 shrink-0" />
            <span>{localError || error}</span>
          </div>
        )}

        <div>
          <label className="block text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-1.5">
            Room Code or Shareable Link
          </label>
          <input
            type="text"
            value={codeOrUrl}
            onChange={handleCodeOrUrlChange}
            placeholder="e.g. K7M2P9 or watchtogether://..."
            maxLength={2048}
            className="w-full px-3.5 py-2.5 rounded-xl bg-zinc-900 border border-zinc-700 font-mono tracking-wider text-base text-zinc-100 placeholder-zinc-500 uppercase focus:outline-none focus:border-indigo-500"
            data-testid="room-code-input"
          />
          {forbiddenChars.length > 0 && (
            <p
              data-testid="crockford-forbidden-hint"
              className="text-[11px] text-amber-400 mt-1"
            >
              Notice: Crockford Base32 excludes 0, O, 1, I ({forbiddenChars.join(', ')} entered).
            </p>
          )}
        </div>

        <label className="text-xs text-zinc-400">Host address
          <input data-testid="join-host-address-input" value={hostAddr} onChange={e => setHostAddr(e.target.value)} placeholder="IP or hostname:8999" className="mt-1 w-full p-2 rounded-xl bg-zinc-900 border border-zinc-700 text-zinc-100" />
        </label>

        <div>
          <label className="block text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-1.5">
            Room Token
          </label>
          <input
            type="text"
            value={token}
            onChange={(e) => setToken(e.target.value.trim())}
            placeholder="32+ character authorization token"
            className="w-full px-3.5 py-2 rounded-xl bg-zinc-900 border border-zinc-700 text-xs font-mono text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
            data-testid="room-token-input"
          />
        </div>

        <div>
          <label className="block text-xs font-semibold uppercase tracking-wider text-zinc-400 mb-1.5">
            Your Display Name
          </label>
          <input
            type="text"
            value={displayName}
            onChange={(e) => setDisplayName(e.target.value)}
            placeholder="Guest"
            className="w-full px-3.5 py-2 rounded-xl bg-zinc-900 border border-zinc-700 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
            data-testid="display-name-input"
          />
        </div>

        <Button
          variant="primary"
          size="lg"
          disabled={isLoading || !isCodeValid || token.length < 32}
          onClick={handleJoin}
          className="w-full mt-2"
          data-testid="join-room-submit-btn"
        >
          <LogIn className="w-4 h-4 mr-1.5" />
          {isLoading ? 'Joining Room...' : 'Join Room'}
        </Button>
      </div>
    </Modal>
  );
}
