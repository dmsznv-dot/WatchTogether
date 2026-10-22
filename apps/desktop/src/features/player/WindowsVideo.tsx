import React, { useEffect, useRef, useState } from 'react';
import { invokeCommand } from '../../services/ipc';
export const usesWindowsCanvas = () => /Windows/i.test(navigator.userAgent);

export function WindowsVideo() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let stopped = false;
    let timer: number | undefined;
    const draw = async () => {
      const target = canvas.current;
      if (!target || stopped) return;
      // One frame in flight; cap CPU/IPC usage on large monitors.
      const scale = Math.min(1, 1280 / Math.max(1, target.clientWidth), 720 / Math.max(1, target.clientHeight));
      const width = Math.max(16, Math.floor(target.clientWidth * scale / 16) * 16);
      const height = Math.max(16, Math.round(target.clientHeight * scale));
      try {
        const data = await invokeCommand<ArrayBuffer>('cmd_player_frame', { width, height });
        if (stopped) return;
        if (target.width !== width || target.height !== height) { target.width = width; target.height = height; }
        target.getContext('2d', { alpha: false })?.putImageData(new ImageData(new Uint8ClampedArray(data), width, height), 0, 0);
        timer = window.setTimeout(draw, 33);
      } catch (err) { if (!stopped) setError(String(err)); }
    };
    void draw();
    return () => { stopped = true; window.clearTimeout(timer); };
  }, []);
  return <><canvas ref={canvas} className="absolute inset-0 w-full h-full bg-black" aria-label="Video" />{error && <div role="alert" className="absolute p-4 text-rose-300 bg-black">Video output: {error}</div>}</>;
}
