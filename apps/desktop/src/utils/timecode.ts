export function formatTimecode(seconds: number, forceHours = false): string {
  if (isNaN(seconds) || seconds < 0) {
    return forceHours ? '00:00:00' : '00:00';
  }
  const totalSecs = Math.floor(seconds);
  const hrs = Math.floor(totalSecs / 3600);
  const mins = Math.floor((totalSecs % 3600) / 60);
  const secs = totalSecs % 60;

  const pad = (n: number) => n.toString().padStart(2, '0');

  if (hrs > 0 || forceHours) {
    return `${pad(hrs)}:${pad(mins)}:${pad(secs)}`;
  }
  return `${pad(mins)}:${pad(secs)}`;
}

export function formatDurationPair(currentSec: number, totalSec: number): {
  current: string;
  total: string;
  formatted: string;
} {
  const needsHours = totalSec >= 3600 || currentSec >= 3600;
  const currentStr = formatTimecode(currentSec, needsHours);
  const totalStr = formatTimecode(totalSec, needsHours);
  return {
    current: currentStr,
    total: totalStr,
    formatted: `${currentStr} / ${totalStr}`,
  };
}
