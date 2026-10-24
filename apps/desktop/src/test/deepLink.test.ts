import { describe, it, expect } from 'vitest';
import { createShareableLink, createConnectionCode, parseJoinInput } from '../utils/deepLink';

describe('Deep Link Utilities', () => {
  it('round-trips a complete connection code without a directory server', () => {
    const code = createConnectionCode('K7M2P9', 'a'.repeat(64), 'eyJpIjoiaG9zdCJ9');
    expect(code.startsWith('WT2-')).toBe(true);
    expect(parseJoinInput(code)).toEqual({roomCode:'K7M2P9',roomToken:'a'.repeat(64),hostAddr:'eyJpIjoiaG9zdCJ9',isUrl:true});
    // chat apps and terminals may wrap a long code across lines
    expect(parseJoinInput(code.slice(0, 30) + '\n' + code.slice(30)).roomCode).toBe('K7M2P9');
    expect(parseJoinInput('WT2-broken').roomCode).toBe('');
  });
  it('flags IP-based WT1 invitations from older versions', () => {
    expect(parseJoinInput('WT1-eyJyb29tQ29kZSI6IksifQ')).toEqual({roomCode:'',isUrl:true,isLegacy:true});
  });
  it('preserves host endpoint and credentials in an invitation', () => {
    const link = createShareableLink('K7M2P9', 'a'.repeat(64), '203.0.113.4:8999');
    expect(parseJoinInput(link)).toMatchObject({roomCode:'K7M2P9',roomToken:'a'.repeat(64),hostAddr:'203.0.113.4:8999'});
  });
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
