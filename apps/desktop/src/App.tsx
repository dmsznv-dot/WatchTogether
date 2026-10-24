import React, { useState, useCallback, useEffect } from 'react';
import { VideoCanvas } from './features/player/VideoCanvas';
import { HeaderHUD } from './features/player/HeaderHUD';
import { ControlsHUD } from './features/player/ControlsHUD';
import { HostAuthorityToast } from './features/rooms/HostAuthorityToast';
import { CreateRoomModal } from './features/rooms/CreateRoomModal';
import { JoinRoomModal } from './features/rooms/JoinRoomModal';
import { SettingsModal } from './features/settings/SettingsModal';
import {
  usePlayer,
  useTracks,
  useRoom,
  useSettings,
  useAutoHide,
  useKeyboardShortcuts,
} from './hooks';
import { hasTauri, windowApi } from './services/ipc';

export function App() {
  const native = hasTauri();
  useEffect(() => {
    document.documentElement.toggleAttribute('data-native', native);
    return () => { document.documentElement.removeAttribute('data-native'); };
  }, [native]);
  const settings = useSettings();

  const player = usePlayer({
    initialVolume: settings.volume,
    initialMuted: settings.isMuted,
  });

  const tracks = useTracks();
  const room = useRoom();

  const [isFullscreen, setIsFullscreen] = useState<boolean>(false);
  const [isCreateModalOpen, setIsCreateModalOpen] = useState<boolean>(false);
  const [isJoinModalOpen, setIsJoinModalOpen] = useState<boolean>(false);
  const [isSettingsModalOpen, setIsSettingsModalOpen] = useState<boolean>(false);
  const [invitation, setInvitation] = useState<string | undefined>();
  useEffect(() => {
    if (!native) return;
    let disposed = false;
    let stop: (() => void) | undefined;
    const receive = (urls: string[]) => {
      const url = urls.find(value => value.startsWith('watchtogether://join/'));
      if (!disposed && url) { setInvitation(url); setIsJoinModalOpen(true); }
    };
    import('@tauri-apps/plugin-deep-link').then(async ({ getCurrent, onOpenUrl }) => {
      const unlisten = await onOpenUrl(receive);
      if (disposed) { unlisten(); return; }
      stop = unlisten;
      receive((await getCurrent()) || []);
    }).catch(console.error);
    return () => { disposed = true; stop?.(); };
  }, [native]);

  useEffect(() => {
    if (!native) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    import('@tauri-apps/api/webviewWindow').then(async ({ getCurrentWebviewWindow }) => {
      const window = getCurrentWebviewWindow();
      const sync = async () => {
        const fullscreen = await window.isFullscreen();
        if (!disposed) setIsFullscreen(fullscreen);
      };
      const stop = await window.onResized(() => { void sync().catch(console.error); });
      if (disposed) { stop(); return; }
      unlisten = stop;
      await sync();
    }).catch(console.error);
    return () => { disposed = true; unlisten?.(); };
  }, [native]);

  const isModalOpen =
    isCreateModalOpen || isJoinModalOpen || isSettingsModalOpen;

  // Auto-hide controls & cursor after 2.5s of inactivity during active playback
  const { isVisible: isHudVisible, bindHover } = useAutoHide({
    isPlaying: player.isPlaying,
    isSuspended: isModalOpen || player.isScrubbing,
    timeoutMs: 2500,
  });

  const handleToggleFullscreen = useCallback(async () => {
    const next = await windowApi.toggleFullscreen();
    setIsFullscreen(next);
  }, []);

  const handleOpenFilePicker = useCallback(async () => {
    const selected = await windowApi.openFileDialog();
    if (selected) {
      await player.openFile(selected);
    }
  }, [player]);

  const handleUnauthorizedAction = useCallback(() => {
    room.notifyHostAuthority();
  }, [room]);

  // Global Keyboard Shortcuts
  useKeyboardShortcuts({
    disabled: isModalOpen,
    onTogglePlayPause: () => {
      if (!room.canControlPlayback) {
        handleUnauthorizedAction();
        return;
      }
      player.togglePlayPause();
    },
    onSeekRelative: (delta) => {
      if (!room.canControlPlayback) {
        handleUnauthorizedAction();
        return;
      }
      player.seek(player.positionSec + delta);
    },
    onToggleFullscreen: handleToggleFullscreen,
    onToggleMute: () => {
      player.toggleMute();
      settings.setIsMuted(!player.isMuted);
    },
    onAdjustVolume: (delta) => {
      const nextVol = Math.max(0, Math.min(1, player.volume + delta));
      player.setVolume(nextVol);
      settings.setVolume(nextVol);
    },
    onOpenFile: handleOpenFilePicker,
    onEscape: () => {
      if (isFullscreen) {
        handleToggleFullscreen();
      }
    },
  });

  return (
    <div className={`relative w-screen h-screen ${native ? "bg-transparent" : "bg-[#0B0D11]"} text-zinc-100 overflow-hidden font-sans select-none`}>
      {/* Central Video Viewport (Canvas / Surface) */}
      <VideoCanvas
        currentFile={player.currentFile}
        isPlaying={player.isPlaying}
        aspectRatio={16 / 9}
        hideCursor={player.isPlaying && !isHudVisible}
        onOpenFile={player.openFile}
        onLoadSubtitle={tracks.loadExternalSubtitle}
        onTogglePlayPause={() => {
          if (!room.canControlPlayback) {
            handleUnauthorizedAction();
            return;
          }
          player.togglePlayPause();
        }}
        onToggleFullscreen={handleToggleFullscreen}
        onSelectFileClick={handleOpenFilePicker}
      />

      {/* Top Header HUD Overlay */}
      <HeaderHUD
        isVisible={isHudVisible}
        currentFile={player.currentFile}
        sessionType={room.sessionType}
        roomCode={room.roomCode}
        snapshot={room.snapshot}
        permissionMode={room.permissionMode}
        isHost={room.isHost}
        onOpenFileDialog={handleOpenFilePicker}
        onOpenCreateModal={() => setIsCreateModalOpen(true)}
        onOpenJoinModal={() => setIsJoinModalOpen(true)}
        onOpenSettingsModal={() => setIsSettingsModalOpen(true)}
        onSetPermissions={room.setPermissions}
        onKickParticipant={room.kickParticipant}
        onLeaveRoom={room.leaveRoom}
        bindHover={bindHover}
      />

      {/* Floating Bottom Media Controls HUD */}
      <ControlsHUD
        isVisible={isHudVisible}
        isPlaying={player.isPlaying}
        positionSec={player.positionSec}
        durationSec={player.durationSec}
        speed={player.speed}
        volume={player.volume}
        isMuted={player.isMuted}
        isFullscreen={isFullscreen}
        isScrubbing={player.isScrubbing}
        canControlPlayback={room.canControlPlayback}
        audioTracks={tracks.audioTracks}
        subtitleTracks={tracks.subtitleTracks}
        onTogglePlayPause={player.togglePlayPause}
        onSeekRelative={(delta) => player.seek(player.positionSec + delta)}
        onStartScrub={player.startScrub}
        onUpdateScrub={player.updateScrub}
        onCommitScrub={player.commitScrub}
        onSelectSpeed={player.setSpeed}
        onVolumeChange={(v) => {
          player.setVolume(v);
          settings.setVolume(v);
        }}
        onMuteToggle={() => {
          player.toggleMute();
          settings.setIsMuted(!player.isMuted);
        }}
        onSelectAudio={tracks.selectAudio}
        onSelectSubtitle={tracks.selectSubtitle}
        onLoadExternalSubtitle={tracks.loadExternalSubtitle}
        onToggleFullscreen={handleToggleFullscreen}
        onUnauthorizedAction={handleUnauthorizedAction}
        bindHover={bindHover}
      />

      {/* Host Authority Toast (for non-hosts in HostOnly mode) */}
      <HostAuthorityToast message={room.authorityToast} />

      {room.error && !isModalOpen && <div role="alert" className="absolute top-24 left-4 right-4 z-40 p-3 rounded-xl bg-rose-950 text-rose-200">{room.error}</div>}

      {/* Modals */}
      <CreateRoomModal
        isOpen={isCreateModalOpen}
        onClose={() => setIsCreateModalOpen(false)}
        roomCode={room.roomCode}
        roomToken={room.roomToken}
        hostAddr={room.hostAddr}
        initialHostName={settings.displayName}
        isLoading={room.isLoading}
        error={room.error}
        onCreateRoom={room.createRoom}
        onEndRoom={room.leaveRoom}
      />

      <JoinRoomModal
        initialInvitation={invitation}
        isOpen={isJoinModalOpen}
        onClose={() => setIsJoinModalOpen(false)}
        initialDisplayName={settings.displayName}
        isLoading={room.isLoading}
        error={room.error}
        onJoinRoom={room.joinRoom}
      />

      <SettingsModal
        isOpen={isSettingsModalOpen}
        onClose={() => setIsSettingsModalOpen(false)}
        displayName={settings.displayName}
        preferredAudioLang={settings.preferredAudioLang}
        preferredSubLang={settings.preferredSubLang}
        subtitlesEnabled={settings.subtitlesEnabled}
        onDisplayNameChange={settings.setDisplayName}
        onAudioLangChange={settings.setPreferredAudioLang}
        onSubLangChange={settings.setPreferredSubLang}
        onSubtitlesEnabledChange={settings.setSubtitlesEnabled}
      />
    </div>
  );
}

export default App;
