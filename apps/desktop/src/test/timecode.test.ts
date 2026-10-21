import { describe, it, expect } from 'vitest';
import { formatTimecode, formatDurationPair } from '../utils/timecode';

describe('Timecode Formatting Utilities', () => {
  it('formats seconds into MM:SS format when under 1 hour', () => {
    expect(formatTimecode(0)).toBe('00:00');
    expect(formatTimecode(45)).toBe('00:45');
    expect(formatTimecode(125)).toBe('02:05');
    expect(formatTimecode(3599)).toBe('59:59');
  });

  it('formats seconds into HH:MM:SS format when >= 1 hour', () => {
    expect(formatTimecode(3600)).toBe('01:00:00');
    expect(formatTimecode(3665)).toBe('01:01:05');
    expect(formatTimecode(7325)).toBe('02:02:05');
  });

  it('honors forceHours parameter', () => {
    expect(formatTimecode(45, true)).toBe('00:00:45');
    expect(formatTimecode(0, true)).toBe('00:00:00');
  });

  it('handles negative numbers and NaN gracefully', () => {
    expect(formatTimecode(-10)).toBe('00:00');
    expect(formatTimecode(NaN)).toBe('00:00');
    expect(formatTimecode(-10, true)).toBe('00:00:00');
  });

  it('formats duration pairs symmetrically', () => {
    expect(formatDurationPair(45, 120)).toEqual({
      current: '00:45',
      total: '02:00',
      formatted: '00:45 / 02:00',
    });

    expect(formatDurationPair(120, 3605)).toEqual({
      current: '00:02:00',
      total: '01:00:05',
      formatted: '00:02:00 / 01:00:05',
    });
  });
});
