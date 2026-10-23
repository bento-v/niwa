// Plant art. Every drawing sits in an 80 x 72 box with the pot at the bottom.
//
// The set is drawn to one method so the plants read as a family:
//   - light comes from the upper left, so every form has a lit and a shaded side;
//   - leaves are leaf-shaped and split along the midrib into two tones;
//   - trunks and branches are tapered, filled shapes with a shaded edge;
//   - each stage is the same plant grown further, not the final drawing scaled.
//
// A drawing has two layers: `back` is drawn behind the pot (stems tuck into
// it) and `front` on top of it (moss, fallen leaves).

export type SpeciesId =
  | 'sprout'
  | 'succulent'
  | 'camellia'
  | 'pine'
  | 'fern'
  | 'bamboo'
  | 'hydrangea'
  | 'maple'
  | 'sakura'
  | 'wisteria'
  | 'ginkgo';

export interface Drawing {
  back: string;
  front?: string;
}

export const STAGE_NAMES = ['seed', 'sprout', 'sapling', 'young', 'fully grown'] as const;

type Point = [number, number];
/** A light and a dark shade of one colour: the lit and the shaded side. */
type Tone = [light: string, dark: string];
/** A centre line with a width at each point. */
type Spine = [x: number, y: number, width: number][];

// ------------------------------------------------------------ palette
const LEAF_DEEP: Tone = ['#5F7F43', '#46602F'];
const LEAF: Tone = ['#7A9A55', '#5F7F43'];
const LEAF_FRESH: Tone = ['#93B06C', '#76985A'];
const LEAF_NEW: Tone = ['#B4CB8E', '#93B06C'];
const WOOD: Tone = ['#6F5239', '#553D2A'];
const SOIL: Tone = ['#77594A', '#5E4634'];
const DRY: Tone = ['#C9B79C', '#A89379'];

// ------------------------------------------------------------ drawing kit
const r = (value: number) => Math.round(value * 100) / 100;
const pt = ([x, y]: Point) => `${r(x)} ${r(y)}`;
const fill = (d: string, color: string) => `<path d="${d}" fill="${color}"/>`;
const line = (d: string, color: string, width: number) =>
  `<path d="${d}" fill="none" stroke="${color}" stroke-width="${width}" stroke-linecap="round"/>`;
const lerp = (a: Point, b: Point, t: number): Point => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
/** Moves a point toward or away from an origin, to draw a younger plant. */
const toward = (origin: Point, point: Point, scale: number): Point => lerp(origin, point, scale);

/**
 * A leaf from `base` to `tip`, `width` across at its widest, split along the
 * midrib. The half facing the light gets the lighter shade. `bend` bows the
 * leaf sideways.
 */
function leaf(base: Point, tip: Point, width: number, [light, dark]: Tone, bend = 0): string {
  const dx = tip[0] - base[0];
  const dy = tip[1] - base[1];
  const length = Math.hypot(dx, dy) || 1;
  const nx = dy / length;
  const ny = -dx / length;
  // The widest point sits a little before the middle, as on a real leaf.
  const wide: Point = [base[0] + dx * 0.42, base[1] + dy * 0.42];
  const rib: Point = [base[0] + dx * 0.5 + nx * bend * 2, base[1] + dy * 0.5 + ny * bend * 2];
  const edge = (side: number): Point => [
    wide[0] + nx * (side * width + bend) * 2,
    wide[1] + ny * (side * width + bend) * 2,
  ];
  const half = (control: Point, color: string) =>
    fill(`M${pt(base)} Q${pt(control)} ${pt(tip)} Q${pt(rib)} ${pt(base)} Z`, color);
  const firstIsLit = nx + ny < 0;
  return half(edge(1), firstIsLit ? light : dark) + half(edge(-1), firstIsLit ? dark : light);
}

/** A smooth curve through the points, continuing a path that is already at the first. */
function through(points: Point[]): string {
  let d = '';
  for (let i = 0; i < points.length - 1; i++) {
    const p0 = points[Math.max(i - 1, 0)];
    const p1 = points[i];
    const p2 = points[i + 1];
    const p3 = points[Math.min(i + 2, points.length - 1)];
    d += ` C${r(p1[0] + (p2[0] - p0[0]) / 6)} ${r(p1[1] + (p2[1] - p0[1]) / 6)} ${r(
      p2[0] - (p3[0] - p1[0]) / 6,
    )} ${r(p2[1] - (p3[1] - p1[1]) / 6)} ${pt(p2)}`;
  }
  return d;
}

/** The right-hand normal at each point of a spine. */
function normals(spine: Spine): Point[] {
  return spine.map((_, i) => {
    const [px, py] = spine[Math.max(i - 1, 0)];
    const [qx, qy] = spine[Math.min(i + 1, spine.length - 1)];
    const length = Math.hypot(qx - px, qy - py) || 1;
    return [-(qy - py) / length, (qx - px) / length];
  });
}

/** A tapered band along a spine, between two edges (-1 is the left, 1 the right). */
function band(spine: Spine, from: number, to: number): string {
  const normal = normals(spine);
  const edge = (e: number): Point[] =>
    spine.map(([x, y, w], i) => [x + normal[i][0] * (w / 2) * e, y + normal[i][1] * (w / 2) * e]);
  const near = edge(from);
  const far = edge(to).reverse();
  return `M${pt(near[0])}${through(near)} L${pt(far[0])}${through(far)} Z`;
}

/** A trunk, branch or stem: the full shape, then a shaded band on its lower-right side. */
function limb(spine: Spine, [light, dark]: Tone = WOOD, shade = true): string {
  const body = fill(band(spine, -1, 1), light);
  if (!shade) return body;
  const [sx, sy] = normals(spine).reduce(([ax, ay], [bx, by]) => [ax + bx, ay + by]);
  const shadeOnRight = sx + sy >= 0;
  return body + fill(shadeOnRight ? band(spine, 0.15, 1) : band(spine, -1, -0.15), dark);
}

const place = (x: number, y: number, scale: number, inner: string, rotate = 0) =>
  `<g transform="translate(${r(x)} ${r(y)})${rotate ? ` rotate(${rotate})` : ''} scale(${r(scale)})">${inner}</g>`;

/** A small, repeatable source of variation, so foliage looks grown rather than stamped. */
function scatter(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = Math.imul(state ^ (state >>> 15), state | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The three shades of one layer of foliage: its underside, its body and its lit top. */
interface Foliage {
  shade: string;
  body: string;
  light: string;
}

/** How one kind of leaf is drawn inside a clump. Angles are in degrees, 0 pointing up. */
interface LeafKind {
  /** How wide the leaf is at size 1, which sets how closely leaves sit along an edge. */
  span: number;
  size: number;
  stamp(x: number, y: number, size: number, angle: number, color: string): string;
  /** A leaf on the lit top, where there is room for more detail. Defaults to `stamp` in the light shade. */
  lit?(x: number, y: number, size: number, angle: number, colors: Foliage): string;
}

/** Where the lit leaves sit in a clump, as fractions of its half-width and half-height. */
const LIT_SPOTS: Point[] = [
  [-0.5, -0.34],
  [-0.12, -0.6],
  [0.24, -0.44],
  [-0.34, 0.02],
  [0.06, -0.14],
];

/**
 * A clump of foliage: a mass whose outline is made of its own leaves, shaded
 * underneath and lit from the upper left. Canopies are built by overlapping
 * clumps from back to front, so each one's lit top sits against the shaded
 * underside of the clump behind it. That overlap is what gives depth; leaves
 * are never laid flat over a single shape.
 */
function clump(cx: number, cy: number, width: number, height: number, c: Foliage, kind: LeafKind, seed: number): string {
  const rx = width / 2;
  const ry = height / 2;
  const vary = scatter(seed);
  const count = Math.max(8, Math.round((Math.PI * (rx + ry)) / (kind.span * kind.size * 0.56)));

  let top = '';
  let under = '';
  for (let i = 0; i < count; i++) {
    const a = ((i + vary() * 0.6) / count) * Math.PI * 2;
    const x = cx + Math.cos(a) * rx * 0.9;
    const y = cy + Math.sin(a) * ry * 0.9;
    // Leaves on the rim point outward, along the ellipse's normal.
    const angle = (Math.atan2(Math.cos(a) * ry, -Math.sin(a) * rx) * 180) / Math.PI + (vary() - 0.5) * 40;
    const size = kind.size * (0.8 + vary() * 0.35);
    if (Math.sin(a) > 0.3) under += kind.stamp(x, y, size, angle, c.shade);
    else top += kind.stamp(x, y, size, angle, c.body);
  }

  // The underside: a crescent along the bottom, heavier on the side away from the light.
  const edge = cy + ry * 0.2;
  const crescent = fill(
    `M${r(cx - rx * 0.98)} ${r(edge)} A${r(rx)} ${r(ry)} 0 0 0 ${r(cx + rx * 0.98)} ${r(edge)} Q${r(cx - rx * 0.2)} ${r(cy + ry * 0.66)} ${r(cx - rx * 0.98)} ${r(edge)} Z`,
    c.shade,
  );

  // Body-coloured leaves hanging over the crescent break up its edge.
  let fringe = '';
  for (const u of [-0.56, -0.16, 0.24, 0.62]) {
    fringe += kind.stamp(
      cx + rx * (u + (vary() - 0.5) * 0.14),
      cy + ry * (0.26 + vary() * 0.12),
      kind.size * 0.9,
      180 + u * 50 + (vary() - 0.5) * 30,
      c.body,
    );
  }

  let lit = '';
  const spots = LIT_SPOTS.slice(0, rx > 6 ? 5 : rx > 4.4 ? 4 : 3);
  for (const [u, v] of spots) {
    const x = cx + rx * (u + (vary() - 0.5) * 0.16);
    const y = cy + ry * (v + (vary() - 0.5) * 0.16);
    const angle = (Math.atan2(u, -v + 0.5) * 180) / Math.PI + (vary() - 0.5) * 36;
    const size = kind.size * (0.78 + vary() * 0.24);
    lit += kind.lit ? kind.lit(x, y, size, angle, c) : kind.stamp(x, y, size, angle, c.light);
  }

  return (
    `<ellipse cx="${r(cx)}" cy="${r(cy)}" rx="${r(rx * 0.94)}" ry="${r(ry * 0.94)}" fill="${c.body}"/>` +
    top +
    crescent +
    under +
    fringe +
    lit
  );
}

/** One clump in a canopy: its place, its size, which layer's shades it uses, and its seed. */
type ClumpSpec = [cx: number, cy: number, width: number, height: number, layer: Foliage, seed: number];
const clumps = (kind: LeafKind, specs: ClumpSpec[]) =>
  specs.map(([cx, cy, width, height, layer, seed]) => clump(cx, cy, width, height, layer, kind, seed)).join('');

/** Greens for the back, middle and front of a canopy: darker and duller the further back. */
const GREEN_BACK: Foliage = { shade: '#374B27', body: '#46602F', light: '#557339' };
const GREEN_MID: Foliage = { shade: '#46602F', body: '#5A7A3F', light: '#76985A' };
const GREEN_FRONT: Foliage = { shade: '#50703A', body: '#688A49', light: '#93B06C' };

// ------------------------------------------------------------ pot & soil
const POT = {
  clay: { body: '#A8825E', shade: '#93704F', band: '#8A6848', rim: '#9A7451', rimShade: '#84623F', shine: '#C39D74' },
  grey: { body: '#A08A72', shade: '#8D7862', band: '#85715C', rim: '#917C66', rimShade: '#7D6A56', shine: '#B3A08B' },
};

/** The rim, with a chip knocked out of it on the withered pot. */
const RIM_WHOLE = 'M26.8 45.8 H53.2 Q55.6 45.8 55.6 48.2 V49 Q55.6 51.4 53.2 51.4 H26.8 Q24.4 51.4 24.4 49 V48.2 Q24.4 45.8 26.8 45.8 Z';
const RIM_CHIPPED = 'M26.8 45.8 H31.2 L32.9 47.6 L34.8 45.8 H53.2 Q55.6 45.8 55.6 48.2 V49 Q55.6 51.4 53.2 51.4 H26.8 Q24.4 51.4 24.4 49 V48.2 Q24.4 45.8 26.8 45.8 Z';

function pot(withered: boolean, detailed: boolean): string {
  const c = withered ? POT.grey : POT.clay;
  return (
    (detailed ? `<ellipse cx="40" cy="69.7" rx="18.5" ry="1.9" fill="#3E3226" opacity="0.1"/>` : '') +
    fill('M27.2 50.5 H52.8 L50.3 66.4 Q49.9 69 47.2 69 H32.8 Q30.1 69 29.7 66.4 Z', c.body) +
    // The side away from the light, cut parallel to the pot's wall.
    fill('M45.2 50.5 H52.8 L50.3 66.4 Q49.9 69 47.2 69 H42.4 Z', c.shade) +
    // The rim's shadow on the body.
    fill('M27.2 50.5 H52.8 L52.4 53 H27.6 Z', c.band) +
    fill(withered ? RIM_CHIPPED : RIM_WHOLE, c.rim) +
    fill('M45 45.8 H53.2 Q55.6 45.8 55.6 48.2 V49 Q55.6 51.4 53.2 51.4 H43.6 Z', c.rimShade) +
    (withered
      ? // The broken face inside the chip.
        fill('M32.9 47.6 L34.8 45.8 H35.5 L33.3 48.1 Z', c.rimShade)
      : '') +
    // The shine stops short of the chip on the withered pot.
    (detailed
      ? `<rect x="27" y="46.9" width="${withered ? 3.6 : 11}" height="1.3" rx="0.65" fill="${c.shine}"/>`
      : '')
  );
}

/** A heap of soil for the seed stage, with a seed drawn on top by each species. */
const MOUND =
  fill('M30.4 47.2 C32.6 43.4 36.6 41.6 40.4 41.8 C44.2 42 47.6 43.8 49.6 47.2 Z', SOIL[1]) +
  fill('M32.2 46 C34.2 43.6 37 42.4 39.8 42.4 C37.6 43.2 35.6 44.4 34.2 46.4 Z', SOIL[0]);

/** A rounded seed lying on the mound, lit from the upper left. */
const seed = (cx: number, cy: number, rx: number, ry: number, rotate: number, [light, dark]: Tone) =>
  `<g transform="rotate(${rotate} ${cx} ${cy})">` +
  `<ellipse cx="${cx}" cy="${cy}" rx="${rx}" ry="${ry}" fill="${dark}"/>` +
  `<ellipse cx="${r(cx - rx * 0.18)}" cy="${r(cy - ry * 0.22)}" rx="${r(rx * 0.74)}" ry="${r(ry * 0.66)}" fill="${light}"/>` +
  `</g>`;

// ---------------------------------------------------------------- sprout
const SPROUT_STEM: Tone = ['#7A9A55', '#5E7A3F'];

const SPROUT: Drawing[] = [
  { back: MOUND + seed(40.2, 42.4, 2.7, 1.9, -14, ['#D8BC8E', '#B8956A']) },
  {
    back:
      limb([[40, 47.5, 1.5], [39.8, 44.6, 1.3], [40.1, 42.2, 1]], SPROUT_STEM) +
      leaf([40, 43], [35.2, 40.2], 1.7, LEAF_FRESH, 0.2) +
      leaf([40.1, 42.6], [44.8, 39.4], 1.7, LEAF_FRESH, -0.2),
  },
  {
    back:
      limb([[40, 47.5, 1.8], [39.7, 43.5, 1.6], [40.2, 40, 1.3], [40, 37.6, 1]], SPROUT_STEM) +
      leaf([40, 39.6], [32, 34.6], 2.9, LEAF_FRESH, 0.3) +
      leaf([40.1, 38.6], [47.8, 33], 2.9, LEAF_FRESH, -0.3),
  },
  {
    back:
      limb([[40, 47.5, 2], [39.5, 42.5, 1.8], [40.3, 37.5, 1.5], [40, 33.6, 1.1]], SPROUT_STEM) +
      leaf([40, 37.2], [29.6, 30.2], 3.8, LEAF_FRESH, 0.4) +
      leaf([40.2, 35.8], [50.2, 28.2], 3.8, LEAF_FRESH, -0.4) +
      leaf([40, 34.4], [40.6, 29.6], 1.3, LEAF_NEW),
  },
  {
    back:
      limb([[40, 47.5, 2.3], [39.4, 42, 2.1], [40.4, 36, 1.7], [40, 30.4, 1.2], [40.3, 28, 0.9]], SPROUT_STEM) +
      leaf([40, 35.6], [27.2, 26.4], 4.7, LEAF_FRESH, 0.5) +
      leaf([40.3, 33.6], [52.6, 23.4], 4.7, LEAF_FRESH, -0.5) +
      leaf([40.1, 30.2], [36, 22.6], 2, LEAF_NEW, 0.2) +
      leaf([40.3, 29.4], [43.4, 20.8], 2.1, LEAF_NEW, -0.2),
  },
];

// ------------------------------------------------------------- succulent
const SUCCULENT_BACK: Tone = ['#8FA87A', '#728C63'];
const SUCCULENT_FRONT: Tone = ['#ABC09A', '#8FA87A'];
const SUCCULENT_HEART: Tone = ['#C6D5B4', '#ABC09A'];
const BLUSH: Tone = ['#E0AE95', '#CC917A'];
const CROWN: Point = [40, 47.5];

/** One fleshy leaf of the rosette, growing from the crown toward `tip`. */
function fleshy(tip: Point, width: number, tone: Tone, growth: number, blush: boolean): string {
  const end = toward(CROWN, tip, growth);
  const base: Point = [CROWN[0] + (tip[0] - CROWN[0]) * 0.14, CROWN[1]];
  return (
    leaf(base, end, width * growth, tone) +
    (blush ? leaf(lerp(base, end, 0.8), end, width * growth * 0.42, BLUSH) : '')
  );
}

/** The rosette at a given size: outer leaves first, then the inner ones over them. */
function rosette(growth: number, leaves: number, blush = false): string {
  const outer: [Point, number, Tone][] = [
    [[21.6, 36], 4.3, SUCCULENT_BACK],
    [[58.4, 36], 4.3, SUCCULENT_BACK],
    [[28.6, 27.4], 4.4, SUCCULENT_BACK],
    [[51.4, 27.4], 4.4, SUCCULENT_BACK],
  ];
  const inner: [Point, number, Tone][] = [
    [[26.4, 41.4], 3.6, SUCCULENT_FRONT],
    [[53.6, 41.4], 3.6, SUCCULENT_FRONT],
    [[34, 30.6], 4, SUCCULENT_FRONT],
    [[46, 30.6], 4, SUCCULENT_FRONT],
    [[40, 24.6], 3.8, SUCCULENT_HEART],
  ];
  // Younger plants have only their innermost leaves.
  const all = [...outer, ...inner].slice(9 - leaves);
  return all.map(([tip, width, tone]) => fleshy(tip, width, tone, growth, blush)).join('');
}

const SUCCULENT: Drawing[] = [
  { back: MOUND + seed(40, 42.2, 2.2, 1.8, 0, ['#C6D5B4', '#9BB08A']) },
  { back: rosette(0.42, 3) },
  { back: rosette(0.62, 5) },
  { back: rosette(0.82, 9) },
  { back: rosette(1, 9, true) },
];

// -------------------------------------------------------------- camellia
const PETAL: Tone = ['#E9B6A1', '#D48E77'];

/** A camellia bloom seen face-on: a ring of petals, a tighter ring inside, a gold heart. */
function bloom(cx: number, cy: number, size: number, turn = 0): string {
  const ring = (count: number, distance: number, rx: number, ry: number, color: string, offset: number) =>
    Array.from({ length: count }, (_, i) => {
      const angle = turn + offset + (360 / count) * i;
      return `<ellipse cx="0" cy="${r(-distance)}" rx="${r(rx)}" ry="${r(ry)}" transform="rotate(${angle})" fill="${color}"/>`;
    }).join('');
  return place(
    cx,
    cy,
    size,
    ring(6, 0.56, 0.5, 0.56, PETAL[0], 0) +
      ring(5, 0.28, 0.34, 0.4, PETAL[1], 36) +
      `<circle r="0.24" fill="#EBC56F"/><circle cx="-0.06" cy="-0.06" r="0.11" fill="#F5DC9A"/>`,
  );
}

/** A closed bud with its green cup. */
const bud = (x: number, y: number) =>
  leaf([x, y + 1.6], [x + 0.3, y - 2], 1.25, PETAL) + leaf([x - 0.2, y + 1.9], [x + 0.1, y - 0.2], 1.3, LEAF_DEEP);

/** A glossy oval leaf. On the lit top it is drawn split along its midrib. */
const OVAL = 'M0 -3 C1.7 -1.8 1.9 0.8 0 3 C-1.9 0.8 -1.7 -1.8 0 -3 Z';
const CAMELLIA_LEAF: LeafKind = {
  span: 3.6,
  size: 0.82,
  stamp: (x, y, size, angle, color) => place(x, y, size, fill(OVAL, color), angle),
  lit: (x, y, size, angle, c) => {
    const radians = ((angle - 90) * Math.PI) / 180;
    const reach = 3 * size;
    const along: Point = [Math.cos(radians) * reach, Math.sin(radians) * reach];
    return leaf([x - along[0], y - along[1]], [x + along[0], y + along[1]], 1.7 * size, [c.light, c.body]);
  },
};

const CAMELLIA_TRUNK = limb([[40, 48, 3], [39.8, 45, 2.7], [40.2, 42, 2.4]]);

const CAMELLIA: Drawing[] = [
  { back: MOUND + seed(40.2, 42.3, 2.5, 2.1, 0, ['#9A6A48', '#74492F']) },
  {
    back:
      limb([[40, 47.5, 1.5], [39.8, 44, 1.3], [40.1, 40.4, 1]], ['#6B7A45', '#55632F']) +
      leaf([39.9, 42], [32.6, 38.8], 2.4, LEAF_DEEP, 0.3) +
      leaf([40.1, 41], [47.4, 37.2], 2.4, LEAF_DEEP, -0.3) +
      leaf([40, 40.6], [40.6, 36], 1.3, LEAF_FRESH),
  },
  {
    back:
      limb([[40, 48, 2.3], [39.7, 44, 2], [40.2, 40, 1.7]]) +
      clumps(CAMELLIA_LEAF, [
        [40, 35.4, 13.4, 11, GREEN_BACK, 11],
        [35.8, 38, 9.6, 8.4, GREEN_MID, 12],
        [44.6, 37.6, 9.6, 8.4, GREEN_MID, 13],
        [40.4, 39.8, 8.6, 6.8, GREEN_FRONT, 14],
      ]),
  },
  {
    back:
      CAMELLIA_TRUNK +
      clumps(CAMELLIA_LEAF, [
        [40, 41.6, 21, 10, GREEN_BACK, 21],
        [34.4, 31.6, 14, 11, GREEN_BACK, 22],
        [46, 31, 14, 11, GREEN_BACK, 23],
        [40, 27.6, 13, 9.6, GREEN_MID, 24],
        [31, 38.6, 11.6, 9.6, GREEN_MID, 25],
        [49, 38.6, 11.6, 9.6, GREEN_MID, 26],
      ]) +
      bud(34.6, 33.6) +
      bud(46.4, 29) +
      clumps(CAMELLIA_LEAF, [
        [37.4, 40.2, 12, 9, GREEN_FRONT, 27],
        [44.8, 40.6, 11.6, 8.6, GREEN_FRONT, 28],
      ]) +
      bud(45, 38.6),
  },
  {
    back:
      CAMELLIA_TRUNK +
      clumps(CAMELLIA_LEAF, [
        [40, 41, 24, 11, GREEN_BACK, 31],
        [33, 29.6, 16, 12, GREEN_BACK, 32],
        [47, 28.6, 16, 12, GREEN_BACK, 33],
        [40, 25, 15, 10.6, GREEN_MID, 34],
        [29.6, 37.6, 13, 11, GREEN_MID, 35],
        [50.4, 37.6, 13, 11, GREEN_MID, 36],
      ]) +
      // Two blooms sit back among the leaves; the front clumps overlap them.
      bloom(33.2, 31.4, 5.6, 10) +
      bloom(47, 26, 5.2, 40) +
      leaf([43.4, 29.8], [47.6, 28.6], 1.5, [GREEN_FRONT.light, GREEN_FRONT.body]) +
      clumps(CAMELLIA_LEAF, [
        [36.8, 39.2, 14, 10.4, GREEN_FRONT, 37],
        [46.2, 40.2, 13, 9.4, GREEN_FRONT, 38],
      ]) +
      bloom(44.4, 39.4, 5.4, 70) +
      leaf([40.8, 42.6], [45, 42.4], 1.4, [GREEN_FRONT.light, GREEN_FRONT.body]) +
      bud(53.4, 33.6),
  },
];

// ------------------------------------------------------------------ pine
const PAD_BODY = 'M-10 0 C-11 -3.4 -8.8 -6.4 -5.8 -6 C-5 -9.6 -1 -11.2 2 -9.2 C4.6 -11.4 8.6 -9.6 8.6 -6.2 C10.8 -5.4 11.2 -2.2 10 0 Q0 2 -10 0 Z';
const PAD_SHADE = 'M-10 0 C-10.3 -1.3 -10.1 -2.6 -9.4 -3.6 Q0 -0.5 10.3 -3.2 C10.7 -2.1 10.6 -1 10 0 Q0 2 -10 0 Z';

/** A cloud-shaped pad of needles, `width` across, resting on its base at (cx, cy). */
function pad(cx: number, cy: number, width: number, back = false): string {
  const [light, dark] = back ? (['#4E6A35', '#3D542A'] as Tone) : LEAF_DEEP;
  // Each pad has a darker one tucked behind it, showing above and to the right.
  const behind = back ? '' : pad(cx + width * 0.1, cy - width * 0.1, width * 0.86, true);
  return behind + place(
    cx,
    cy,
    width / 20,
    fill(PAD_BODY, light) +
      fill(PAD_SHADE, dark) +
      (back
        ? ''
        : line('M-6.6 -5.4 C-5.6 -7.8 -3 -9 -0.6 -8.4', '#8FAE68', 0.9) +
          line('M2.6 -8.6 C4.2 -9.6 6.2 -9 6.9 -7.4', '#8FAE68', 0.9) +
          line('M-8.8 -2.6 C-9.1 -3.6 -8.6 -4.5 -7.8 -4.9', '#7A9A55', 0.8)),
  );
}

const MOSS =
  fill('M30.4 46.7 C31.2 44.7 33.6 44.2 35 45.4 C36.2 43.6 39.2 43.6 40.2 45.4 C41.6 43.8 44.4 44 45.2 45.6 C46.8 44.6 49 45.2 49.6 46.7 Z', '#6E8F4A') +
  fill('M31.6 46 C32.4 44.9 33.8 44.8 34.8 45.6 C33.8 45.6 32.6 45.7 31.6 46 Z M36.2 45.2 C37 44.3 38.6 44.2 39.6 45 C38.4 44.9 37.2 45 36.2 45.2 Z', '#93B06C');

/** A pine needle, as a thin leaf from the top of the seedling's stem. */
const needle = (angle: number, length: number, tone: Tone) => {
  const radians = ((angle - 90) * Math.PI) / 180;
  return leaf([40, 40.2], [40 + Math.cos(radians) * length, 40.2 + Math.sin(radians) * length], 0.55, tone);
};

const PINE_TRUNK: Spine = [
  [40, 48, 5.8],
  [38.2, 43.4, 5.1],
  [40.4, 38.8, 4.5],
  [42.4, 34.2, 3.9],
  [41.2, 29.8, 3.4],
  [42.6, 25.4, 2.8],
  [45.4, 22.4, 2.1],
  [46.8, 20.6, 1.5],
];

const PINE: Drawing[] = [
  {
    back:
      MOUND +
      fill('M40.4 42.6 C43 39.2 46.6 38 49.6 38.8 C47.6 41.6 44 43 40.4 42.6 Z', '#DCC79E') +
      fill('M40.4 42.6 C43.6 41.4 46.8 40 49.6 38.8 C47.6 41.6 44 43 40.4 42.6 Z', '#C4AB7E') +
      seed(39.5, 42.9, 2.2, 1.6, -18, ['#8E6E50', '#6B4F38']),
  },
  {
    back:
      limb([[40, 47.5, 1.7], [39.7, 44, 1.5], [40, 40.2, 1.1]], WOOD) +
      needle(-64, 6.2, LEAF_DEEP) +
      needle(64, 6.2, LEAF_DEEP) +
      needle(-40, 7, LEAF) +
      needle(40, 7, LEAF) +
      needle(-18, 7.6, LEAF_FRESH) +
      needle(18, 7.6, LEAF) +
      needle(0, 7.8, LEAF_FRESH),
  },
  {
    back:
      limb([[40, 48, 4.3], [38.6, 43.6, 3.8], [40.4, 39.2, 3.2], [41.6, 34.8, 2.6], [41, 31, 2], [42.6, 28, 1.4]]) +
      limb([[41.2, 37.6, 1.9], [37.6, 36.4, 1.4], [33.6, 36.8, 1]]) +
      pad(32.2, 37.8, 11) +
      pad(43.2, 28.6, 13.4),
  },
  {
    back:
      limb(PINE_TRUNK) +
      limb([[41.4, 35.6, 2.4], [36.4, 33.6, 1.8], [31, 33.4, 1.3], [27.6, 34, 0.9]]) +
      pad(40.4, 25.6, 9.6, true) +
      pad(27.4, 35, 13.8) +
      pad(46.4, 21.6, 15.6),
  },
  {
    back:
      limb(PINE_TRUNK) +
      limb([[41.4, 35.6, 2.5], [36.4, 33.4, 1.9], [30.6, 33.2, 1.4], [26.4, 34, 1]]) +
      limb([[42.2, 28.6, 2], [47, 28.4, 1.5], [51.6, 29.6, 1.1], [54.4, 30.8, 0.8]]) +
      pad(39.4, 25.2, 11.4, true) +
      pad(26.4, 35, 16.4) +
      pad(54.4, 32, 12.8) +
      pad(46.8, 21.6, 18.4),
    front: MOSS,
  },
];

// ---------------------------------------------------------------- bamboo
const CULM = { body: '#86A85F', light: '#A6C27F', shade: '#6C8C49', node: '#587640', ring: '#B9CF98' };
const BAMBOO_LEAF: Tone = ['#7FA256', '#5C7C3E'];

/** A cane rising from the pot to `top`, jointed at each of `nodes`. */
function culm(x: number, top: number, width: number, nodes: number[]): string {
  const height = 48.5 - top;
  return (
    `<rect x="${r(x)}" y="${top}" width="${width}" height="${r(height)}" rx="${r(width * 0.3)}" fill="${CULM.body}"/>` +
    `<rect x="${r(x + width * 0.14)}" y="${r(top + 0.5)}" width="${r(width * 0.2)}" height="${r(height - 0.5)}" rx="${r(width * 0.1)}" fill="${CULM.light}"/>` +
    fill(
      `M${r(x + width * 0.66)} ${top} H${r(x + width * 0.7)} Q${r(x + width)} ${top} ${r(x + width)} ${r(top + width * 0.3)} V48.5 H${r(x + width * 0.66)} Z`,
      CULM.shade,
    ) +
    nodes
      .map(
        (y) =>
          `<rect x="${r(x - 0.25)}" y="${y}" width="${r(width + 0.5)}" height="1" rx="0.5" fill="${CULM.node}"/>` +
          `<rect x="${r(x)}" y="${r(y - 0.7)}" width="${r(width)}" height="0.6" fill="${CULM.ring}"/>`,
      )
      .join('')
  );
}

/** A twig from a joint, ending in a fan of slender leaves. */
function spray(from: Point, joint: Point, tips: Point[]): string {
  return (
    line(`M${pt(from)} L${pt(joint)}`, CULM.node, 0.7) +
    tips.map((tip, i) => leaf(joint, tip, 1.35, BAMBOO_LEAF, i % 2 ? 0.25 : -0.25)).join('')
  );
}

const BAMBOO: Drawing[] = [
  {
    back:
      MOUND +
      fill('M37.2 44.6 C37.6 41 39 38.4 40.2 37 C41.4 38.4 42.6 41 42.8 44.6 Z', '#8A6A4E') +
      fill('M37.2 44.6 C37.6 41 39 38.4 40.2 37 C40 39.6 39.4 42.2 38.8 44.6 Z', '#B08C66') +
      fill('M39.6 44.6 C39.8 42.4 40.2 40.6 40.8 39.4 C41.6 40.8 42 42.6 42 44.6 Z', '#A6C27F'),
  },
  {
    back: culm(38.1, 37.6, 3.8, [43]) + spray([41.9, 39.6], [44, 38.2], [[51, 36], [50.4, 40.4]]),
  },
  {
    back:
      culm(33.8, 35, 3.6, [42]) +
      culm(38.6, 25.6, 4.2, [33, 41]) +
      spray([42.8, 28], [45.6, 26], [[53.6, 22.6], [54.4, 27.4], [50.6, 20.4]]) +
      spray([33.8, 37], [31.6, 35.6], [[24.6, 33.6], [25.4, 38.2]]),
  },
  {
    back:
      culm(33.2, 25, 4.2, [33, 41.5]) +
      culm(44.6, 31, 3.8, [39.5]) +
      culm(38.6, 16, 4.6, [24.5, 33, 41.5]) +
      spray([43.2, 19], [46.4, 17], [[55.4, 13.6], [56, 18.6], [51.4, 11.4]]) +
      spray([33.2, 27.4], [30.4, 25.6], [[22, 23.6], [22.6, 28.4], [26, 20.6]]) +
      spray([48.4, 33], [50.8, 31.6], [[58.4, 30], [57.4, 34.6]]),
  },
  {
    back:
      culm(33.2, 15, 4.2, [25, 35.5]) +
      culm(44.6, 21, 3.8, [31, 40.5]) +
      culm(38.6, 7, 4.6, [17.5, 28, 38.5]) +
      spray([43.2, 10], [46.2, 8], [[55, 4.6], [56.2, 9.4], [51, 2.8]]) +
      spray([33.2, 17.6], [30.2, 15.6], [[21.4, 13.4], [22, 18.6], [25.6, 10.4]]) +
      spray([48.4, 23.4], [51, 21.8], [[59.6, 20], [59, 25], [55.6, 16.8]]) +
      spray([33.2, 27.6], [30.6, 26.6], [[22.6, 26], [23.6, 30.8]]) +
      spray([43.2, 20.4], [45.4, 19.6], [[52.4, 17.6]]),
  },
];

// ----------------------------------------------------------------- maple
const MAPLE_RED = { shade: '#A4573C', body: '#BF6E4C', light: '#D48B60', glow: '#E3A679' };
/** A small palmate leaf, about six units across, centred on its stalk end. */
const PALMATE = 'M0 -3.2 L0.8 -1.3 L2.9 -1.9 L1.7 -0.1 L2.6 1.5 L0.7 1.1 L0 2.6 L-0.7 1.1 L-2.6 1.5 L-1.7 -0.1 L-2.9 -1.9 L-0.8 -1.3 Z';
const palmate = (x: number, y: number, size: number, rotate: number, color: string) =>
  place(x, y, size, fill(PALMATE, color), rotate);

const MAPLE_LEAF: LeafKind = {
  span: 5.6,
  size: 0.84,
  stamp: (x, y, size, angle, color) => palmate(x, y, size, angle, color),
};
/** Reds for the back, middle and front of the maple's canopy. */
const MAPLE_BACK: Foliage = { shade: '#873F2B', body: '#A4573C', light: '#B5654A' };
const MAPLE_MID: Foliage = { shade: '#A4573C', body: '#BF6E4C', light: '#CF8459' };
const MAPLE_FRONT: Foliage = { shade: '#B5654A', body: '#CB7C55', light: '#DD9B6D' };

const MAPLE_WOOD =
  limb([[40, 48, 3.6], [39.4, 43.4, 3], [40.4, 39, 2.6], [40, 35.4, 2.2], [40.4, 31, 1.5]]) +
  limb([[40, 38, 1.9], [36.4, 34, 1.4], [32.4, 31.4, 0.9]]) +
  limb([[40.4, 36.8, 1.8], [44.4, 33, 1.3], [48.4, 30.6, 0.8]]);

const MAPLE: Drawing[] = [
  {
    back:
      MOUND +
      fill('M40.6 42.6 C42.6 38.4 46.6 36.4 50 37 C49 40.6 45.2 43 40.6 42.6 Z', MAPLE_RED.light) +
      fill('M40.6 42.6 C44 41.2 47.4 39.2 50 37 C49 40.6 45.2 43 40.6 42.6 Z', MAPLE_RED.body) +
      seed(39.6, 43, 2.1, 1.6, -20, ['#9A6A48', '#74492F']),
  },
  {
    back:
      limb([[40, 47.5, 1.3], [39.8, 44, 1.1], [40.1, 40.4, 0.8]], ['#8A5F45', '#6B4733']) +
      palmate(36.2, 38.6, 1.25, -28, MAPLE_RED.body) +
      palmate(44, 37.4, 1.25, 26, MAPLE_RED.light),
  },
  {
    back:
      limb([[40, 48, 2.6], [39.6, 43.6, 2.2], [40.3, 39.4, 1.8], [40, 36, 1.3]]) +
      limb([[40.2, 40.6, 1.2], [37.4, 37.6, 0.8]]) +
      clumps(MAPLE_LEAF, [
        [40, 29.6, 13, 9, MAPLE_MID, 41],
        [36, 33.6, 10, 7, MAPLE_FRONT, 42],
        [44.4, 33.4, 10, 7, MAPLE_FRONT, 43],
      ]),
  },
  {
    back:
      MAPLE_WOOD +
      clumps(MAPLE_LEAF, [
        [33.6, 26.4, 14, 9.6, MAPLE_BACK, 51],
        [47, 26, 14, 9.6, MAPLE_BACK, 52],
        [40, 21.6, 15, 10, MAPLE_MID, 53],
        [35.4, 30.6, 12.6, 8, MAPLE_FRONT, 54],
        [45.6, 31, 12, 8, MAPLE_FRONT, 55],
      ]),
  },
  {
    back:
      MAPLE_WOOD +
      clumps(MAPLE_LEAF, [
        [31.6, 23.6, 17, 11, MAPLE_BACK, 61],
        [49, 23, 17, 11, MAPLE_BACK, 62],
        [26.4, 28.6, 14, 9, MAPLE_MID, 63],
        [54, 28.6, 14, 9, MAPLE_MID, 64],
        [40, 17.6, 19, 11.6, MAPLE_MID, 65],
        [35, 28, 15, 9.6, MAPLE_FRONT, 66],
        [46.6, 29, 14.6, 9, MAPLE_FRONT, 67],
      ]),
    front: palmate(63.6, 66.6, 0.95, 72, MAPLE_RED.body) + palmate(18.6, 67.4, 0.7, -40, MAPLE_RED.light),
  },
];

// -------------------------------------------------------------- wisteria
const LILAC = { deep: '#8E78BE', mid: '#A994D2', light: '#C8B9E6', pale: '#E0D6F2' };

/** A hanging cluster of flowers from (x, top), tapering to a point `length` below. */
function raceme(x: number, top: number, length: number, open = true): string {
  const rows = Math.max(3, Math.round(length / 2));
  let out = line(`M${x} ${r(top - 1.6)} V${r(top + 0.4)}`, '#6E8F4A', 0.6);
  for (let i = 0; i < rows; i++) {
    const t = i / (rows - 1);
    const size = (open ? 1.65 : 1.2) * (1 - t * 0.6);
    const y = top + t * length;
    if (i === rows - 1) {
      out += `<circle cx="${x}" cy="${r(y)}" r="${r(size * 0.8)}" fill="${open ? LILAC.pale : LILAC.light}"/>`;
    } else {
      // Two florets a row: the left catches the light.
      out +=
        `<circle cx="${r(x + size * 0.56)}" cy="${r(y + size * 0.22)}" r="${r(size)}" fill="${open ? LILAC.deep : LILAC.mid}"/>` +
        `<circle cx="${r(x - size * 0.5)}" cy="${r(y)}" r="${r(size)}" fill="${open ? LILAC.mid : LILAC.light}"/>` +
        `<circle cx="${r(x - size * 0.74)}" cy="${r(y - size * 0.3)}" r="${r(size * 0.46)}" fill="${open ? LILAC.light : LILAC.pale}"/>`;
    }
  }
  return out;
}

/** A pinnate leaf: paired leaflets along a stalk from `from` to `to`. */
function pinnate(from: Point, to: Point, pairs: number, tone: Tone): string {
  const dx = to[0] - from[0];
  const dy = to[1] - from[1];
  const length = Math.hypot(dx, dy) || 1;
  const nx = dy / length;
  const ny = -dx / length;
  let out = line(`M${pt(from)} L${pt(to)}`, tone[1], 0.5);
  for (let i = 1; i <= pairs; i++) {
    const at = lerp(from, to, i / (pairs + 0.6));
    const reach = 3.3 - i * 0.22;
    for (const side of [1, -1]) {
      out += leaf(at, [at[0] + nx * reach * side + (dx / length) * 1.5, at[1] + ny * reach * side + (dy / length) * 1.5], 1, tone);
    }
  }
  return out + leaf(lerp(from, to, 0.86), [to[0] + (dx / length) * 2, to[1] + (dy / length) * 2], 1, tone);
}

const WISTERIA_TRUNK =
  limb([[40, 48, 4.6], [37.6, 43.6, 4], [40.6, 39.2, 3.5], [42.4, 34.8, 3], [40.2, 30.4, 2.6], [39.6, 26, 2.2], [41.6, 22.4, 1.7]]) +
  limb([[40.2, 29.4, 2.1], [34.4, 25.8, 1.6], [28, 24.8, 1.2], [23, 26.4, 0.8]]) +
  limb([[41.2, 23.4, 1.8], [47, 20.6, 1.4], [53.4, 21, 1], [58, 23.6, 0.7]]);
const LEAFLET = 'M0 -3 C1.15 -1.6 1.25 0.8 0 3 C-1.25 0.8 -1.15 -1.6 0 -3 Z';
const WISTERIA_LEAF: LeafKind = {
  span: 2.5,
  size: 0.8,
  stamp: (x, y, size, angle, color) => place(x, y, size, fill(LEAFLET, color), angle),
};
/** The canopy along the two arching branches: leaf sprays at the ends, clumps over them. */
const wisteriaLeaves = () =>
  pinnate([25, 22.4], [17.6, 20.4], 3, LEAF) +
  pinnate([31.4, 17.6], [27.6, 10.4], 3, LEAF) +
  pinnate([41.4, 13.4], [42, 6], 3, LEAF_FRESH) +
  pinnate([51, 15.4], [55.6, 9], 3, LEAF) +
  pinnate([57.4, 20.4], [64, 19], 3, LEAF) +
  clumps(WISTERIA_LEAF, [
    [30.6, 21.6, 16, 8.6, GREEN_BACK, 71],
    [50, 19.6, 16, 8.6, GREEN_BACK, 72],
    [24.6, 23.4, 10.6, 6.6, GREEN_MID, 73],
    [56.8, 21.6, 10.6, 6.6, GREEN_MID, 74],
    [41, 16.6, 17, 9.6, GREEN_MID, 75],
    [35, 20.6, 12.6, 7.4, GREEN_FRONT, 76],
    [46.6, 18.8, 12.6, 7.4, GREEN_FRONT, 77],
  ]);

const WISTERIA: Drawing[] = [
  { back: MOUND + seed(40.2, 42.6, 3, 1.9, -12, ['#8A7488', '#66536A']) },
  {
    back:
      line('M40 47.5 C38.6 45 41.8 43.2 40.4 40.4 C39.6 38.6 40.8 36.8 42.6 36.6', '#6E8F4A', 1.2) +
      pinnate([40.4, 41.4], [33.6, 38.6], 2, LEAF_FRESH) +
      leaf([40.8, 38.6], [46.4, 36.6], 1.2, LEAF_FRESH),
  },
  {
    back:
      limb([[40, 48, 2.8], [38.2, 44, 2.4], [40.6, 40, 2], [41.6, 36, 1.6], [40.6, 32.4, 1.2]]) +
      limb([[41, 37.4, 1.3], [36.6, 34.6, 0.9], [32.6, 34.4, 0.6]]) +
      pinnate([33.6, 34.2], [27.6, 31.4], 3, LEAF) +
      pinnate([38.4, 33.6], [35.4, 27], 3, LEAF_FRESH) +
      pinnate([40.8, 32.6], [43.6, 25.6], 3, LEAF_FRESH) +
      pinnate([41.6, 35.6], [48.6, 32.4], 3, LEAF),
  },
  {
    back:
      WISTERIA_TRUNK +
      raceme(25.4, 29, 5, false) +
      raceme(32, 28.4, 5.6, false) +
      raceme(48.4, 26.4, 5.6, false) +
      raceme(55.4, 27, 5, false) +
      wisteriaLeaves(),
  },
  {
    back:
      WISTERIA_TRUNK +
      raceme(24.4, 29, 9) +
      raceme(30.6, 28.6, 13.6) +
      raceme(36.4, 28.4, 7) +
      raceme(46.4, 27, 8.4) +
      raceme(52.4, 26.4, 13.6) +
      raceme(57.8, 27.4, 8) +
      wisteriaLeaves(),
    front:
      `<circle cx="62.4" cy="67.6" r="1.1" fill="${LILAC.mid}"/>` +
      `<circle cx="65.6" cy="68.3" r="0.8" fill="${LILAC.light}"/>` +
      `<circle cx="19.4" cy="68" r="0.9" fill="${LILAC.light}"/>`,
  },
];

// ------------------------------------------------------------------- fern
const FROND_BACK: Tone = ['#557339', '#46602F'];
const FROND: Tone = ['#6E8F4A', '#5A7A3F'];
const FROND_FRESH: Tone = ['#8DAE62', '#6E8F4A'];

/** A point on a quadratic curve, and the direction it's heading there. */
function onCurve(a: Point, c: Point, b: Point, t: number): [Point, Point] {
  const u = 1 - t;
  const p: Point = [u * u * a[0] + 2 * u * t * c[0] + t * t * b[0], u * u * a[1] + 2 * u * t * c[1] + t * t * b[1]];
  const dx = 2 * u * (c[0] - a[0]) + 2 * t * (b[0] - c[0]);
  const dy = 2 * u * (c[1] - a[1]) + 2 * t * (b[1] - c[1]);
  const length = Math.hypot(dx, dy) || 1;
  return [p, [dx / length, dy / length]];
}

/**
 * A fern frond arching from `base` through `bend` to `tip`: a tapering stalk
 * with paired leaflets, longest about a third of the way along. Each leaflet
 * is split along its midrib, so the frond takes the light like the rest.
 */
function frond(base: Point, bend: Point, tip: Point, reach: number, tone: Tone, pairs = 9): string {
  const spine: Spine = [0, 0.25, 0.5, 0.75, 1].map((t) => {
    const [p] = onCurve(base, bend, tip, t);
    return [p[0], p[1], 0.8 - t * 0.55];
  });
  let out = limb(spine, [tone[1], tone[1]], false);
  for (let i = 1; i <= pairs; i++) {
    const t = 0.12 + (0.84 * i) / pairs;
    const [p, d] = onCurve(base, bend, tip, t);
    const length = reach * Math.min(1, t * 3.2) * (1 - t * 0.74);
    for (const side of [1, -1]) {
      const end: Point = [
        p[0] + (-d[1] * side * 0.88 + d[0] * 0.48) * length,
        p[1] + (d[0] * side * 0.88 + d[1] * 0.48) * length,
      ];
      out += leaf(p, end, length * 0.3, tone);
    }
  }
  const [last] = onCurve(base, bend, tip, 0.97);
  const [end, d] = onCurve(base, bend, tip, 1);
  return out + leaf(last, [end[0] + d[0] * reach * 0.4, end[1] + d[1] * reach * 0.4], reach * 0.09, tone);
}

/** A fiddlehead: a frond still curled up tight at the end of its stalk. */
function fiddlehead(stalk: Spine, [cx, cy]: Point, size: number): string {
  return (
    limb(stalk, FROND_FRESH) +
    `<circle cx="${r(cx)}" cy="${r(cy)}" r="${r(size)}" fill="${FROND[1]}"/>` +
    `<circle cx="${r(cx - size * 0.2)}" cy="${r(cy - size * 0.22)}" r="${r(size * 0.7)}" fill="${FROND_FRESH[0]}"/>` +
    `<circle cx="${r(cx + size * 0.14)}" cy="${r(cy + size * 0.08)}" r="${r(size * 0.32)}" fill="${FROND[1]}"/>`
  );
}

const FERN: Drawing[] = [
  { back: MOUND + fiddlehead([[40, 45.4, 1], [40.2, 43.4, 0.9], [40.9, 42.2, 0.8]], [41.9, 41.6], 1.3) },
  {
    back:
      frond([39.8, 47.2], [34.2, 40.4], [30.4, 39.4], 2.6, FROND_FRESH, 5) +
      fiddlehead([[39.6, 47.5, 1.3], [39.2, 43, 1.2], [39.8, 38.8, 1], [41, 36.6, 0.9]], [42.2, 36], 1.9) +
      fiddlehead([[41, 47.5, 1], [41.6, 45, 0.9], [42.6, 43.4, 0.8]], [43.6, 42.9], 1.2),
  },
  {
    back:
      frond([39.6, 47.2], [30.6, 37.6], [24.4, 38.8], 4, FROND, 7) +
      frond([40.6, 47.2], [49.4, 35.8], [55.6, 37.6], 4, FROND, 7) +
      frond([40, 47.2], [38.6, 35.6], [41.6, 28.2], 3.6, FROND_FRESH, 7) +
      fiddlehead([[41.2, 47.5, 1], [42.2, 44.6, 0.9], [43.6, 43, 0.8]], [44.6, 42.4], 1.3),
  },
  {
    back:
      frond([39.4, 47], [26, 31], [18.6, 37.4], 5, FROND_BACK, 9) +
      frond([40.6, 47], [54, 31], [61.4, 37.4], 5, FROND_BACK, 9) +
      frond([39.6, 47.2], [33, 30], [31.4, 21.6], 4.4, FROND, 8) +
      frond([40.4, 47.2], [47.6, 30.4], [50, 22], 4.4, FROND, 8) +
      frond([40, 47.4], [41, 36], [38.4, 27.6], 3.8, FROND_FRESH, 7) +
      fiddlehead([[38.8, 47.5, 1], [37.8, 44.4, 0.9], [36.6, 42.6, 0.8]], [35.6, 42.2], 1.3),
  },
  {
    back:
      frond([39, 47], [22, 28.6], [12.6, 39.6], 6.2, FROND_BACK, 10) +
      frond([41, 47], [58, 28.6], [67.4, 39.6], 6.2, FROND_BACK, 10) +
      frond([39.6, 47], [31.4, 23], [29.6, 12.4], 5.4, FROND_BACK, 10) +
      frond([40.4, 47], [48.6, 23], [50.4, 12.4], 5.4, FROND_BACK, 10) +
      frond([39.4, 47], [26, 32.6], [17, 30], 5.4, FROND, 9) +
      frond([40.6, 47], [54, 32.6], [63, 30], 5.4, FROND, 9) +
      frond([40, 47.2], [37.6, 30], [41, 19], 5, FROND_FRESH, 9) +
      frond([40.2, 47.4], [47, 38.4], [56, 44.6], 4.4, FROND_FRESH, 8) +
      fiddlehead([[38.6, 47.5, 1], [37.6, 44, 0.9], [36.4, 42, 0.8]], [35.4, 41.6], 1.3),
  },
];

// -------------------------------------------------------------- hydrangea
const BROAD = 'M0 -3 C2.2 -1.8 2.3 1 0 3 C-2.3 1 -2.2 -1.8 0 -3 Z';
const HYDRANGEA_LEAF: LeafKind = {
  span: 4.2,
  size: 0.95,
  stamp: (x, y, size, angle, color) => place(x, y, size, fill(BROAD, color), angle),
  lit: (x, y, size, angle, c) => {
    const radians = ((angle - 90) * Math.PI) / 180;
    const reach = 2.3 * size;
    const along: Point = [Math.cos(radians) * reach, Math.sin(radians) * reach];
    return leaf([x - along[0], y - along[1]], [x + along[0], y + along[1]], 2 * size, [c.light, c.body]);
  },
};

/** Four rounded petals: one floret of a hydrangea head. */
const petals4 = (color: string) =>
  [0, 90, 180, 270]
    .map((a) => `<ellipse cx="0" cy="-0.95" rx="0.8" ry="0.95" transform="rotate(${a})" fill="${color}"/>`)
    .join('');
/** Florets, packed into a round head the way leaves are packed into a clump. */
const FLORET: LeafKind = {
  span: 2.6,
  size: 0.8,
  stamp: (x, y, size, angle, color) => place(x, y, size, petals4(color), angle + 45),
  lit: (x, y, size, angle, c) => place(x, y, size, petals4(c.light) + `<circle r="0.32" fill="${c.shade}"/>`, angle + 45),
};
const HEAD_BLUE: Foliage = { shade: '#5E70AA', body: '#7D90C8', light: '#A9B9E4' };
const HEAD_PALE: Foliage = { shade: '#7486BE', body: '#97A8D8', light: '#C2CEEE' };
const HEAD_LILAC: Foliage = { shade: '#866AAA', body: '#A388C4', light: '#C8B4E0' };
const HEAD_BUD: Foliage = { shade: '#A6B884', body: '#C2D1A0', light: '#DCE6C2' };
const HYDRANGEA_STEM: Tone = ['#7D8A55', '#5F6B3E'];

const HYDRANGEA_STEMS =
  limb([[39.4, 48, 2], [37.8, 42, 1.7], [35.4, 36, 1.3], [33.6, 31, 0.9]], HYDRANGEA_STEM) +
  limb([[40.6, 48, 2], [42.4, 42, 1.7], [45.4, 35.4, 1.3], [48, 29.4, 0.9]], HYDRANGEA_STEM);

const HYDRANGEA: Drawing[] = [
  { back: MOUND + seed(40.2, 42.4, 2, 1.5, -6, ['#A08062', '#7A5C42']) },
  {
    back:
      limb([[40, 47.5, 1.4], [39.8, 43.6, 1.2], [40.1, 39.6, 0.9]], HYDRANGEA_STEM) +
      leaf([40, 43.4], [33.2, 40.8], 2.5, LEAF, 0.3) +
      leaf([40.1, 43], [46.8, 40.4], 2.5, LEAF, -0.3) +
      leaf([40, 40.2], [36.2, 35.6], 1.7, LEAF_FRESH, 0.2) +
      leaf([40.1, 40], [44.2, 35.4], 1.7, LEAF_FRESH, -0.2),
  },
  {
    back:
      limb([[39.6, 48, 1.7], [38.4, 43, 1.4], [37, 38.6, 1]], HYDRANGEA_STEM) +
      limb([[40.4, 48, 1.7], [41.8, 43, 1.4], [43.6, 38.2, 1]], HYDRANGEA_STEM) +
      clumps(HYDRANGEA_LEAF, [
        [36, 37.6, 12, 9.4, GREEN_BACK, 81],
        [45, 36.8, 12, 9.4, GREEN_MID, 82],
        [40.2, 41.6, 14, 7.6, GREEN_FRONT, 83],
      ]),
  },
  {
    back:
      HYDRANGEA_STEMS +
      clumps(HYDRANGEA_LEAF, [
        [31.6, 36.6, 15, 11, GREEN_BACK, 84],
        [48.8, 35.6, 15, 11, GREEN_BACK, 85],
        [40, 33, 14, 10, GREEN_MID, 86],
      ]) +
      clumps(FLORET, [
        [33.8, 29.6, 8.6, 7.6, HEAD_BUD, 87],
        [47.4, 28, 9, 8, HEAD_BUD, 88],
      ]) +
      clumps(HYDRANGEA_LEAF, [
        [34.6, 41, 14, 8, GREEN_FRONT, 89],
        [46.2, 41, 13, 7.6, GREEN_FRONT, 90],
      ]),
  },
  {
    back:
      HYDRANGEA_STEMS +
      clumps(HYDRANGEA_LEAF, [
        [28.6, 37, 17, 12, GREEN_BACK, 91],
        [51.6, 36, 17, 12, GREEN_BACK, 92],
        [40, 35, 16, 11, GREEN_MID, 93],
      ]) +
      clumps(FLORET, [
        [31.2, 27.6, 15.4, 13.4, HEAD_BLUE, 94],
        [49.6, 25.4, 14.6, 12.8, HEAD_LILAC, 95],
        [40.6, 33.4, 13.4, 11.6, HEAD_PALE, 96],
      ]) +
      clumps(HYDRANGEA_LEAF, [
        [32.4, 41.6, 15, 8.4, GREEN_FRONT, 97],
        [48.2, 41.6, 14, 8, GREEN_FRONT, 98],
      ]),
  },
];

// ----------------------------------------------------------------- sakura
/** Five petals around a centre, pointing up at angle 0. */
const petals5 = (color: string, distance = 0.95, rx = 0.7, ry = 0.92) =>
  [0, 72, 144, 216, 288]
    .map((a) => `<ellipse cx="0" cy="${-distance}" rx="${rx}" ry="${ry}" transform="rotate(${a})" fill="${color}"/>`)
    .join('');

const BLOSSOM: LeafKind = {
  span: 2.9,
  size: 0.78,
  stamp: (x, y, size, angle, color) => place(x, y, size, petals5(color), angle),
  lit: (x, y, size, angle, c) => place(x, y, size, petals5(c.light) + `<circle r="0.34" fill="#C9667E"/>`, angle),
};
const SAKURA_BACK: Foliage = { shade: '#C07A8D', body: '#D394A5', light: '#E0AABA' };
const SAKURA_MID: Foliage = { shade: '#D394A5', body: '#E5AFBE', light: '#F0C7D2' };
const SAKURA_FRONT: Foliage = { shade: '#DFA4B4', body: '#EFC2CE', light: '#F8DDE4' };
const BARK: Tone = ['#5E4238', '#45302A'];
const PINK_BUD: Tone = ['#F0C2CE', '#D98FA4'];

/** A loose petal, drifting or fallen. */
const petal = (x: number, y: number, turn: number, color: string) =>
  `<ellipse cx="${x}" cy="${y}" rx="1" ry="0.62" transform="rotate(${turn} ${x} ${y})" fill="${color}"/>`;

const SAKURA_WOOD =
  limb([[40, 48, 5.4], [41.8, 43.6, 4.8], [39.6, 39.4, 4.2], [37.4, 35.4, 3.6], [38.6, 31.2, 3], [41, 28, 2.4], [42, 25.6, 1.8]], BARK) +
  limb([[38.4, 37, 2.6], [33, 34.6, 2], [26.6, 33.6, 1.5], [21.6, 34.6, 1], [18.6, 36.4, 0.7]], BARK) +
  limb([[39.6, 31.4, 2.2], [45.4, 29.2, 1.7], [51.6, 28.8, 1.2], [56.6, 30.4, 0.8]], BARK);

const SAKURA: Drawing[] = [
  { back: MOUND + seed(40.2, 42.6, 2.2, 1.9, -10, ['#E8D6B6', '#C9AE88']) },
  {
    back:
      limb([[40, 47.5, 1.3], [39.7, 44, 1.1], [40.2, 40.6, 0.8]], ['#7A4E40', '#5E3A30']) +
      leaf([40, 42.6], [34, 39.8], 2, LEAF_FRESH, 0.3) +
      leaf([40.1, 41.6], [46, 38.6], 2, LEAF_FRESH, -0.3) +
      leaf([40.2, 41], [40.5, 38.2], 0.9, PINK_BUD),
  },
  {
    back:
      limb([[40, 48, 3], [40.8, 44, 2.6], [39.6, 40, 2.2], [38.8, 36.4, 1.7], [39.8, 33.4, 1.2]], BARK) +
      limb([[39.4, 39, 1.4], [35, 37, 1], [31.6, 37, 0.7]], BARK) +
      clumps(CAMELLIA_LEAF, [
        [32.4, 35.6, 8.4, 6, GREEN_MID, 111],
        [40.6, 30.6, 9.4, 6.8, GREEN_FRONT, 112],
      ]) +
      leaf([33.6, 36.4], [36.4, 33.2], 0.9, PINK_BUD) +
      leaf([41.4, 31.4], [44.4, 28.6], 0.9, PINK_BUD) +
      leaf([39.4, 31.6], [37.6, 28.2], 0.8, PINK_BUD),
  },
  {
    back:
      SAKURA_WOOD +
      clumps(BLOSSOM, [
        [32.4, 30.6, 13, 8.4, SAKURA_BACK, 121],
        [47.6, 27, 13, 8.4, SAKURA_BACK, 122],
        [22.4, 33, 9.6, 6.4, SAKURA_MID, 123],
        [40.6, 24.4, 12, 8, SAKURA_MID, 124],
        [54.4, 29.8, 9.6, 6.4, SAKURA_FRONT, 125],
        [29.6, 34.6, 7, 4.4, SAKURA_FRONT, 126],
      ]),
  },
  {
    back:
      SAKURA_WOOD +
      clumps(BLOSSOM, [
        [30, 28, 17, 11, SAKURA_BACK, 131],
        [48, 24.6, 17, 11, SAKURA_BACK, 132],
        [39.6, 20, 16, 10.6, SAKURA_BACK, 133],
        [20.4, 32, 12, 8, SAKURA_MID, 134],
        [57.4, 28.6, 12, 8, SAKURA_MID, 135],
        [33.6, 22.6, 13, 9, SAKURA_MID, 136],
        [46, 19.4, 13, 9, SAKURA_MID, 137],
        [27, 32.2, 13, 8.4, SAKURA_FRONT, 138],
        [41.4, 27, 13, 8.4, SAKURA_FRONT, 139],
        [52.6, 29.8, 12, 8, SAKURA_FRONT, 140],
      ]),
    front:
      petal(61.6, 67.6, 20, SAKURA_MID.body) +
      petal(65.4, 68.6, -30, SAKURA_FRONT.body) +
      petal(18.4, 68.2, 40, SAKURA_FRONT.body) +
      petal(63.4, 53.4, -12, SAKURA_MID.light),
  },
];

// ----------------------------------------------------------------- ginkgo
/** A ginkgo leaf: a fan with a notch in its top edge, on a stalk at (0, 3). */
const FAN =
  'M0 3 L-0.3 0.4 C-1.6 0 -3 -1.1 -3 -2.4 C-2 -3.4 -0.6 -3.3 -0.18 -2.8 L0 -2 L0.18 -2.8 C0.6 -3.3 2 -3.4 3 -2.4 C3 -1.1 1.6 0 0.3 0.4 Z';
/** The right half of the fan, away from the light. */
const FAN_RIGHT = 'M0 3 L0 -2 L0.18 -2.8 C0.6 -3.3 2 -3.4 3 -2.4 C3 -1.1 1.6 0 0.3 0.4 Z';
const fan = (x: number, y: number, size: number, angle: number, [light, dark]: Tone) =>
  place(x, y, size, fill(FAN, light) + fill(FAN_RIGHT, dark), angle);
const FAN_KIND: LeafKind = {
  span: 4.4,
  size: 0.72,
  stamp: (x, y, size, angle, color) => place(x, y, size, fill(FAN, color), angle),
  lit: (x, y, size, angle, c) => fan(x, y, size, angle, [c.light, c.body]),
};
const GINKGO_BACK: Foliage = { shade: '#4F6B36', body: '#5F7F43', light: '#76985A' };
const GINKGO_GREEN: Foliage = { shade: '#5F7F43', body: '#7A9A55', light: '#9AB872' };
const TURNING: Foliage = { shade: '#8C9645', body: '#A8B054', light: '#C5C672' };
const GOLD_BACK: Foliage = { shade: '#A47C2C', body: '#BD9236', light: '#CDA645' };
const GOLD_MID: Foliage = { shade: '#BD9236', body: '#D4AA44', light: '#E5C260' };
const GOLD_FRONT: Foliage = { shade: '#CEA33E', body: '#E2BD55', light: '#F0D27A' };
const GINKGO_BARK: Tone = ['#7D6A55', '#5E4F40'];

// Ginkgo branches grow in tiers, with the trunk showing between them.
const GINKGO_WOOD =
  limb([[40, 48, 4.4], [39.6, 42, 3.8], [40.2, 35, 3.2], [39.8, 27, 2.6], [40.2, 19, 2], [40, 9, 1.2]], GINKGO_BARK) +
  limb([[40, 37, 1.8], [34, 34.2, 1.3], [28.6, 33.2, 0.8]], GINKGO_BARK) +
  limb([[40.1, 36, 1.7], [46.4, 33.2, 1.2], [51.4, 32.4, 0.7]], GINKGO_BARK) +
  limb([[40, 26.6, 1.4], [35.4, 23.6, 1], [32, 22.6, 0.6]], GINKGO_BARK) +
  limb([[40.1, 25.4, 1.3], [44.8, 22.4, 0.9], [48, 21.6, 0.5]], GINKGO_BARK);

const GINKGO: Drawing[] = [
  { back: MOUND + seed(40.2, 42.4, 2.4, 2, -8, ['#EEDFA8', '#CDB478']) },
  {
    back:
      limb([[40, 47.5, 1.2], [39.8, 44, 1], [40.1, 40.4, 0.8]], ['#8A9A55', '#6E7E40']) +
      fan(36.8, 39.4, 0.95, -38, LEAF_FRESH) +
      fan(43.4, 38.6, 0.95, 34, LEAF),
  },
  {
    back:
      limb([[40, 48, 2.4], [39.8, 43, 2], [40.2, 37, 1.6], [40, 32, 1.1]], GINKGO_BARK) +
      clumps(FAN_KIND, [
        [40, 31, 10, 9, GINKGO_BACK, 141],
        [35.6, 37, 9, 7, GINKGO_GREEN, 142],
        [44.6, 36.2, 9, 7, GINKGO_GREEN, 143],
      ]),
  },
  {
    back:
      GINKGO_WOOD +
      clumps(FAN_KIND, [
        [32.4, 32, 12, 8, GINKGO_BACK, 151],
        [48, 31, 12, 8, GINKGO_BACK, 152],
        [35.4, 21.6, 10, 7, GINKGO_BACK, 153],
        [45, 20.6, 10, 7, GINKGO_GREEN, 154],
        [40, 11.6, 8, 7, GINKGO_GREEN, 155],
        [37, 34.2, 11, 6.4, GINKGO_GREEN, 156],
        [45.4, 33.4, 11, 6.4, TURNING, 157],
        [40.4, 23.2, 10, 6, TURNING, 158],
      ]),
  },
  {
    back:
      GINKGO_WOOD +
      clumps(FAN_KIND, [
        [30.6, 32, 15, 9.4, GOLD_BACK, 161],
        [49.6, 31, 15, 9.4, GOLD_BACK, 162],
        [34, 21.4, 13, 8.4, GOLD_BACK, 163],
        [46.2, 20.4, 13, 8.4, GOLD_BACK, 164],
        [40, 10.4, 11, 9, GOLD_MID, 165],
        [26.6, 35, 10, 6.4, GOLD_MID, 166],
        [53.6, 34, 10, 6.4, GOLD_MID, 167],
        [36, 34.8, 13, 7.6, GOLD_FRONT, 168],
        [45.6, 34, 13, 7.6, GOLD_MID, 169],
        [40.2, 23.4, 12, 7, GOLD_FRONT, 170],
        [40, 13.8, 8, 6, GOLD_FRONT, 171],
      ]),
    front: fan(62.8, 67.6, 0.5, 70, ['#E6C25A', '#C9A040']) + fan(17.8, 68.2, 0.42, -60, ['#DDB54E', '#BD9236']),
  },
];

// -------------------------------------------------------------- withered
const DRY_DEEP: Tone = ['#B5A188', '#957F67'];

/**
 * The withered plant, shared by every species: a dried stem bowed over, its
 * leaves hanging, in a pot with a chip knocked out of its rim.
 */
const WITHERED: Drawing = {
  back:
    limb(
      [[40, 48, 2.2], [40.4, 42.2, 1.9], [41.6, 36.6, 1.6], [44.4, 32.9, 1.3], [48, 32.2, 1.1], [50.4, 34.6, 0.9], [51, 37.4, 0.7]],
      ['#A38A6E', '#86705A'],
    ) +
    // The head of the plant, hanging from the end of the bowed stem.
    leaf([51, 37], [50.2, 43.2], 1.6, DRY_DEEP, -0.3) +
    leaf([40.4, 43.2], [32.4, 47.4], 2.3, DRY, 0.6) +
    leaf([41.8, 37.6], [46.8, 42.4], 1.9, DRY_DEEP, -0.4) +
    leaf([43.2, 34.4], [40.8, 29.2], 1.5, DRY, 0.6),
  front:
    // The chip that broke off the rim, lying at the foot of the pot.
    fill('M21.2 69.7 L22.6 67.9 L24.9 68.5 L25.4 69.7 Z', POT.grey.rim) +
    fill('M22.6 67.9 L24.9 68.5 L25.4 69.7 L23.7 69.7 Z', POT.grey.rimShade) +
    leaf([60.4, 67.6], [67.8, 66.2], 1.5, DRY, 0.3),
};

// In the seed picker's order, which follows the unlocks (`Species::ALL`).
const STAGES: Record<SpeciesId, Drawing[]> = {
  sprout: SPROUT,
  succulent: SUCCULENT,
  camellia: CAMELLIA,
  pine: PINE,
  fern: FERN,
  bamboo: BAMBOO,
  hydrangea: HYDRANGEA,
  maple: MAPLE,
  sakura: SAKURA,
  wisteria: WISTERIA,
  ginkgo: GINKGO,
};

export const SPECIES_IDS = Object.keys(STAGES) as SpeciesId[];

/**
 * SVG markup for a potted plant, to place inside an 80 x 72 viewBox, as the
 * three layers it is drawn in. `stage` runs from 1 (seed) to 5 (fully grown).
 * `detailed` adds the ground shadow and the shine on the pot's rim, for
 * larger sizes.
 */
export function plantLayers(
  species: SpeciesId,
  stage: number,
  { withered = false, detailed = true } = {},
): { back: string; pot: string; front: string } {
  const stages = STAGES[species] ?? SPROUT;
  const drawing = withered ? WITHERED : stages[Math.min(Math.max(stage, 1), 5) - 1];
  return { back: drawing.back, pot: pot(withered, detailed), front: drawing.front ?? '' };
}

export function plantMarkup(
  species: SpeciesId,
  stage: number,
  options: { withered?: boolean; detailed?: boolean } = {},
): string {
  const { back, pot, front } = plantLayers(species, stage, options);
  return back + pot + front;
}
