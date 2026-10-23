// Words for focus times. The rules live in niwa-core's schedule.rs; this only
// describes them.

import type { Schedule } from './api';

const TIME = new Intl.DateTimeFormat(undefined, { hour: 'numeric', minute: '2-digit' });
const WEEKDAY = new Intl.DateTimeFormat(undefined, { weekday: 'short' });
export const DAY_LETTERS = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];
export const DAY_NAMES = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];
const SHORT_DAYS = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
const EVERY_DAY = 0b111_1111;
const WEEKDAYS = 0b001_1111;
const WEEKENDS = 0b110_0000;

/** "9:00 AM" or "09:00", as the user's clock shows it. */
export function timeText(minutes: number): string {
  const at = new Date(2026, 0, 5, Math.floor(minutes / 60), minutes % 60);
  return TIME.format(at);
}

/** "Weekdays", "Every day", "Mon, Wed, Fri". */
export function daysText(days: number): string {
  if ((days & EVERY_DAY) === EVERY_DAY) return 'Every day';
  if (days === WEEKDAYS) return 'Weekdays';
  if (days === WEEKENDS) return 'Weekends';
  return SHORT_DAYS.filter((_, i) => days & (1 << i)).join(', ');
}

/** How long each window lasts, in minutes. */
export function lengthMinutes(schedule: Pick<Schedule, 'start' | 'end'>): number {
  return schedule.end > schedule.start ? schedule.end - schedule.start : 1440 - schedule.start + schedule.end;
}

/** The next time any enabled focus time starts after `now`. */
export function nextStart(schedules: Schedule[], now = new Date()): Date | null {
  let best: Date | null = null;
  for (const schedule of schedules.filter((s) => s.enabled)) {
    for (let ahead = 0; ahead <= 7; ahead++) {
      const day = new Date(now.getFullYear(), now.getMonth(), now.getDate() + ahead);
      const weekday = (day.getDay() + 6) % 7;
      if (!(schedule.days & (1 << weekday))) continue;
      const starts = new Date(day.getFullYear(), day.getMonth(), day.getDate(), 0, schedule.start);
      if (starts > now) {
        if (!best || starts < best) best = starts;
        break;
      }
    }
  }
  return best;
}

/** "Today 9:00 AM", "Tomorrow 9:00 AM", "Mon 9:00 AM". */
export function whenText(at: Date, now = new Date()): string {
  const days = Math.round(
    (new Date(at.getFullYear(), at.getMonth(), at.getDate()).getTime() -
      new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()) /
      86_400_000,
  );
  const day = days === 0 ? 'Today' : days === 1 ? 'Tomorrow' : WEEKDAY.format(at);
  return `${day} ${TIME.format(at)}`;
}
