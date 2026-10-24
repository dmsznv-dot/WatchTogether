import React, { useEffect, useRef, useState } from 'react';
import { invokeCommand } from '../../services/ipc';
import { create2dDrawer, createWebGlDrawer, FrameDrawer, frameSize } from './frameDrawer';
export const usesWindowsCanvas = () => /Windows/i.test(navigator.userAgent);

interface SharedBufferEvent extends Event {
  additionalData?: { wtFrameBuffer?: number };
  getBuffer(): ArrayBuffer;
}
interface WebView2Bridge {
  addEventListener(type: 'sharedbufferreceived', listener: (event: SharedBufferEvent) => void): void;
  removeEventListener(type: 'sharedbufferreceived', listener: (event: SharedBufferEvent) => void): void;
}

/**
 * Windows video: libmpv renders on the CPU, frames are drawn here at display rate.
 * Preferred path: mpv writes into WebView2 shared memory and only a tiny request
 * crosses IPC; WebGL uploads the frame. Fallback: the frame bytes travel through IPC.
 */
export function WindowsVideo() {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const target = canvas.current;
    if (!target) return;
    let stopped = false;
    let frame = 0;
    let busy = false;
    let last = { width: 0, height: 0 };
    let next = 0;
    const shared: ArrayBuffer[] = [];
    const webview = (window as unknown as { chrome?: { webview?: WebView2Bridge } }).chrome?.webview;
    const onShared = (event: SharedBufferEvent) => {
      const index = event.additionalData?.wtFrameBuffer;
      if (typeof index === 'number') shared[index] = event.getBuffer();
    };
    webview?.addEventListener('sharedbufferreceived', onShared);

    const gl = createWebGlDrawer(target);
    let drawer: FrameDrawer | null = gl;
    let useShared = false;

    const step = async () => {
      const { width, height } = frameSize(target.clientWidth, target.clientHeight, window.devicePixelRatio || 1);
      const force = width !== last.width || height !== last.height;
      if (useShared) {
        const index = next;
        const fresh = await invokeCommand<boolean>('cmd_player_frame_shared', { index, width, height, force });
        if (stopped || !fresh) return;
        drawer!.draw(new Uint8Array(shared[index]), width, height);
        next = (index + 1) % shared.length;
      } else {
        const data = await invokeCommand<ArrayBuffer>('cmd_player_frame', { width, height, force });
        if (stopped || data.byteLength === 0) return;
        drawer!.draw(new Uint8Array(data), width, height);
      }
      last = { width, height };
    };
    // One request in flight; requestAnimationFrame paces requests to the display.
    const tick = () => {
      if (stopped) return;
      frame = requestAnimationFrame(tick);
      if (busy) return;
      busy = true;
      step()
        .catch((err) => { if (!stopped) setError(String(err)); stopped = true; })
        .finally(() => { busy = false; });
    };

    const start = async () => {
      if (gl && webview) {
        try {
          const count = await invokeCommand<number>('cmd_video_share_frames');
          const deadline = performance.now() + 2000;
          while (!stopped && shared.filter(Boolean).length < count && performance.now() < deadline) {
            await new Promise((resolve) => setTimeout(resolve, 10));
          }
          useShared = shared.filter(Boolean).length >= count && count > 0;
        } catch (err) {
          console.warn('Shared video buffers unavailable, copying frames through IPC:', err);
        }
      }
      // The IPC path returns opaque RGBA, which the 2D canvas can draw too.
      drawer = drawer ?? create2dDrawer(target);
      if (!drawer) { setError('No canvas context for video'); return; }
      if (!stopped) frame = requestAnimationFrame(tick);
    };
    void start();
    return () => {
      stopped = true;
      cancelAnimationFrame(frame);
      webview?.removeEventListener('sharedbufferreceived', onShared);
    };
  }, []);
  return <><canvas ref={canvas} className="absolute inset-0 w-full h-full bg-black" aria-label="Video" />{error && <div role="alert" className="absolute p-4 text-rose-300 bg-black">Video output: {error}</div>}</>;
}
