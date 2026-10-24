import React, { useEffect, useRef, useState } from 'react';
import { invokeCommand } from '../../services/ipc';
import { create2dDrawer, createWebGlDrawer, createWebGpuHdrDrawer, FrameDrawer, frameSize } from './frameDrawer';
export const usesWindowsCanvas = () => /Windows/i.test(navigator.userAgent);

interface SharedBufferEvent extends Event {
  additionalData?: { wtFrameBuffer?: number };
  getBuffer(): ArrayBuffer;
}
interface WebView2Bridge {
  addEventListener(type: 'sharedbufferreceived', listener: (event: SharedBufferEvent) => void): void;
  removeEventListener(type: 'sharedbufferreceived', listener: (event: SharedBufferEvent) => void): void;
}
export interface DisplayHdr {
  /** Windows HDR is on for the window's monitor and the GPU renderer can output HDR. */
  hdr: boolean;
  peakNits: number;
}
interface FrameInfo {
  fresh: boolean;
  format: 'rgba8' | 'rgba16f';
}

/** Peak assumed when the monitor does not report one (typical HDR monitors: 400–1000). */
const DEFAULT_PEAK_NITS = 1000;

/**
 * HDR only when the monitor is in HDR mode and the page can show HDR; otherwise SDR,
 * where HDR video is tone-mapped. Returns the peak to request, or undefined for SDR.
 */
export function hdrPeakFor(display: DisplayHdr | null, hdrOutputReady: boolean): number | undefined {
  if (!display?.hdr || !hdrOutputReady) return undefined;
  return display.peakNits > 0 ? display.peakNits : DEFAULT_PEAK_NITS;
}

/**
 * Windows video: libmpv renders (on the GPU when available), frames are drawn here at
 * display rate. Preferred path: mpv writes into WebView2 shared memory and only a tiny
 * request crosses IPC; WebGL shows SDR frames, an HDR WebGPU canvas HDR frames.
 * Fallback: the frame bytes travel through IPC (SDR only).
 */
export function WindowsVideo() {
  const sdrCanvas = useRef<HTMLCanvasElement>(null);
  const hdrCanvas = useRef<HTMLCanvasElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [showingHdr, setShowingHdr] = useState(false);
  useEffect(() => {
    const sdrTarget = sdrCanvas.current;
    const hdrTarget = hdrCanvas.current;
    if (!sdrTarget || !hdrTarget) return;
    let stopped = false;
    let frame = 0;
    let busy = false;
    let last = { width: 0, height: 0, peak: undefined as number | undefined };
    let next = 0;
    const shared: ArrayBuffer[] = [];
    const webview = (window as unknown as { chrome?: { webview?: WebView2Bridge } }).chrome?.webview;
    const onShared = (event: SharedBufferEvent) => {
      const index = event.additionalData?.wtFrameBuffer;
      if (typeof index === 'number') shared[index] = event.getBuffer();
    };
    webview?.addEventListener('sharedbufferreceived', onShared);

    const gl = createWebGlDrawer(sdrTarget);
    let sdrDrawer: FrameDrawer | null = gl;
    let hdrDrawer: FrameDrawer | null = null;
    let hdrDrawerTried = false;
    let display: DisplayHdr | null = null;
    let useShared = false;

    const refreshDisplay = async () => {
      try {
        display = await invokeCommand<DisplayHdr>('cmd_video_display_hdr');
        if (display.hdr && !hdrDrawerTried && useShared) {
          hdrDrawerTried = true;
          hdrDrawer = await createWebGpuHdrDrawer(hdrTarget);
        }
      } catch (err) {
        console.warn('Cannot read the monitor HDR state:', err);
        display = null;
      }
    };
    const hdrQuery = window.matchMedia?.('(dynamic-range: high)');
    const onDisplayChange = () => { void refreshDisplay(); };
    hdrQuery?.addEventListener?.('change', onDisplayChange);
    let unlistenMoved: (() => void) | undefined;
    import('@tauri-apps/api/webviewWindow')
      .then(({ getCurrentWebviewWindow }) => getCurrentWebviewWindow().onMoved(onDisplayChange))
      .then((fn) => { if (stopped) fn(); else unlistenMoved = fn; })
      .catch(() => {});

    const step = async () => {
      const { width, height } = frameSize(sdrTarget.clientWidth, sdrTarget.clientHeight, window.devicePixelRatio || 1);
      const peak = useShared ? hdrPeakFor(display, !!hdrDrawer) : undefined;
      const force = width !== last.width || height !== last.height || peak !== last.peak;
      if (useShared) {
        const index = next;
        const info = await invokeCommand<FrameInfo>('cmd_player_frame_shared', {
          index, width, height, force, hdrPeakNits: peak,
        });
        if (stopped || !info.fresh) return;
        const pixels = new Uint8Array(shared[index]);
        if (info.format === 'rgba16f' && hdrDrawer) {
          hdrDrawer.draw(pixels, width, height);
          setShowingHdr(true);
        } else {
          sdrDrawer!.draw(pixels, width, height);
          setShowingHdr(false);
        }
        next = (index + 1) % shared.length;
      } else {
        const data = await invokeCommand<ArrayBuffer>('cmd_player_frame', { width, height, force });
        if (stopped || data.byteLength === 0) return;
        sdrDrawer!.draw(new Uint8Array(data), width, height);
      }
      last = { width, height, peak };
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
      if (useShared) await refreshDisplay();
      // The IPC path returns opaque RGBA, which the 2D canvas can draw too.
      sdrDrawer = sdrDrawer ?? create2dDrawer(sdrTarget);
      if (!sdrDrawer) { setError('No canvas context for video'); return; }
      if (!stopped) frame = requestAnimationFrame(tick);
    };
    void start();
    return () => {
      stopped = true;
      cancelAnimationFrame(frame);
      webview?.removeEventListener('sharedbufferreceived', onShared);
      hdrQuery?.removeEventListener?.('change', onDisplayChange);
      unlistenMoved?.();
    };
  }, []);
  return (
    <>
      <canvas ref={sdrCanvas} className={`absolute inset-0 w-full h-full bg-black ${showingHdr ? 'invisible' : ''}`} aria-label="Video" />
      <canvas ref={hdrCanvas} className={`absolute inset-0 w-full h-full bg-black ${showingHdr ? '' : 'invisible'}`} aria-label="HDR video" data-testid="hdr-canvas" />
      {error && <div role="alert" className="absolute p-4 text-rose-300 bg-black">Video output: {error}</div>}
    </>
  );
}
