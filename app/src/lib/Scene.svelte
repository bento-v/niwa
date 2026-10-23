<script lang="ts">
  import { cubicOut } from 'svelte/easing';
  import { fade } from 'svelte/transition';
  import { reduced, travel } from './motion.svelte';
  import { plantLayers, type SpeciesId } from './plants';

  interface Props {
    species: SpeciesId;
    stage?: number;
    withered?: boolean;
    /** The shorter scene sits on the Focus screen; the taller on the result screens. */
    tall?: boolean;
    label: string;
    /** A barely-there sway, as if in a draught: for the plant that's growing now. */
    alive?: boolean;
  }

  let { species, stage = 5, withered = false, tall = false, label, alive = false }: Props = $props();

  const uid = $props.id();

  // The room is drawn as wide as the card allows; the stand and the plant
  // keep their size and stay in the middle. 348 is the card at its smallest.
  const DRAWN_WIDTH = 348;
  let width = $state(DRAWN_WIDTH);
  const centre = $derived(Math.max(0, (width - DRAWN_WIDTH) / 2));

  // The two layouts from the mockup: where the floor starts, and where the
  // stand and the plant sit on it.
  const layout = $derived(
    tall
      ? {
          height: 300,
          floor: 238,
          board: 272,
          shadow: { cy: 278, rx: 94, ry: 5 },
          legs: { y: 268, width: 14, height: 10, left: 96, right: 238 },
          base: { x: 86, y: 261, width: 176, height: 8 },
          top: { x: 90, y: 256, width: 168, height: 6 },
          shine: { x: 95, y: 257, width: 74, height: 1.4 },
          plant: 'translate(70,77) scale(2.6)',
        }
      : {
          height: 236,
          floor: 188,
          board: 214,
          shadow: { cy: 220, rx: 80, ry: 4 },
          legs: { y: 212, width: 12, height: 8, left: 108, right: 228 },
          base: { x: 100, y: 206, width: 148, height: 7 },
          top: { x: 104, y: 202, width: 140, height: 5 },
          shine: { x: 108, y: 202.8, width: 62, height: 1.2 },
          plant: 'translate(86,51) scale(2.2)',
        },
  );

  const colors = $derived(
    withered
      ? {
          wall: '#E9E3D9',
          weave: '#DCD3C5',
          rail: '#D4C8B6',
          floor: '#DFD7CA',
          edge: '#D2C7B6',
          board: '#D6CCBD',
          wood: '#655646',
          woodTop: '#7A6A58',
          woodShine: '#8E7E6C',
        }
      : {
          wall: '#EFE7D9',
          weave: '#E2D5C0',
          rail: '#DCCCB1',
          floor: '#E6DBC9',
          edge: '#D9CBB3',
          board: '#DED2BE',
          wood: '#5E4430',
          woodTop: '#7A5B40',
          woodShine: '#9A7A5A',
        },
  );

  const layers = $derived(plantLayers(species, stage, { withered, detailed: true }));

  // The sway: a barely-there turn about the soil line, once every 11 s. It
  // moves the leaves well under a pixel a second, so it's redrawn four times
  // a second (each step under a fifth of a pixel at the leaf tips) rather
  // than as a CSS animation, which on SVG repaints the whole scene on every
  // frame of the monitor (240 a second on a fast one). Each redraw repaints
  // the whole scene, so it was cut from ten a second on Oct 17, 2026.
  const SWAY_DEGREES = 0.7;
  const SWAY_PERIOD_MS = 11_000;
  const SWAY_STEP_MS = 250;
  let sway = $state(0);
  $effect(() => {
    if (!alive || reduced()) {
      sway = 0;
      return;
    }
    const start = performance.now();
    const timer = setInterval(() => {
      sway = SWAY_DEGREES * Math.sin((2 * Math.PI * (performance.now() - start)) / SWAY_PERIOD_MS);
    }, SWAY_STEP_MS);
    return () => clearInterval(timer);
  });
  // About the soil line, in the drawing's own 80 x 72 units.
  const swaying = $derived(sway ? `rotate(${sway.toFixed(3)} 40 48)` : undefined);

  /** A new stage fades in while rising slightly from the pot, so growth reads as upward. */
  function sprout(_node: Element) {
    const from = 1 - travel(0.06);
    return {
      duration: 900,
      easing: cubicOut,
      css: (t: number) => `opacity: ${t}; transform: scale(${from + (1 - from) * t})`,
    };
  }
</script>

<div class="scene" style:height="{layout.height}px" style:background={colors.wall} bind:clientWidth={width}>
  <svg {width} height={layout.height} viewBox="0 0 {width} {layout.height}" role="img" aria-label={label}>
    <defs>
      <!-- Ajiro: woven bamboo. -->
      <pattern id="{uid}-ajiro" width="24" height="24" patternUnits="userSpaceOnUse">
        <rect width="24" height="24" fill={colors.wall} />
        <g fill="none" stroke={colors.weave} stroke-width="1">
          <path d="M0 0.5H24M0 12.5H24M0.5 0V24M12.5 0V24" />
          <path d="M0 4.5H12M0 8.5H12" />
          <path d="M16.5 0V12M20.5 0V12" />
          <path d="M4.5 12V24M8.5 12V24" />
          <path d="M12 16.5H24M12 20.5H24" />
        </g>
      </pattern>
    </defs>
    <rect {width} height={layout.floor} fill="url(#{uid}-ajiro)" />
    <rect y="22" {width} height="5" fill={colors.rail} />
    <rect x="14" width="6" height={layout.floor} fill={colors.rail} />
    <rect x={width - 20} width="6" height={layout.floor} fill={colors.rail} />
    <rect y={layout.floor} {width} height={layout.height - layout.floor} fill={colors.floor} />
    <rect y={layout.floor - 2} {width} height="3" fill={colors.edge} />
    <rect y={layout.board} {width} height="1" fill={colors.board} />

    <!-- The stand and the plant, in the middle of the room. -->
    <g transform="translate({centre} 0)">
      <ellipse cx="174" cy={layout.shadow.cy} rx={layout.shadow.rx} ry={layout.shadow.ry} fill="#3E3226" opacity="0.08" />
      <rect x={layout.legs.left} y={layout.legs.y} width={layout.legs.width} height={layout.legs.height} rx="1" fill={colors.wood} />
      <rect x={layout.legs.right} y={layout.legs.y} width={layout.legs.width} height={layout.legs.height} rx="1" fill={colors.wood} />
      <rect {...layout.base} rx="2" fill={colors.wood} />
      <rect {...layout.top} rx="2" fill={colors.woodTop} />
      <rect {...layout.shine} rx="0.7" fill={colors.woodShine} />
      {#if withered && tall}
        <path d="M282 288 C286 283 293 283 297 287 C292 291 286 291 282 288 Z" fill="#C4B197" />
      {/if}

      <g transform={layout.plant}>
        <!-- Each new stage fades in over the last; the pot stays put. Both
             layers sway together about the point where the stem meets the soil. -->
        {#key `${species}-${stage}-${withered}`}
          <g class="plant" in:sprout out:fade={{ duration: 500 }}>
            <g transform={swaying}>{@html layers.back}</g>
          </g>
        {/key}
        {@html layers.pot}
        {#key `${species}-${stage}-${withered}`}
          <g in:fade={{ duration: 900 }} out:fade={{ duration: 500 }}>
            <g transform={swaying}>{@html layers.front}</g>
          </g>
        {/key}
      </g>
    </g>
  </svg>
</div>

<style>
  .scene {
    position: relative;
    margin: 0 16px;
    border-radius: 18px;
    overflow: hidden;
    flex-shrink: 0;
    transition: background-color 0.25s ease;
  }
  svg {
    position: absolute;
    left: 0;
    top: 0;
  }
  /* Scale from where the stem meets the soil. */
  .plant {
    transform-box: fill-box;
    transform-origin: 50% 100%;
  }
</style>
