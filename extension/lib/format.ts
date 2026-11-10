// Format: sizes written for people, the same way the app writes them.

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
