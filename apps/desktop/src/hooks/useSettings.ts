import { useState, useCallback } from 'react';
import { storage } from '../services/storage';

const KEYS = {
  VOLUME: 'watchtogether:volume',
  MUTED: 'watchtogether:muted',
  AUDIO_LANG: 'watchtogether:audio_lang',
  SUB_LANG: 'watchtogether:sub_lang',
  SUB_ENABLED: 'watchtogether:sub_enabled',
  DISPLAY_NAME: 'watchtogether:display_name',
  MINI_PLAYER: 'watchtogether:mini_player',
};

export function useSettings() {
  const [volume, setVolumeState] = useState<number>(() => {
    const val = storage.getItem(KEYS.VOLUME);
    return val !== null ? parseFloat(val) : 1.0;
  });

  const [isMuted, setIsMutedState] = useState<boolean>(() => {
    return storage.getItem(KEYS.MUTED) === 'true';
  });

  const [preferredAudioLang, setAudioLangState] = useState<string | undefined>(() => {
    return storage.getItem(KEYS.AUDIO_LANG) || undefined;
  });

  const [preferredSubLang, setSubLangState] = useState<string | undefined>(() => {
    return storage.getItem(KEYS.SUB_LANG) || undefined;
  });

  const [subtitlesEnabled, setSubtitlesEnabledState] = useState<boolean>(() => {
    return storage.getItem(KEYS.SUB_ENABLED) === 'true';
  });

  const [displayName, setDisplayNameState] = useState<string>(() => {
    return storage.getItem(KEYS.DISPLAY_NAME) || 'Guest';
  });

  // On by default: switching to another app while a video plays shows the mini player.
  const [miniPlayerOnSwitch, setMiniPlayerState] = useState<boolean>(() => {
    return storage.getItem(KEYS.MINI_PLAYER) !== 'false';
  });

  const setMiniPlayerOnSwitch = useCallback((enabled: boolean) => {
    setMiniPlayerState(enabled);
    storage.setItem(KEYS.MINI_PLAYER, enabled.toString());
  }, []);

  const setVolume = useCallback((v: number) => {
    const clamped = Math.max(0, Math.min(1, v));
    setVolumeState(clamped);
    storage.setItem(KEYS.VOLUME, clamped.toString());
  }, []);

  const setIsMuted = useCallback((m: boolean) => {
    setIsMutedState(m);
    storage.setItem(KEYS.MUTED, m.toString());
  }, []);

  const setPreferredAudioLang = useCallback((lang: string | undefined) => {
    setAudioLangState(lang);
    if (lang) storage.setItem(KEYS.AUDIO_LANG, lang);
    else storage.removeItem(KEYS.AUDIO_LANG);
  }, []);

  const setPreferredSubLang = useCallback((lang: string | undefined) => {
    setSubLangState(lang);
    if (lang) storage.setItem(KEYS.SUB_LANG, lang);
    else storage.removeItem(KEYS.SUB_LANG);
  }, []);

  const setSubtitlesEnabled = useCallback((enabled: boolean) => {
    setSubtitlesEnabledState(enabled);
    storage.setItem(KEYS.SUB_ENABLED, enabled.toString());
  }, []);

  const setDisplayName = useCallback((name: string) => {
    const sanitized = name.trim() || 'Guest';
    setDisplayNameState(sanitized);
    storage.setItem(KEYS.DISPLAY_NAME, sanitized);
  }, []);

  return {
    volume,
    isMuted,
    preferredAudioLang,
    preferredSubLang,
    subtitlesEnabled,
    displayName,
    miniPlayerOnSwitch,
    setMiniPlayerOnSwitch,
    setVolume,
    setIsMuted,
    setPreferredAudioLang,
    setPreferredSubLang,
    setSubtitlesEnabled,
    setDisplayName,
  };
}
