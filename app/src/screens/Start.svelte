<script lang="ts">
  import type { Overview, Schedule, SpeciesInfo } from '../lib/api';
  import { nextStart, whenText } from '../lib/schedule';
  import { duration } from '../lib/format';
  import Plant from '../lib/Plant.svelte';
  import { STAGE_NAMES, type SpeciesId } from '../lib/plants';
  import { dragScroll } from '../lib/dragscroll';
  import { reduced } from '../lib/motion.svelte';
  import { radioKeys } from '../lib/radiogroup';

  interface Props {
    overview: Overview;
    /** Why the last attempt to start failed, if it did. */
    error: string;
    onstart: (minutes: number, species: SpeciesId) => void;
    onblocks: () => void;
    schedules: Schedule[];
    onschedule: () => void;
  }

  let { overview, error, onstart, onblocks, schedules, onschedule }: Props = $props();

  const next = $derived(nextStart(schedules));

  const PRESETS = [25, 50, 90];
  const CUSTOM = PRESETS.length;
  const STEP = 5;

  // Remember the last choices between launches; they're a convenience only.
  function recall<T>(key: string, fallback: T): T {
    try {
      const saved = localStorage.getItem(key);
      return saved === null ? fallback : (JSON.parse(saved) as T);
    } catch {
      return fallback;
    }
  }
  function remember(key: string, value: unknown) {
    try {
      localStorage.setItem(key, JSON.stringify(value));
    } catch {
      // Not being able to remember is fine.
    }
  }

  let choice = $state(recall('niwa.duration', 1));
  let custom = $state(recall('niwa.custom', 60));
  let wanted = $state<SpeciesId>(recall('niwa.seed', 'pine'));

  const minutes = $derived(choice === CUSTOM ? custom : PRESETS[choice]);
  const usable = (species: SpeciesInfo) => species.unlocked && minutes >= species.minMinutes;

  // The seed the user asked for, or the rarest one this session can grow.
  const picked = $derived(
    overview.species.find((s) => s.id === wanted && usable(s)) ??
      overview.species.findLast(usable) ??
      overview.species[0],
  );

  $effect(() => {
    remember('niwa.duration', choice);
    remember('niwa.custom', custom);
    remember('niwa.seed', wanted);
  });

  function requirement(species: SpeciesInfo): string {
    if (!species.unlocked) return `${species.unlockHours} h total`;
    return species.minMinutes ? `${species.minMinutes}m+` : 'any';
  }
  function describe(species: SpeciesInfo): string {
    if (!species.unlocked) {
      return `${species.name}, unlocks at ${species.unlockHours} hours of focus`;
    }
    if (minutes < species.minMinutes) {
      return `${species.name}, needs ${species.minMinutes} minutes`;
    }
    return species.name;
  }

  function nudge(by: number) {
    custom = Math.min(overview.maxMinutes, Math.max(overview.minMinutes, custom + by));
  }

  let seeds: HTMLDivElement;
  // Bring the chosen seed into view: at once when the screen opens, gliding
  // after that. Dragging and the wheel are handled by `dragScroll`.
  let shown = false;
  $effect(() => {
    const seed = seeds.querySelector<HTMLElement>(`[data-seed="${picked.id}"]`);
    if (!seed) return;
    // Scroll the row only: scrollIntoView would also nudge the card itself.
    const left = seed.offsetLeft - 16;
    const right = seed.offsetLeft + seed.offsetWidth + 16 - seeds.clientWidth;
    const to = seeds.scrollLeft > left ? left : seeds.scrollLeft < right ? right : null;
    if (to !== null) seeds.scrollTo({ left: to, behavior: shown && !reduced() ? 'smooth' : 'instant' });
    shown = true;
  });
</script>

<div class="screen">
  <div class="title">
    <h1 class="display">Plant a seed</h1>
    <p>Longer sessions grow rarer plants.</p>
  </div>

  <div class="durations" role="radiogroup" aria-label="Duration" use:radioKeys>
    <div class="thumb" aria-hidden="true" style:transform="translateX(calc({choice * 100}% + {choice * 6}px))"></div>
    {#each PRESETS as preset, i}
      <button role="radio" aria-checked={choice === i} tabindex={choice === i ? 0 : -1} onclick={() => (choice = i)}>{preset} min</button>
    {/each}
    <button role="radio" aria-checked={choice === CUSTOM} tabindex={choice === CUSTOM ? 0 : -1} onclick={() => (choice = CUSTOM)}>Custom</button>
  </div>

  <div class="custom" class:open={choice === CUSTOM} inert={choice !== CUSTOM}>
    <div class="custom-row">
      <button class="step" aria-label="5 minutes less" disabled={custom <= overview.minMinutes} onclick={() => nudge(-STEP)}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M6 12h12" /></svg>
      </button>
      <input
        type="range"
        aria-label="Custom duration"
        aria-valuetext={duration(custom)}
        min={overview.minMinutes}
        max={overview.maxMinutes}
        step={STEP}
        bind:value={custom}
        style:--fill="{((custom - overview.minMinutes) / (overview.maxMinutes - overview.minMinutes)) * 100}%"
      />
      <button class="step" aria-label="5 minutes more" disabled={custom >= overview.maxMinutes} onclick={() => nudge(STEP)}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M6 12h12M12 6v12" /></svg>
      </button>
      <output>{duration(custom)}</output>
    </div>
  </div>

  <div class="seeds" role="radiogroup" aria-label="Seed" bind:this={seeds} use:radioKeys use:dragScroll>
    <div class="track">
    {#each overview.species as species (species.id)}
      {@const ok = usable(species)}
      <button
        class="seed"
        class:locked={!ok}
        role="radio"
        data-seed={species.id}
        aria-checked={picked.id === species.id}
        tabindex={picked.id === species.id ? 0 : -1}
        aria-disabled={!ok}
        aria-label={describe(species)}
        onclick={() => ok && (wanted = species.id)}
      >
        <Plant species={species.id} width={56} />
        <span class="name">
          {#if !ok}
            <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></svg>
          {/if}
          {species.name}
        </span>
        <span class="need">{requirement(species)}</span>
      </button>
    {/each}
    </div>
  </div>

  <div class="stages">
    <div class="eyebrow">Growth stages</div>
    {#key picked.id}
      <div class="strip">
        {#each STAGE_NAMES as name, i}
          <span class="stage" style:--i={i}>
            <Plant species={picked.id} stage={i + 1} width={52} label="Stage {i + 1}, {name}" />
          </span>
        {/each}
      </div>
    {/key}
  </div>

  <button class="list" onclick={onblocks}>
    <span>
      <strong>{overview.list.name}</strong>
      <small>{overview.list.summary}</small>
    </span>
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 6l6 6-6 6" /></svg>
  </button>

  <button class="list schedule" onclick={onschedule}>
    <span>
      <strong>Schedule</strong>
      <small>{next ? `Next: ${whenText(next)}` : schedules.length ? 'All off' : 'No focus times'}</small>
    </span>
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 6l6 6-6 6" /></svg>
  </button>

  {#if error}
    <p class="error" role="alert">{error}</p>
  {/if}

  <button class="btn primary start" onclick={() => onstart(minutes, picked.id)}>
    Plant &amp; start · {duration(minutes)}
  </button>
</div>

<style>
  .title {
    padding: 18px 24px 0;
  }
  .title h1 {
    font-size: 26px;
  }
  .title p {
    margin-top: 2px;
    font-size: 13px;
    color: var(--muted);
  }

  .durations {
    position: relative;
    margin: 14px 16px 0;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 6px;
  }
  .durations .thumb {
    position: absolute;
    top: 0;
    left: 0;
    width: calc((100% - 18px) / 4);
    height: 44px;
    box-sizing: border-box;
    border-radius: 12px;
    border: 1.5px solid var(--matcha);
    background: var(--matcha-tint);
    transition: transform 0.35s var(--ease);
  }
  .durations button {
    position: relative;
    height: 44px;
    border-radius: 12px;
    border: 1px solid var(--line);
    background: transparent;
    font-size: 14px;
  }
  .durations button[aria-checked='true'] {
    border-color: transparent;
    color: var(--matcha-deep);
    font-weight: 600;
  }

  /* The custom row opens under the duration buttons. */
  .custom {
    display: grid;
    grid-template-rows: 0fr;
    opacity: 0;
    transition:
      grid-template-rows 0.3s var(--ease),
      opacity 0.2s ease;
  }
  .custom.open {
    grid-template-rows: 1fr;
    opacity: 1;
  }
  .custom-row {
    overflow: hidden;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 16px;
  }
  .custom.open .custom-row {
    padding-top: 10px;
    /* Room for the focus ring. */
    padding-bottom: 2px;
  }
  .step {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .step:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  output {
    width: 78px;
    flex-shrink: 0;
    text-align: right;
    font-size: 14px;
    font-weight: 600;
    color: var(--matcha-deep);
    font-variant-numeric: tabular-nums;
  }
  input[type='range'] {
    appearance: none;
    -webkit-appearance: none;
    flex: 1;
    min-width: 0;
    height: 32px;
    margin: 0;
    background: transparent;
    cursor: pointer;
    border-radius: 8px;
  }
  input[type='range']::-webkit-slider-runnable-track {
    height: 6px;
    border-radius: 3px;
    background: linear-gradient(to right, #6e8f4a var(--fill), var(--line-soft) var(--fill));
  }
  input[type='range']::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 20px;
    height: 20px;
    margin-top: -7px;
    border-radius: 50%;
    background: var(--surface);
    border: 1.5px solid var(--matcha);
    box-shadow: 0 1px 3px rgba(62, 50, 38, 0.25);
    transition: transform 0.2s var(--ease);
  }
  input[type='range']:active::-webkit-slider-thumb {
    transform: scale(1.12);
  }

  /* Seven seeds in a row that scrolls; the fifth peeks in to show there's more. */
  .seeds {
    /* Seeds measure their place from the row, for scrolling one into view. */
    position: relative;
    margin-top: 10px;
    padding: 2px 16px 4px;
    overflow-x: auto;
    scrollbar-width: none;
    scroll-padding: 0 16px;
    mask-image: linear-gradient(to right, transparent, #000 14px, #000 calc(100% - 22px), transparent);
  }
  /* The row can be dragged; the track is what stretches past its ends. */
  .track {
    display: flex;
    gap: 6px;
    width: max-content;
  }
  .seeds:global(.dragging),
  .seeds:global(.dragging) .seed {
    cursor: grabbing;
  }
  .seed {
    /* At least 74px; a long name (Hydrangea) widens its own tile. */
    flex: 0 0 auto;
    min-width: 74px;
    box-sizing: border-box;
    padding: 6px 7px 8px;
    border-radius: 14px;
    border: 1.5px solid var(--line);
    background: var(--tile);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  .seed .name {
    font-size: 12px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 3px;
  }
  .seed .need {
    font-size: 11px;
    color: var(--muted);
  }
  .seed[aria-checked='true'] {
    border-color: var(--matcha);
    background: var(--surface);
    box-shadow: 0 4px 10px rgba(62, 50, 38, 0.12);
    transform: translateY(-2px);
  }
  .seed[aria-checked='true'] .name {
    color: var(--matcha-deep);
  }
  .seed[aria-checked='true'] .need {
    color: var(--matcha);
  }
  .seed.locked {
    border-style: dashed;
    border-color: var(--line-strong);
    background: transparent;
    color: #7a6650;
    cursor: not-allowed;
  }
  .seed.locked .need {
    color: inherit;
  }
  .seed.locked :global(svg:first-child) {
    opacity: 0.45;
  }

  .stages {
    margin: 6px 16px 0;
    padding: 8px 12px 6px;
    border-radius: 16px;
    background: var(--panel);
  }
  .stages .eyebrow {
    letter-spacing: 0.12em;
  }
  .strip {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
  }
  .stage {
    animation: stage-in 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 40ms);
  }
  @keyframes stage-in {
    from {
      opacity: 0;
      transform: translateY(calc(4px * var(--travel)));
    }
  }
  .seed:not(.locked):not([aria-checked='true']):hover {
    transform: translateY(-1px);
  }

  .list {
    margin: 10px 16px 0;
    height: 48px;
    flex-shrink: 0;
    padding: 0 14px;
    border-radius: 14px;
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: space-between;
    text-align: left;
  }
  .list.schedule {
    margin-top: 6px;
  }
  .list span {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .list strong {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
  }
  .list small {
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .error {
    margin: 8px 20px 0;
    font-size: 12px;
    color: var(--clay-deep);
  }
  .start {
    margin: 10px 16px 0;
    font-size: 15px;
    flex-shrink: 0;
    width: calc(100% - 32px);
  }
</style>
