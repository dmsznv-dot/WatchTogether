import { describe, it, expect } from 'vitest';
import {
  normalizeRoomCode,
  isValidRoomCode,
  getForbiddenCrockfordChars,
  generateMockRoomCode,
} from '../utils/crockford';

describe('Crockford Base32 Room Code Utilities', () => {
  it('normalizes whitespace, dashes, and lowercase characters', () => {
    expect(normalizeRoomCode('  k7m-2p9  ')).toBe('K7M2P9');
    expect(normalizeRoomCode('234 567')).toBe('234567');
    expect(normalizeRoomCode('abc-def')).toBe('ABCDEF');
  });

  it('validates correct 6-character Crockford codes', () => {
    expect(isValidRoomCode('K7M2P9')).toBe(true);
    expect(isValidRoomCode('234567')).toBe(true);
    expect(isValidRoomCode('ABCDEF')).toBe(true);
  });

  it('rejects invalid lengths', () => {
    expect(isValidRoomCode('K7M2P')).toBe(false); // 5 chars
    expect(isValidRoomCode('K7M2P99')).toBe(false); // 7 chars
    expect(isValidRoomCode('')).toBe(false);
  });

  it('rejects forbidden Crockford characters (0, O, 1, I)', () => {
    expect(isValidRoomCode('K7M2P0')).toBe(false); // forbidden 0
    expect(isValidRoomCode('K7M2PO')).toBe(false); // forbidden O
    expect(isValidRoomCode('K7M2P1')).toBe(false); // forbidden 1
    expect(isValidRoomCode('K7M2PI')).toBe(false); // forbidden I
  });

  it('identifies forbidden characters in candidate string', () => {
    expect(getForbiddenCrockfordChars('k7m-0o1i')).toEqual(['0', 'O', '1', 'I']);
    expect(getForbiddenCrockfordChars('K7M2P9')).toEqual([]);
  });

  it('generates compliant random 6-character Crockford codes', () => {
    for (let i = 0; i < 20; i++) {
      const code = generateMockRoomCode();
      expect(code).toHaveLength(6);
      expect(isValidRoomCode(code)).toBe(true);
      expect(getForbiddenCrockfordChars(code)).toHaveLength(0);
    }
  });
});
