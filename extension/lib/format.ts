// Format: sizes and lengths written for people, the same way the app writes them.

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** `45_600_000` → `"45.6 MB"`. Decimal units, one decimal from MB up. */
export function formatBytes(bytes: number): string {
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit++;
  }
  return `${unit >= 2 ? value.toFixed(1) : Math.round(value)} ${UNITS[unit]}`;
}

/** `3725` → `"1:02:05"`, `65` → `"1:05"`. */
export function formatDuration(seconds: number): string {
  const whole = Math.round(seconds);
  const [hours, minutes, rest] = [Math.floor(whole / 3600), Math.floor(whole / 60) % 60, whole % 60];
  const pad = (part: number) => String(part).padStart(2, "0");
  return hours ? `${hours}:${pad(minutes)}:${pad(rest)}` : `${minutes}:${pad(rest)}`;
}
