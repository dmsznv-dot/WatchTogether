import React from 'react';
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Scrubber } from '../features/player/Scrubber';

describe('Scrubber Component', () => {
  it('renders played and buffered progress bars', () => {
    render(
      <Scrubber
        positionSec={30}
        durationSec={120}
        bufferedSec={90}
        isScrubbing={false}
        onStartScrub={vi.fn()}
        onUpdateScrub={vi.fn()}
        onCommitScrub={vi.fn()}
      />
    );

    const played = screen.getByTestId('scrubber-played');
    const buffered = screen.getByTestId('scrubber-buffered');

    expect(played).toHaveStyle({ width: '25%' });
    expect(buffered).toHaveStyle({ width: '75%' });
  });

  it('triggers onStartScrub on mouse down', () => {
    const onStartScrub = vi.fn();
    render(
      <Scrubber
        positionSec={0}
        durationSec={100}
        isScrubbing={false}
        onStartScrub={onStartScrub}
        onUpdateScrub={vi.fn()}
        onCommitScrub={vi.fn()}
      />
    );

    const scrubber = screen.getByTestId('timeline-scrubber');
    fireEvent.mouseDown(scrubber, { clientX: 50 });

    expect(onStartScrub).toHaveBeenCalled();
  });

  it('does not trigger scrub when disabled', () => {
    const onStartScrub = vi.fn();
    render(
      <Scrubber
        positionSec={0}
        durationSec={100}
        isScrubbing={false}
        disabled={true}
        onStartScrub={onStartScrub}
        onUpdateScrub={vi.fn()}
        onCommitScrub={vi.fn()}
      />
    );

    const scrubber = screen.getByTestId('timeline-scrubber');
    fireEvent.mouseDown(scrubber, { clientX: 50 });

    expect(onStartScrub).not.toHaveBeenCalled();
  });
});
