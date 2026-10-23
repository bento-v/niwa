<script lang="ts">
  import { tick } from 'svelte';
  import { fade, fly, scale } from 'svelte/transition';
  import { backend, type Garden, type GardenPlant } from '../lib/api';
  import { duration, lengthInWords, shortDate, shortDuration, slipsInWords } from '../lib/format';
  import { settle, travel } from '../lib/motion.svelte';
  import Plant from '../lib/Plant.svelte';
  import Scene from '../lib/Scene.svelte';

  interface Props {
    onplant: () => void;
  }

  let { onplant }: Props = $props();

  let garden = $state<Garden | null>(null);
  backend.garden().then((loaded) => (garden = loaded));

  /** The plant being looked at closely, by its place in the garden. */
  let looking = $state<number | null>(null);
  const plant = $derived(garden && looking !== null ? garden.plants[looking] : null);
  /** Which way the last step went: older plants come in from the right, newer from the left. */
  let stepped = $state(0);

  let back = $state<HTMLButtonElement>();
  let grid = $state<HTMLDivElement>();

  // A long garden is drawn a page at a time as it scrolls into view. Every
  // plant is a few hundred SVG nodes; a year of sessions drawn at once would
  // hold the screen up for seconds.
  const PAGE = 30;
  let shown = $state(PAGE);
  let more = $state<HTMLDivElement>();
  $effect(() => {
    // Watched afresh after every page, so a page that leaves the end still
    // in view asks for the next one.
    void shown;
    if (!more || !grid) return;
    const observer = new IntersectionObserver((entries) => {
      if (entries.some((entry) => entry.isIntersecting)) shown += PAGE;
    }, { root: grid, rootMargin: '0px 0px 400px 0px' });
    observer.observe(more);
    return () => observer.disconnect();
  });

  // Keyboard focus follows the view: into the closer look when it opens, and
  // back to the plant's own tile when it closes.
  async function look(at: number) {
    stepped = 0;
    looking = at;
    await tick();
    back?.focus();
  }
  function step(by: number) {
    if (looking === null || !garden) return;
    const next = looking + by;
    if (next < 0 || next >= garden.plants.length) return;
    stepped = by;
    looking = next;
  }
  async function leave() {
    const at = looking;
    // Stepping can go past the plants drawn so far; draw up to this one.
    shown = Math.max(shown, (at ?? 0) + 1);
    looking = null;
    await tick();
    grid?.querySelectorAll<HTMLElement>('.tile')[at ?? 0]?.focus();
  }

  const DAY = 86_400_000;
  // A plant from the last day is still "new".
  const isNew = (index: number, createdMs: number) => index === 0 && Date.now() - createdMs < DAY;

  const WHEN = new Intl.DateTimeFormat(undefined, {
    weekday: 'long',
    month: 'short',
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
  });

  function story(plant: GardenPlant): string {
    if (plant.withered) {
      return `Withered after ${lengthInWords(plant.minutes)} of a ${duration(plant.plannedMinutes)} session.`;
    }
    return `${lengthInWords(plant.minutes)} of focus, ${slipsInWords(plant.slips)}.`;
  }

  function onkeydown(event: KeyboardEvent) {
    if (looking === null) return;
    // A sheet open over the garden (Settings) keeps its keys to itself.
    if (event.target instanceof Element && event.target.closest('[role="dialog"]')) return;
    if (event.key === 'Escape') leave();
    if (event.key === 'ArrowLeft') step(-1);
    if (event.key === 'ArrowRight') step(1);
  }
</script>

<svelte:window {onkeydown} />

<div class="screen">
  {#if plant && looking !== null && garden}
    <!-- A closer look at one plant, opened from its tile. -->
    <div class="closer" in:scale={{ start: 1 - travel(0.04), duration: 340, easing: settle }}>
      {#key looking}
      <div class="scene-wrap" in:fly={{ x: travel(stepped * 14), duration: 360, easing: settle }}>
        <Scene
          species={plant.species}
          withered={plant.withered}
          label={plant.withered ? `A withered ${plant.speciesName.toLowerCase()}` : `A fully grown ${plant.speciesName.toLowerCase()}`}
        />
        <span class="mode" class:strict={plant.mode === 'strict'}>{plant.mode === 'strict' ? 'Strict' : 'Gentle'}</span>
      </div>
      <div class="about" in:fade={{ duration: 360, delay: 40 }}>
        <h1 class="display">{plant.withered ? `Withered ${plant.speciesName.toLowerCase()}` : plant.speciesName}</h1>
        <p>{story(plant)}</p>
        <p class="when">{WHEN.format(new Date(plant.createdMs))}</p>
      </div>
      {/key}
      <div class="nav">
        <button class="step" aria-label="Newer plant" disabled={looking === 0} onclick={() => step(-1)}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15 6l-6 6 6 6" /></svg>
        </button>
        <button class="btn back" bind:this={back} onclick={leave}>Back to garden</button>
        <button class="step" aria-label="Older plant" disabled={looking === garden.plants.length - 1} onclick={() => step(1)}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 6l6 6-6 6" /></svg>
        </button>
      </div>
    </div>
  {:else}
    <div class="title">
      <h1 class="display">Your garden</h1>
      {#if garden && garden.plants.length > 0}
        <span>{garden.grown} grown · {garden.withered} withered</span>
      {/if}
    </div>

    {#if garden && garden.plants.length === 0}
      <div class="empty">
        <Plant species="sprout" stage={1} width={96} detailed />
        <p>Nothing planted yet. Finish a session and your first plant will stand here.</p>
        <button class="btn primary" onclick={onplant}>Plant a seed</button>
      </div>
    {:else if garden}
      <div class="grid scroll" bind:this={grid}>
        {#each garden.plants.slice(0, shown) as plant, i (plant.id)}
          {@const fresh = isNew(i, plant.createdMs)}
          <button
            class="tile"
            class:fresh
            class:withered={plant.withered}
            style:--delay="{Math.min(i, 11) * 28}ms"
            aria-label="{plant.withered ? 'Withered ' : ''}{plant.speciesName}, {shortDuration(plant.minutes)}, {shortDate(plant.createdMs)}{fresh ? ', new' : ''}. Look closer."
            onclick={() => look(i)}
          >
            {#if fresh}
              <span class="badge">New</span>
            {/if}
            <Plant species={plant.species} withered={plant.withered} detailed width={76} />
            <span class="what">{plant.speciesName}</span>
            <span class="when">{shortDuration(plant.minutes)} · {shortDate(plant.createdMs)}</span>
          </button>
        {/each}
        {#if shown < garden.plants.length}
          <div class="more" bind:this={more}></div>
        {/if}
      </div>
      <div class="footer">
        {#if garden.nextUnlock}
          Next unlock: {garden.nextUnlock.name} at {garden.nextUnlock.hours} h
        {:else}
          Every species unlocked
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .title {
    padding: 20px 24px 12px;
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 8px;
  }
  .title h1 {
    font-size: 26px;
    white-space: nowrap;
  }
  .title span {
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
  }

  .grid {
    margin: 0 16px;
    padding: 12px;
    border-radius: 18px;
    background: var(--panel);
    display: grid;
    /* Three across at the card's smallest; more as it widens. */
    grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
    gap: 10px;
    align-content: start;
    flex: 0 1 auto;
    min-height: 0;
  }
  .more {
    grid-column: 1 / -1;
    height: 1px;
  }
  .tile {
    position: relative;
    min-width: 0;
    background: var(--tile);
    border-radius: 14px;
    border: 1.5px solid transparent;
    padding: 8px 6px 7px;
    display: flex;
    flex-direction: column;
    align-items: center;
    /* Tiles settle in one after another. */
    animation: tile-in 0.36s var(--ease) both;
    animation-delay: var(--delay);
  }
  @keyframes tile-in {
    from {
      opacity: 0;
      transform: translateY(calc(6px * var(--travel))) scale(calc(1 - 0.02 * var(--travel)));
    }
  }
  .tile:hover {
    border-color: var(--line-strong);
  }
  .tile.fresh {
    border-color: #6e8f4a;
  }
  .tile.withered {
    background: #efe6d6;
  }
  .badge {
    position: absolute;
    top: 6px;
    left: 6px;
    padding: 1px 6px;
    border-radius: 999px;
    background: var(--matcha-tint);
    color: var(--matcha-deep);
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .what,
  .when {
    max-width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .what {
    margin-top: 4px;
    font-size: 11px;
    font-weight: 500;
    color: var(--text);
  }
  .fresh .what {
    color: var(--matcha-deep);
    font-weight: 600;
  }
  .withered .what {
    color: #7a6650;
    font-style: italic;
  }
  .when {
    font-size: 10px;
    letter-spacing: 0.02em;
    color: var(--muted);
  }

  .footer {
    padding: 14px 24px 16px;
    font-size: 12px;
    color: var(--muted);
    margin-top: auto;
  }

  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding: 0 40px 40px;
    text-align: center;
  }
  .empty p {
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
  }
  .empty .btn {
    padding: 0 28px;
  }

  .closer {
    flex: 1;
    display: flex;
    flex-direction: column;
    padding-top: 14px;
  }
  .scene-wrap {
    position: relative;
  }
  .mode {
    position: absolute;
    top: 12px;
    right: 30px;
    font-size: 11px;
    font-weight: 600;
    padding: 4px 9px;
    border-radius: 999px;
    background: var(--matcha-tint);
    color: var(--matcha-deep);
  }
  .mode.strict {
    background: var(--clay-tint);
    color: var(--clay-deep);
  }
  .about {
    padding: 16px 24px 0;
    text-align: center;
  }
  .about h1 {
    font-size: 26px;
  }
  .about p {
    margin-top: 6px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
  }
  .about .when {
    margin-top: 2px;
    font-size: 12px;
    white-space: normal;
  }
  .nav {
    margin-top: auto;
    padding: 14px 16px 16px;
    display: flex;
    gap: 8px;
  }
  .nav .back {
    flex: 1;
    height: 44px;
  }
  .step {
    width: 44px;
    height: 44px;
    flex-shrink: 0;
    border-radius: 12px;
    border: 1px solid var(--line-strong);
    background: transparent;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .step:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }
</style>
