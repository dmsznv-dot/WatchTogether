import { describe, it, expect } from 'vitest';
import { createShareableLink, parseJoinInput } from '../utils/deepLink';

describe('Deep Link Utilities', () => {
  it('generates shareable deep links matching schema', () => {
    const link = createShareableLink('K7M2P9', '0123456789abcdef0123456789abcdef');
    expect(link).toBe(
      'watchtogether://join/K7M2P9?token=0123456789abcdef0123456789abcdef'
    );
  });

  it('parses raw 6-character room codes', () => {
    const parsed = parseJoinInput('k7m-2p9');
    expect(parsed).toEqual({
      roomCode: 'K7M2P9',
      isUrl: false,
    });
  });

  it('parses full deep link URLs with query parameter token', () => {
    const parsed = parseJoinInput(
      'watchtogether://join/K7M2P9?token=0123456789abcdef0123456789abcdef'
    );
    expect(parsed).toEqual({
      roomCode: 'K7M2P9',
      roomToken: '0123456789abcdef0123456789abcdef',
      isUrl: true,
    });
  });

  it('parses path-based deep link URLs', () => {
    const parsed = parseJoinInput(
      'watchtogether://join/K7M2P9/0123456789abcdef0123456789abcdef'
    );
    expect(parsed.roomCode).toBe('K7M2P9');
    expect(parsed.roomToken).toBe('0123456789abcdef0123456789abcdef');
    expect(parsed.isUrl).toBe(true);
  });
});
