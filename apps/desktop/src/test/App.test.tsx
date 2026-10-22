import React from 'react';
import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen, fireEvent, act } from '@testing-library/react';
import App from '../App';
import { mockBackend } from '../services/mockBackend';

describe('App Integration', () => {
  beforeEach(() => {
    mockBackend.reset();
  });

  it('renders application with HUD controls and empty video hero', async () => {
    await act(async () => { render(<App />); });

    expect(screen.getByTestId('video-canvas-container')).toBeInTheDocument();
    expect(screen.getByTestId('header-hud')).toBeInTheDocument();
    expect(screen.getByTestId('controls-hud')).toBeInTheDocument();
    expect(screen.getByText('No Video Loaded')).toBeInTheDocument();
  });

  it('loads video file when Open Video File is clicked', async () => {
    render(<App />);

    const openBtn = screen.getByText('Open Video File');
    await act(async () => {
      fireEvent.click(openBtn);
    });

    expect(screen.getByTestId('media-surface')).toBeInTheDocument();
    expect(screen.getByTestId('video-title')).toHaveTextContent('sample_movie.mp4');
  });

  it('opens and closes settings modal', async () => {
    render(<App />);

    const settingsBtn = screen.getByTestId('settings-btn');
    await act(async () => {
      fireEvent.click(settingsBtn);
    });

    expect(screen.getByText('Settings & Shortcuts')).toBeInTheDocument();

    const doneBtn = screen.getByText('Done');
    await act(async () => {
      fireEvent.click(doneBtn);
    });

    expect(screen.queryByText('Settings & Shortcuts')).not.toBeInTheDocument();
  });
});
