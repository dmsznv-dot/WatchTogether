import React from 'react';
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, act, renderHook } from '@testing-library/react';
import { UpdateBanner } from '../features/update/UpdateBanner';

const downloadAndInstall = vi.fn();
const check = vi.fn();
const relaunch = vi.fn();
vi.mock('@tauri-apps/plugin-updater', () => ({ check: () => check() }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: () => relaunch() }));
vi.mock('../services/ipc', async (original) => ({ ...(await original<object>()), hasTauri: () => true }));

describe('Update banner', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    check.mockReset();
    downloadAndInstall.mockReset();
    relaunch.mockReset();
  });
  afterEach(() => vi.useRealTimers());

  it('stays hidden until an update is found', () => {
    render(<UpdateBanner status={{ state: 'idle' }} onInstall={vi.fn()} onDismiss={vi.fn()} />);
    expect(screen.queryByTestId('update-banner')).not.toBeInTheDocument();
  });

  it('finds an update, downloads it with progress and restarts', async () => {
    const { useUpdater } = await import('../hooks/useUpdater');
    downloadAndInstall.mockImplementation(async (onEvent: (e: unknown) => void) => {
      onEvent({ event: 'Started', data: { contentLength: 200 } });
      onEvent({ event: 'Progress', data: { chunkLength: 100 } });
    });
    check.mockResolvedValue({ version: '0.2.7', downloadAndInstall });
    const { result } = renderHook(() => useUpdater());
    await act(async () => { await vi.advanceTimersByTimeAsync(6_000); });
    expect(result.current.status).toEqual({ state: 'available', version: '0.2.7' });

    render(<UpdateBanner status={result.current.status} onInstall={result.current.install} onDismiss={vi.fn()} />);
    expect(screen.getByTestId('update-banner')).toHaveTextContent('0.2.7');
    await act(async () => { fireEvent.click(screen.getByTestId('update-install-btn')); });
    expect(downloadAndInstall).toHaveBeenCalledOnce();
    expect(relaunch).toHaveBeenCalledOnce();
  });

  it('shows nothing when offline or already up to date', async () => {
    const { useUpdater } = await import('../hooks/useUpdater');
    check.mockRejectedValueOnce(new Error('network unreachable'));
    const { result } = renderHook(() => useUpdater());
    await act(async () => { await vi.advanceTimersByTimeAsync(6_000); });
    expect(result.current.status).toEqual({ state: 'idle' });
    check.mockResolvedValueOnce(null);
    await act(async () => { await result.current.checkNow(); });
    expect(result.current.status).toEqual({ state: 'idle' });
  });

  it('reports a failed download and lets the user retry', async () => {
    const onInstall = vi.fn();
    render(<UpdateBanner status={{ state: 'error', message: 'signature mismatch' }} onInstall={onInstall} onDismiss={vi.fn()} />);
    expect(screen.getByTestId('update-banner')).toHaveTextContent('signature mismatch');
    fireEvent.click(screen.getByText('Retry'));
    expect(onInstall).toHaveBeenCalledOnce();
  });
});
