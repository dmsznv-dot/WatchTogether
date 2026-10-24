import { describe, it, expect } from 'vitest';
import { mediaFileKind } from '../utils/mediaFiles';

describe('mediaFileKind', () => {
  it('tells video, subtitles and audio apart, including .mkv vs .mka', () => {
    expect(mediaFileKind('/movies/Film.MKV')).toBe('video');
    expect(mediaFileKind('C:\\dubs\\Film.rus.mka')).toBe('audio');
    expect(mediaFileKind('dub.mp3')).toBe('audio');
    expect(mediaFileKind('dub.ac3')).toBe('audio');
    expect(mediaFileKind('subs.ass')).toBe('subtitle');
  });
  it('ignores files the player cannot use', () => {
    expect(mediaFileKind('notes.txt')).toBeNull();
    expect(mediaFileKind('no-extension')).toBeNull();
  });
});
