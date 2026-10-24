import React from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { MiniPlayerOverlay } from '../features/player/MiniPlayerOverlay';

const setup = (isPlaying = true) => {
  const props = { isPlaying, onTogglePlayPause: vi.fn(), onSeekRelative: vi.fn(), onExpand: vi.fn() };
  render(<MiniPlayerOverlay {...props} />);
  return props;
};

describe('MiniPlayerOverlay', () => {
  it('shows controls only while the pointer is over the mini player', () => {
    setup();
    const overlay = screen.getByTestId('mini-player-overlay');
    const controls = screen.getByTestId('mini-play-btn').closest('div.transition-opacity')!;
    expect(controls.className).toContain('opacity-0');
    fireEvent.mouseEnter(overlay);
    expect(controls.className).toContain('opacity-100');
    fireEvent.mouseLeave(overlay);
    expect(controls.className).toContain('opacity-0');
  });

  it('plays, pauses and seeks back and forward from the hover controls', () => {
    const props = setup(true);
    expect(screen.getByTestId('mini-play-btn')).toHaveAttribute('aria-label', 'Pause');
    fireEvent.click(screen.getByTestId('mini-play-btn'));
    fireEvent.click(screen.getByTestId('mini-back-btn'));
    fireEvent.click(screen.getByTestId('mini-forward-btn'));
    expect(props.onTogglePlayPause).toHaveBeenCalledOnce();
    expect(props.onSeekRelative.mock.calls).toEqual([[-10], [10]]);
    expect(props.onExpand).not.toHaveBeenCalled();
  });

  it('expands on double-click and from the expand button', () => {
    const props = setup();
    fireEvent.mouseDown(screen.getByTestId('mini-player-overlay'), { button: 0, detail: 2 });
    fireEvent.click(screen.getByTestId('mini-expand-btn'));
    expect(props.onExpand).toHaveBeenCalledTimes(2);
  });
});
