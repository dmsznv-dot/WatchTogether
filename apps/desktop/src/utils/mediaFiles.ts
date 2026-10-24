/** File types the player accepts; audio matches EXTERNAL_AUDIO_EXTENSIONS in media-engine. */
export const VIDEO_EXTENSIONS = ['mp4', 'mov', 'mkv'];
export const SUBTITLE_EXTENSIONS = ['srt', 'ass'];
export const AUDIO_EXTENSIONS = [
  'mp3', 'm4a', 'aac', 'ac3', 'eac3', 'dts', 'thd', 'flac', 'wav', 'ogg', 'oga', 'opus', 'mka', 'wma', 'mp2',
];

export type MediaFileKind = 'video' | 'subtitle' | 'audio';

/** What a dropped or picked file is, by extension; null when the player cannot use it. */
export function mediaFileKind(path: string): MediaFileKind | null {
  const ext = path.toLowerCase().split('.').pop() ?? '';
  if (VIDEO_EXTENSIONS.includes(ext)) return 'video';
  if (SUBTITLE_EXTENSIONS.includes(ext)) return 'subtitle';
  if (AUDIO_EXTENSIONS.includes(ext)) return 'audio';
  return null;
}
