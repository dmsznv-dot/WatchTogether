import { normalizeRoomCode } from './crockford';

export const DEEP_LINK_SCHEME = 'watchtogether';

export function createShareableLink(roomCode: string, roomToken: string): string {
  return `${DEEP_LINK_SCHEME}://join/${roomCode}?token=${roomToken}`;
}

export interface ParsedJoinInput {
  roomCode: string;
  roomToken?: string;
  isUrl: boolean;
}

export function parseJoinInput(raw: string): ParsedJoinInput {
  const trimmed = raw.trim();
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
      isUrl: true,
    };
  }

  return {
    roomCode: normalizeRoomCode(trimmed),
    isUrl: false,
  };
}
