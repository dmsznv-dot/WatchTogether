import { normalizeRoomCode } from './crockford';

export const DEEP_LINK_SCHEME = 'watchtogether';
/** Invitations carry the host's iroh address (`hostAddr`), so guests need no IP or port. */
export const CONNECTION_CODE_PREFIX = 'WT2-';
/** IP-based invitations from earlier versions; they cannot reach an iroh host. */
const LEGACY_CODE_PREFIX = 'WT1-';

export function createShareableLink(roomCode: string, roomToken: string, hostAddr?: string): string {
  return `${DEEP_LINK_SCHEME}://join/${roomCode}?token=${encodeURIComponent(roomToken)}${hostAddr ? `&host=${encodeURIComponent(hostAddr.trim())}` : ''}`;
}

export interface ParsedJoinInput {
  roomCode: string;
  roomToken?: string;
  hostAddr?: string;
  isUrl: boolean;
  /** Set for an invitation from an older, IP-based version of the app. */
  isLegacy?: boolean;
}

export function parseJoinInput(raw: string): ParsedJoinInput {
  const trimmed = raw.replace(/\s+/g, '');
  if (trimmed.startsWith(LEGACY_CODE_PREFIX)) {
    return {roomCode:'',isUrl:true,isLegacy:true};
  }
  if (trimmed.startsWith(CONNECTION_CODE_PREFIX)) {
    try {
      const encoded = trimmed.slice(CONNECTION_CODE_PREFIX.length).replace(/-/g, '+').replace(/_/g, '/');
      const data = JSON.parse(decodeURIComponent(atob(encoded)));
      if (typeof data.roomCode !== 'string' || typeof data.roomToken !== 'string' || typeof data.hostAddr !== 'string') throw new Error('Invalid invitation');
      return {roomCode: normalizeRoomCode(data.roomCode), roomToken: data.roomToken, hostAddr: data.hostAddr, isUrl: true};
    } catch { return {roomCode:'',isUrl:true}; }
  }
  const prefix = `${DEEP_LINK_SCHEME}://join/`;

  if (trimmed.startsWith(prefix)) {
    const after = trimmed.slice(prefix.length);
    const [pathPart, queryPart] = after.split('?');
    const segments = pathPart.split('/').filter(Boolean);
    const codePart = normalizeRoomCode(segments[0] || '');
    let tokenPart: string | undefined = segments[1] || undefined;

    if (queryPart) {
      const searchParams = new URLSearchParams(queryPart);
      const qToken = searchParams.get('token');
      if (qToken) {
        tokenPart = qToken.trim();
      }
    }

    return {
      roomCode: codePart,
      roomToken: tokenPart,
      ...(queryPart && new URLSearchParams(queryPart).get('host') ? {hostAddr:new URLSearchParams(queryPart).get('host')!} : {}),
      isUrl: true,
    };
  }

  return {
    roomCode: normalizeRoomCode(trimmed),
    isUrl: false,
  };
}

export function createConnectionCode(roomCode: string, roomToken: string, hostAddr: string): string {
  return CONNECTION_CODE_PREFIX + btoa(encodeURIComponent(JSON.stringify({roomCode, roomToken, hostAddr: hostAddr.trim()}))).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');
}
