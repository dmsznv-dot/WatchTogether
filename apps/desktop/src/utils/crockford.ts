export const CROCKFORD_CHARSET = '23456789ABCDEFGHJKLMNPQRSTUVWXYZ';

export function normalizeRoomCode(input: string): string {
  return input.trim().toUpperCase().replace(/[\s-]/g, '');
}

export function isValidRoomCode(code: string): boolean {
  const normalized = normalizeRoomCode(code);
  if (normalized.length !== 6) return false;
  return /^[2-9A-HJ-NP-Z]{6}$/.test(normalized);
}

export function getForbiddenCrockfordChars(input: string): string[] {
  const normalized = input.trim().toUpperCase();
  const forbidden: string[] = [];
  for (const char of normalized) {
    if ('01IO'.includes(char) && !forbidden.includes(char)) {
      forbidden.push(char);
    }
  }
  return forbidden;
}

export function generateMockRoomCode(): string {
  let result = '';
  for (let i = 0; i < 6; i++) {
    const idx = Math.floor(Math.random() * CROCKFORD_CHARSET.length);
    result += CROCKFORD_CHARSET[idx];
  }
  return result;
}

export function generateMockRoomToken(): string {
  const hexChars = '0123456789abcdef';
  let result = '';
  for (let i = 0; i < 64; i++) {
    result += hexChars[Math.floor(Math.random() * hexChars.length)];
  }
  return result;
}
