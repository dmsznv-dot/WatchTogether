import { describe, it, expect } from 'vitest';
import { frameSize } from '../features/player/frameDrawer';

describe('Windows video frame size', () => {
  it('renders full 1080p for a 1080p window instead of the old 720p cap', () => {
    expect(frameSize(1920, 1080, 1)).toEqual({ width: 1920, height: 1080 });
  });
  it('uses device pixels on scaled displays but never exceeds 1080p', () => {
    expect(frameSize(1280, 720, 1.5)).toEqual({ width: 1920, height: 1080 });
    expect(frameSize(2560, 1440, 2)).toEqual({ width: 1920, height: 1080 });
  });
  it('keeps the aspect ratio of ultrawide windows inside the cap', () => {
    const { width, height } = frameSize(3440, 1440, 1);
    expect(width).toBeLessThanOrEqual(1920);
    expect(height).toBeLessThanOrEqual(1080);
    expect(width % 16).toBe(0);
    expect(Math.abs(width / height - 3440 / 1440)).toBeLessThan(0.02);
  });
  it('never asks for an empty frame', () => {
    expect(frameSize(0, 0, 1)).toEqual({ width: 16, height: 16 });
  });
});
