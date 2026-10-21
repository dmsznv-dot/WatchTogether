import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useSettings } from '../hooks/useSettings';
import { storage } from '../services/storage';

describe('useSettings Hook', () => {
  beforeEach(() => {
    storage.clear();
  });

  it('provides sensible defaults when storage is empty', () => {
    const { result } = renderHook(() => useSettings());
    expect(result.current.volume).toBe(1.0);
    expect(result.current.isMuted).toBe(false);
    expect(result.current.displayName).toBe('Guest');
  });

  it('updates and persists volume', () => {
    const { result } = renderHook(() => useSettings());

    act(() => {
      result.current.setVolume(0.65);
    });

    expect(result.current.volume).toBe(0.65);
    expect(storage.getItem('watchtogether:volume')).toBe('0.65');
  });

  it('updates and persists mute status', () => {
    const { result } = renderHook(() => useSettings());

    act(() => {
      result.current.setIsMuted(true);
    });

    expect(result.current.isMuted).toBe(true);
    expect(storage.getItem('watchtogether:muted')).toBe('true');
  });

  it('updates and persists display name', () => {
    const { result } = renderHook(() => useSettings());

    act(() => {
      result.current.setDisplayName('Alice');
    });

    expect(result.current.displayName).toBe('Alice');
    expect(storage.getItem('watchtogether:display_name')).toBe('Alice');
  });

  it('updates and persists preferred language preferences', () => {
    const { result } = renderHook(() => useSettings());

    act(() => {
      result.current.setPreferredAudioLang('jpn');
      result.current.setPreferredSubLang('eng');
    });

    expect(result.current.preferredAudioLang).toBe('jpn');
    expect(result.current.preferredSubLang).toBe('eng');
    expect(storage.getItem('watchtogether:audio_lang')).toBe('jpn');
    expect(storage.getItem('watchtogether:sub_lang')).toBe('eng');
  });
});
