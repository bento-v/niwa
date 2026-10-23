<script lang="ts">
  import type { Ended } from '../lib/api';
  import { clock, duration, lengthInWords, slipsInWords } from '../lib/format';
  import Scene from '../lib/Scene.svelte';

  interface Props {
    ended: Ended;
    ongarden: () => void;
    onagain: () => void;
  }

  let { ended, ongarden, onagain }: Props = $props();

  const grown = $derived(ended.outcome === 'grown');
  const plant = $derived(ended.speciesName.toLowerCase());
</script>

<!-- Fully grown and Withered share a layout: the plant, what happened, two ways on. -->
<div class="screen" class:withered={!grown}>
  <div class="plant">
    <Scene
      tall
      species={ended.species}
      withered={!grown}
      label={grown ? `A fully grown ${plant} in a clay pot` : `A withered ${plant} in a cracked clay pot`}
    />
  </div>

  <div class="words">
    <div class="eyebrow">{grown ? 'Session complete' : 'Session ended early'}</div>
    <h1 class="display">{grown ? `Your ${plant} is fully grown` : `Your ${plant} withered`}</h1>
    {#if grown}
      <p>{lengthInWords(ended.plannedMinutes)} of focus, {slipsInWords(ended.slips)}.</p>
    {:else}
      <p>
        You gave up with {clock(ended.remainingSecs)} left. It will rest in your garden as a
        reminder.
        {ended.todayMinutes === 0 ? 'Finish one session today to keep your streak.' : ''}
      </p>
    {/if}
  </div>

  {#if grown}
    <div class="chips">
      <span class="chip green">{duration(ended.todayMinutes)} today</span>
      <span class="chip">{ended.streakDays}-day streak</span>
    </div>
  {/if}

  <div class="actions">
    {#if grown}
      <button class="btn" onclick={onagain}>Grow another</button>
      <button class="btn primary" data-autofocus onclick={ongarden}>Place in garden</button>
    {:else}
      <button class="btn" onclick={ongarden}>View garden</button>
      <button class="btn primary" onclick={onagain}>Plant a new seed</button>
    {/if}
  </div>
</div>

<style>
  .plant {
    margin-top: 4px;
    animation: rise 0.55s var(--ease) both;
  }
  .words {
    animation: rise 0.45s var(--ease) 0.12s both;
  }
  .chips {
    animation: rise 0.45s var(--ease) 0.22s both;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(calc(8px * var(--travel)));
    }
  }
  .words {
    padding: 20px 24px 0;
    min-width: 0;
    text-align: center;
  }
  .withered .words {
    padding: 20px 28px 0;
  }
  .eyebrow {
    color: var(--matcha);
  }
  .withered .eyebrow {
    color: #7a6650;
  }
  h1 {
    margin-top: 6px;
    font-size: 28px;
  }
  p {
    margin-top: 6px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
  }
  .withered p {
    margin-top: 8px;
  }
  .chips {
    padding: 14px 24px 0;
    display: flex;
    justify-content: center;
    gap: 8px;
  }
  .chips .chip {
    padding: 6px 12px;
    color: #5a4836;
  }
  .chips .chip.green {
    color: var(--matcha-deep);
  }
  .actions {
    padding: 20px 16px 16px;
    margin-top: auto;
    display: flex;
    gap: 8px;
  }
  .actions .btn {
    flex: 1;
  }
  .actions .btn.primary {
    flex: 2;
  }
</style>
