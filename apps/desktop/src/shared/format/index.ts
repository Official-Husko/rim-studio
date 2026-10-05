// Display formatting only: no rules, no rounding that changes meaning. Locale is fixed to
// English until the i18n layer grows other catalogs.
const LOCALE = 'en';

/** A number with grouping and at most `digits` decimals (default 2). */
export function formatNumber(value: number, digits = 2): string {
  if (!Number.isFinite(value)) return '-';
  return new Intl.NumberFormat(LOCALE, { maximumFractionDigits: digits }).format(value);
}

const UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB'] as const;

/** A byte count in binary units, for example "1.5 MiB". */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '-';
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const digits = unit === 0 || value >= 100 ? 0 : 1;
  return `${formatNumber(value, digits)} ${UNITS[unit]}`;
}

/** A duration in milliseconds as "420 ms", "1.2 s", "2 min 5 s" or "1 h 3 min". */
export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return '-';
  if (ms < 1000) return `${Math.round(ms)} ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${formatNumber(seconds, seconds < 10 ? 1 : 0)} s`;
  const totalSeconds = Math.round(seconds);
  const minutes = Math.floor(totalSeconds / 60);
  if (minutes < 60) {
    const rest = totalSeconds % 60;
    return rest ? `${minutes} min ${rest} s` : `${minutes} min`;
  }
  const hours = Math.floor(minutes / 60);
  const restMinutes = minutes % 60;
  return restMinutes ? `${hours} h ${restMinutes} min` : `${hours} h`;
}

/** Pick the singular or plural form for a count: plural(3, "file", "files"). */
export function plural(count: number, one: string, other: string): string {
  return count === 1 ? one : other;
}
