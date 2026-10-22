import { normalizeRoomCode } from './crockford';

export const DEEP_LINK_SCHEME = 'watchtogether';

export function createShareableLink(roomCode: string, roomToken: string, hostAddr?: string): string {
  return `${DEEP_LINK_SCHEME}://join/${roomCode}?token=${encodeURIComponent(roomToken)}${hostAddr ? `&host=${encodeURIComponent(hostAddr.trim())}` : ''}`;
}

export interface ParsedJoinInput {
  roomCode: string;
  roomToken?: string;
  hostAddr?: string;
  isUrl: boolean;
}

export function parseJoinInput(raw: string): ParsedJoinInput {
  const trimmed = raw.trim();
  if (trimmed.startsWith('WT1-')) {
    try {
      const encoded = trimmed.slice(4).replace(/-/g, '+').replace(/_/g, '/');
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
  return 'WT1-' + btoa(encodeURIComponent(JSON.stringify({roomCode, roomToken, hostAddr: hostAddr.trim()}))).replace(/\+/g,'-').replace(/\//g,'_').replace(/=+$/,'');
}
