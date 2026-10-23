import { backend } from './api';

/** How long a "closed" note stays up after the last close. */
const HOLD_MS = 4500;

/**
 * The apps Niwa closed in the last few seconds, for the card's note and the
 * mini timer's ping. One launch often starts several processes, and several
 * apps can close at once: they gather into a single note that stays up until
 * things have been quiet for a moment.
 */
export function closedFeed(onclose?: (label: string) => void) {
  let labels = $state<string[]>([]);
  /** Goes up with every close, so a ping can play again for the same app. */
  let count = $state(0);
  let timer: ReturnType<typeof setTimeout>;

  backend.on('app-closed', ({ label }) => {
    if (!labels.includes(label)) labels = [...labels, label];
    count += 1;
    onclose?.(label);
    clearTimeout(timer);
    timer = setTimeout(() => (labels = []), HOLD_MS);
  });

  return {
    get labels() {
      return labels;
    },
    get count() {
      return count;
    },
    /** "Discord", "Discord and Spotify", "Discord and 2 others". */
    get text() {
      if (labels.length <= 2) return labels.join(' and ');
      return `${labels[0]} and ${labels.length - 1} others`;
    },
    clear() {
      clearTimeout(timer);
      labels = [];
    },
  };
}
