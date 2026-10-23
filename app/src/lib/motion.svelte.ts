// Niwa keeps its own motion setting rather than following Windows: many
// people turn Windows animations off for speed, and the card's motion is
// gentle enough to keep. Settings > Reduce motion swaps movement for fades.
//
// The choice lives in localStorage, which both windows (the card and the mini
// timer) share, and is mirrored onto <html data-motion> for app.css.

const KEY = 'niwa.reduceMotion';

function read(): boolean {
  try {
    return localStorage.getItem(KEY) === 'true';
  } catch {
    return false;
  }
}

let isReduced = $state(read());

function apply() {
  document.documentElement.dataset.motion = isReduced ? 'reduced' : 'full';
}
apply();

// The other window changed it.
window.addEventListener('storage', (event) => {
  if (event.key === KEY) {
    isReduced = read();
    apply();
  }
});

/** Whether movement is swapped for fades. Reactive. */
export function reduced(): boolean {
  return isReduced;
}

export function setReduced(on: boolean) {
  isReduced = on;
  apply();
  try {
    localStorage.setItem(KEY, String(on));
  } catch {
    // It still applies until the window closes.
  }
}

/** A distance to travel, or none when motion is reduced. Fades are kept either way. */
export function travel(distance: number): number {
  return isReduced ? 0 : distance;
}

/** The curve sheets and larger surfaces settle on: quick to start, long soft landing. */
export const settle = (t: number) => 1 - Math.pow(1 - t, 4);
