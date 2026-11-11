// Format: numbers Rust sends, written for people.

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

/** A tool's version without build details: `n9.0.2-39-g7adf…` → `n9.0.2`, `7.1-full_build-…` → `7.1`,
 * `N-118858-g740d…-20250318` → `N-118858` (FFmpeg master builds have no version number). */
export function shortVersion(version: string): string {
  const parts = version.trim().split(/[-:_\s]/);
  return /\d/.test(parts[0]) ? parts[0] : parts.slice(0, 2).join("-");
}

/** `19` → `"0:19"`, `3725` → `"1:02:05"`. */
export function formatDuration(seconds: number): string {
  const total = Math.round(seconds);
  const [h, m, s] = [Math.floor(total / 3600), Math.floor((total % 3600) / 60), total % 60];
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}

/** When a download finished, from unix seconds: the time today, "Yesterday", the weekday this week, else the date. */
export function formatWhen(seconds: number, now = new Date()): string {
  const when = new Date(seconds * 1000);
  const midnight = (date: Date) => new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
  const daysAgo = Math.round((midnight(now) - midnight(when)) / 86_400_000);
  if (daysAgo <= 0) return when.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  if (daysAgo === 1) return "Yesterday";
  if (daysAgo < 7) return when.toLocaleDateString(undefined, { weekday: "long" });
  return when.toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

/** A `YYYY-MM-DD` day as a short local date: `"10 Nov"` or `"Nov 10"`, by locale. */
export function formatDay(day: string): string {
  const [year, month, date] = day.split("-").map(Number);
  return new Date(year, month - 1, date).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

/** `1, "file"` → `"1 file"`, `40, "file"` → `"40 files"`, with `more` → `"50+ files"`. */
export function formatCount(count: number, noun: string, more = false): string {
  return `${count}${more ? "+" : ""} ${noun}${count === 1 && !more ? "" : "s"}`;
}

/** `18000` → `"18K views"`, `1` → `"1 view"`. */
export function formatViews(views: number): string {
  const short = new Intl.NumberFormat(undefined, { notation: "compact", maximumFractionDigits: 1 }).format(views);
  return `${short} ${views === 1 ? "view" : "views"}`;
}
