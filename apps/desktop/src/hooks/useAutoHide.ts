import { useState, useEffect, useRef, useCallback } from 'react';

export interface UseAutoHideOptions {
  isPlaying: boolean;
  isSuspended?: boolean;
  timeoutMs?: number;
}

export function useAutoHide({
  isPlaying,
  isSuspended = false,
  timeoutMs = 2500,
}: UseAutoHideOptions) {
  const [isVisible, setIsVisible] = useState(true);
  const [isHovered, setIsHovered] = useState(false);
  const timerRef = useRef<number | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      window.clearTimeout(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const resetTimer = useCallback(() => {
    clearTimer();
    setIsVisible(true);

    if (isPlaying && !isSuspended && !isHovered) {
      timerRef.current = window.setTimeout(() => {
        setIsVisible(false);
      }, timeoutMs);
    }
  }, [isPlaying, isSuspended, isHovered, timeoutMs, clearTimer]);

  // Keep visible whenever paused, suspended, or hovered
  useEffect(() => {
    if (!isPlaying || isSuspended || isHovered) {
      clearTimer();
      setIsVisible(true);
    } else {
      resetTimer();
    }
    return () => clearTimer();
  }, [isPlaying, isSuspended, isHovered, resetTimer, clearTimer]);

  // Global listeners for activity
  useEffect(() => {
    const handleActivity = () => {
      resetTimer();
    };

    window.addEventListener('mousemove', handleActivity);
    window.addEventListener('mousedown', handleActivity);
    window.addEventListener('touchstart', handleActivity);
    window.addEventListener('keydown', handleActivity);

    return () => {
      window.removeEventListener('mousemove', handleActivity);
      window.removeEventListener('mousedown', handleActivity);
      window.removeEventListener('touchstart', handleActivity);
      window.removeEventListener('keydown', handleActivity);
    };
  }, [resetTimer]);

  const onMouseEnter = useCallback(() => {
    setIsHovered(true);
  }, []);

  const onMouseLeave = useCallback(() => {
    setIsHovered(false);
  }, []);

  return {
    isVisible,
    showControls: resetTimer,
    isHovered,
    bindHover: {
      onMouseEnter,
      onMouseLeave,
    },
  };
}
