/** "18:20", or "1:05:00" past an hour. */
export function clock(secs: number): string {
  const s = Math.max(0, Math.ceil(secs));
  const pad = (n: number) => String(n).padStart(2, '0');
  const hours = Math.floor(s / 3600);
  const minutes = Math.floor((s % 3600) / 60);
  return hours > 0
    ? `${hours}:${pad(minutes)}:${pad(s % 60)}`
    : `${pad(minutes)}:${pad(s % 60)}`;
}

/** How long ending a session early waits, in minutes: five in Gentle; in
 *  Strict a third of the session, rounded up, never under five. Mirrors
 *  `Mode::exit_wait_secs` in niwa-core. */
export function exitWaitMinutes(mode: 'gentle' | 'strict', plannedMinutes: number): number {
  return mode === 'gentle' ? 5 : Math.max(5, Math.ceil(plannedMinutes / 3));
}

/** "50 min", "2 h", "1 h 15 min" */
export function duration(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m} min`;
  return m === 0 ? `${h} h` : `${h} h ${m} min`;
}

/** "50m", "2h", "1h 15m": for tight spots like garden tiles. */
export function shortDuration(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return `${m}m`;
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}

/** "50 minutes", "1 minute" */
export function minutesInWords(minutes: number): string {
  return minutes === 1 ? '1 minute' : `${minutes} minutes`;
}

/** "45 minutes", "1 hour", "1 hour 30 minutes", "5 hours" */
export function lengthInWords(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  if (h === 0) return minutesInWords(m);
  const hours = h === 1 ? '1 hour' : `${h} hours`;
  return m === 0 ? hours : `${hours} ${minutesInWords(m)}`;
}

const DAY = new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' });

/** "Oct 1" */
export function shortDate(ms: number): string {
  return DAY.format(new Date(ms));
}

/** "no slips", "1 slip", "3 slips" */
export function slipsInWords(slips: number): string {
  if (slips === 0) return 'no slips';
  return slips === 1 ? '1 slip' : `${slips} slips`;
}
