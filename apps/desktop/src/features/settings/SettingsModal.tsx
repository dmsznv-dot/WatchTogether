import React from 'react';
import { Keyboard, Sliders, Volume2, Globe } from 'lucide-react';
import { Modal } from '../../components/ui/Modal';
import { Button } from '../../components/ui/Button';

export interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
  displayName: string;
  preferredAudioLang?: string;
  preferredSubLang?: string;
  subtitlesEnabled: boolean;
  onDisplayNameChange: (name: string) => void;
  onAudioLangChange: (lang: string | undefined) => void;
  onSubLangChange: (lang: string | undefined) => void;
  onSubtitlesEnabledChange: (enabled: boolean) => void;
  miniPlayerOnSwitch?: boolean;
  onMiniPlayerOnSwitchChange?: (enabled: boolean) => void;
}

export function SettingsModal({
  isOpen,
  onClose,
  displayName,
  preferredAudioLang,
  preferredSubLang,
  subtitlesEnabled,
  onDisplayNameChange,
  onAudioLangChange,
  onSubLangChange,
  onSubtitlesEnabledChange,
  miniPlayerOnSwitch = true,
  onMiniPlayerOnSwitchChange,
}: SettingsModalProps) {
  const shortcuts = [
    { key: 'Space', desc: 'Play / Pause toggle' },
    { key: '← / →', desc: 'Seek ±5 seconds' },
    { key: 'Shift + ← / →', desc: 'Seek ±30 seconds' },
    { key: '↑ / ↓', desc: 'Local volume ±5%' },
    { key: 'F', desc: 'Toggle Fullscreen' },
    { key: 'M', desc: 'Toggle Local Mute' },
    { key: 'Cmd/Ctrl + O', desc: 'Open Video File' },
    { key: 'I', desc: 'Playback statistics' },
    { key: 'Esc', desc: 'Exit Fullscreen / Mini Player / Close Modal' },
  ];

  return (
    <Modal
      isOpen={isOpen}
      onClose={onClose}
      title="Settings & Shortcuts"
      description="Configure local preferences and view available keyboard shortcuts."
      maxWidth="max-w-lg"
    >
      <div className="flex flex-col gap-6 pt-2">
        {/* User Preferences */}
        <div className="flex flex-col gap-3">
          <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-indigo-400">
            <Sliders className="w-4 h-4" />
            <span>Preferences</span>
          </div>

          <div>
            <label className="block text-xs font-medium text-zinc-400 mb-1">
              Your Display Name
            </label>
            <input
              type="text"
              value={displayName}
              onChange={(e) => onDisplayNameChange(e.target.value)}
              placeholder="Guest"
              className="w-full px-3 py-1.5 rounded-xl bg-zinc-900 border border-zinc-700 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
              data-testid="settings-display-name-input"
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                Preferred Audio Lang
              </label>
              <input
                type="text"
                value={preferredAudioLang || ''}
                onChange={(e) => onAudioLangChange(e.target.value || undefined)}
                placeholder="e.g. eng, jpn"
                className="w-full px-3 py-1.5 rounded-xl bg-zinc-900 border border-zinc-700 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                data-testid="settings-audio-lang-input"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-zinc-400 mb-1">
                Preferred Subtitle Lang
              </label>
              <input
                type="text"
                value={preferredSubLang || ''}
                onChange={(e) => onSubLangChange(e.target.value || undefined)}
                placeholder="e.g. eng, spa"
                className="w-full px-3 py-1.5 rounded-xl bg-zinc-900 border border-zinc-700 text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:border-indigo-500"
                data-testid="settings-sub-lang-input"
              />
            </div>
          </div>
        </div>

        {onMiniPlayerOnSwitchChange && (
          <label className="flex items-start gap-3 text-sm text-zinc-200 cursor-pointer">
            <input
              type="checkbox"
              checked={miniPlayerOnSwitch}
              onChange={(e) => onMiniPlayerOnSwitchChange(e.target.checked)}
              className="mt-0.5 accent-indigo-500"
              data-testid="settings-mini-player-toggle"
            />
            <span>
              Mini player when switching apps
              <span className="block text-xs text-zinc-400">
                While a video plays, Alt+Tab / Cmd+Tab turns the window into a small player on top of other apps. Double-click or press Esc to expand it.
              </span>
            </span>
          </label>
        )}

        {/* Keyboard Shortcuts Table */}
        <div className="flex flex-col gap-2">
          <div className="flex items-center gap-2 text-xs font-semibold uppercase tracking-wider text-indigo-400">
            <Keyboard className="w-4 h-4" />
            <span>Keyboard Shortcuts</span>
          </div>

          <div className="rounded-xl border border-zinc-800 bg-zinc-900/60 overflow-hidden text-xs">
            {shortcuts.map((sc, idx) => (
              <div
                key={sc.key}
                className={`flex items-center justify-between px-3 py-2 ${
                  idx % 2 === 0 ? 'bg-zinc-900/40' : 'bg-transparent'
                }`}
              >
                <kbd className="px-2 py-0.5 rounded bg-zinc-800 border border-zinc-700 text-zinc-200 font-mono text-[11px] shadow-sm">
                  {sc.key}
                </kbd>
                <span className="text-zinc-400">{sc.desc}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="flex justify-end pt-2 border-t border-zinc-800">
          <Button variant="primary" size="sm" onClick={onClose}>
            Done
          </Button>
        </div>
      </div>
    </Modal>
  );
}
