import { reduced } from './motion.svelte';

/** How far a press must travel before it becomes a drag rather than a click. */
const THRESHOLD = 6;
/** Apple's scroll deceleration rate: how far a fling would coast. */
const DECELERATION = 0.998;
/** The settling spring: critically damped, about 0.4 s to arrive. */
const RESPONSE = 0.4;

/**
 * Drag to scroll a horizontal row with a mouse or pen, and glide it with a
 * mouse wheel. Touch and sideways touchpad swipes keep the browser's own
 * scrolling.
 *
 * The row follows the pointer one to one from where it was grabbed. Let go
 * and it carries the pointer's speed, coasts to where that speed would take
 * it, and settles with an item lined up at the start edge. Past either end it
 * stretches with growing resistance and springs back. A drag never counts as
 * a click on the item under the pointer, and grabbing the row mid-glide
 * catches it where it is.
 *
 * `node` is the scroll container. Its single child is the track: moved for
 * the stretch past the ends, and its children are the items lined up.
 */
export function dragScroll(node: HTMLElement) {
  const track = node.firstElementChild as HTMLElement;
  /** Where the row is, in scroll pixels; below 0 or past the end while stretched. */
  let position = 0;
  let frame = 0;
  /** The spring's speed in px/s, and where it's heading, while it runs. */
  let speed = 0;
  let heading: number | null = null;
  let press: { id: number; x: number; from: number; moved: boolean; samples: [number, number][] } | null = null;
  let swallowClick = false;

  const end = () => Math.max(0, node.scrollWidth - node.clientWidth);

  /** Past an end, the row moves less the further it's pulled. */
  function stretch(over: number): number {
    const room = node.clientWidth;
    return (over * room * 0.55) / (room + 0.55 * Math.abs(over));
  }

  function show(to: number) {
    position = to;
    const inside = Math.min(Math.max(to, 0), end());
    node.scrollLeft = inside;
    const over = to - inside;
    track.style.transform = over ? `translateX(${-stretch(over)}px)` : '';
  }

  /** The scroll position that lines each item up with the row's start padding. */
  function stops(): number[] {
    const padding = parseFloat(getComputedStyle(node).paddingLeft) || 0;
    const items = [...track.children] as HTMLElement[];
    return items.map((item) => Math.min(Math.max(item.offsetLeft - padding, 0), end()));
  }

  function nearestStop(to: number): number {
    const all = stops();
    if (to >= end() - 1 || all.length === 0) return end();
    return all.reduce((best, stop) => (Math.abs(stop - to) < Math.abs(best - to) ? stop : best), all[0]);
  }

  /** Settles on `target` with a critically damped spring, starting at `velocity` px/s. */
  function settle(target: number, velocity: number) {
    stop();
    if (reduced()) {
      show(target);
      return;
    }
    const omega = (2 * Math.PI) / RESPONSE;
    speed = velocity;
    heading = target;
    let last = performance.now();
    const step = (now: number) => {
      const dt = Math.min((now - last) / 1000, 1 / 30);
      last = now;
      const accel = -omega * omega * (position - target) - 2 * omega * speed;
      speed += accel * dt;
      const next = position + speed * dt;
      if (Math.abs(next - target) < 0.5 && Math.abs(speed) < 20) {
        show(target);
        stop();
        return;
      }
      show(next);
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
  }

  function stop() {
    cancelAnimationFrame(frame);
    speed = 0;
    heading = null;
  }

  function onpointerdown(event: PointerEvent) {
    if (event.pointerType === 'touch' || event.button !== 0) return;
    swallowClick = false;
    // Catch a gliding row where it is.
    stop();
    if (!track.style.transform) position = node.scrollLeft;
    press = { id: event.pointerId, x: event.clientX, from: position, moved: false, samples: [[event.timeStamp, event.clientX]] };
  }

  function onpointermove(event: PointerEvent) {
    if (!press || event.pointerId !== press.id) return;
    const dx = event.clientX - press.x;
    if (!press.moved) {
      if (Math.abs(dx) < THRESHOLD) return;
      press.moved = true;
      node.setPointerCapture(press.id);
      node.classList.add('dragging');
    }
    show(press.from - dx);
    press.samples.push([event.timeStamp, event.clientX]);
    // Only the last tenth of a second says how fast the hand is moving now.
    while (press.samples.length > 2 && event.timeStamp - press.samples[0][0] > 100) press.samples.shift();
  }

  function onpointerup(event: PointerEvent) {
    if (!press || event.pointerId !== press.id) return;
    const { moved, samples } = press;
    press = null;
    node.classList.remove('dragging');
    if (!moved) return;
    // The click that follows this release comes straight after it, if at all.
    swallowClick = true;
    setTimeout(() => (swallowClick = false), 0);
    const [[t0, x0], [t1, x1]] = [samples[0], samples[samples.length - 1]];
    // A hand that stopped before letting go throws nothing.
    const still = event.timeStamp - t1 > 80;
    const pointerSpeed = t1 > t0 && event.type === 'pointerup' && !still ? (x1 - x0) / (t1 - t0) : 0; // px/ms
    const velocity = -pointerSpeed * 1000; // scroll px/s
    // Where the fling would coast to, then the item nearest that.
    const coast = (velocity / 1000) * (DECELERATION / (1 - DECELERATION));
    settle(nearestStop(Math.min(Math.max(position + coast, 0), end())), velocity);
  }

  /** The click that ends a drag isn't a choice. */
  function onclick(event: MouseEvent) {
    if (!swallowClick) return;
    swallowClick = false;
    event.preventDefault();
    event.stopPropagation();
  }

  /**
   * An up-and-down wheel glides the row sideways, each notch adding to where
   * it's already heading and keeping its speed. A sideways swipe scrolls
   * natively, and takes over from any glide.
   */
  function onwheel(event: WheelEvent) {
    if (press) return;
    if (event.deltaY === 0 || event.deltaX !== 0) {
      stop();
      track.style.transform = '';
      position = node.scrollLeft;
      return;
    }
    event.preventDefault();
    const from = heading ?? node.scrollLeft;
    if (heading === null) position = node.scrollLeft;
    settle(Math.min(Math.max(from + event.deltaY, 0), end()), speed);
  }

  node.addEventListener('pointerdown', onpointerdown);
  node.addEventListener('pointermove', onpointermove);
  node.addEventListener('pointerup', onpointerup);
  node.addEventListener('pointercancel', onpointerup);
  node.addEventListener('click', onclick, true);
  node.addEventListener('wheel', onwheel, { passive: false });
  return {
    destroy() {
      stop();
      node.removeEventListener('pointerdown', onpointerdown);
      node.removeEventListener('pointermove', onpointermove);
      node.removeEventListener('pointerup', onpointerup);
      node.removeEventListener('pointercancel', onpointerup);
      node.removeEventListener('click', onclick, true);
      node.removeEventListener('wheel', onwheel);
    },
  };
}
