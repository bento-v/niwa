<script lang="ts">
  import { fade } from 'svelte/transition';
  import { backend, type Session } from './lib/api';
  import { closedFeed } from './lib/closed.svelte';
  import { clock } from './lib/format';
  import Plant from './lib/Plant.svelte';

  // The mini timer: what the card collapses into when it's minimised. It
  // follows the same session events as the card and opens the card on click.

  let session = $state<Session | null>(null);

  async function refresh(attempt = 0) {
    try {
      session = (await backend.overview()).session;
    } catch {
      // Niwa may still be starting up: try again shortly.
      setTimeout(() => refresh(attempt + 1), Math.min(4000, 250 * 2 ** attempt));
    }
  }
  refresh();
  backend.on('session-tick', (tick) => (session = tick));
  backend.on('session-ended', () => {
    session = null;
    closed.clear();
  });
  // When Niwa closes an app, the timer says so for a few seconds.
  const closed = closedFeed();
  const pinging = $derived(session !== null && closed.labels.length > 0);
  // Catch up whenever the timer is shown again.
  document.addEventListener('visibilitychange', () => {
    if (!document.hidden) refresh();
  });

  // A press that moves becomes a drag; a press that doesn't is a click.
  const DRAG_THRESHOLD = 4;
  let pressedAt: { x: number; y: number } | null = null;
  let dragged = false;

  function onpointerdown(event: PointerEvent) {
    if (event.button !== 0) return;
    pressedAt = { x: event.screenX, y: event.screenY };
    dragged = false;
  }
  function onpointermove(event: PointerEvent) {
    if (!pressedAt) return;
    if (Math.hypot(event.screenX - pressedAt.x, event.screenY - pressedAt.y) > DRAG_THRESHOLD) {
      pressedAt = null;
      dragged = true;
      backend.dragMini();
    }
  }
  function onclick() {
    pressedAt = null;
    if (dragged) {
      dragged = false;
      return;
    }
    backend.openCard();
  }

  const strict = $derived(session?.mode === 'strict');
  const label = $derived(
    session
      ? `Open Niwa. ${clock(session.remainingSecs)} left, ${session.speciesName} stage ${session.stage} of 5, ${strict ? 'Strict' : 'Gentle'}.`
      : 'Open Niwa to plant a seed.',
  );
</script>

<button
  class="mini"
  class:pinging
  aria-label={label}
  title="Click to open Niwa, or drag to move"
  {onpointerdown}
  {onpointermove}
  onpointerup={() => (pressedAt = null)}
  {onclick}
>
  <span class="plant">
    <Plant species={session?.species ?? 'sprout'} stage={session ? session.stage : 5} width={40} tight />
    {#if pinging}
      <!-- One soft ring per close, like a drop on still water. -->
      {#key closed.count}
        <span class="ring" aria-hidden="true"></span>
      {/key}
    {/if}
  </span>
  <span class="words">
    {#if session}
      <span class="time display">{clock(session.remainingSecs)}</span>
      {#if pinging}
        <span class="detail closed" in:fade={{ duration: 240 }}>Closed {closed.text}</span>
      {:else if session.exitSecsLeft !== null}
        <span class="detail ending">Ending in {clock(session.exitSecsLeft)}</span>
      {:else}
        <span class="detail">{session.speciesName} · stage {session.stage} of 5</span>
      {/if}
    {:else}
      <span class="idle display">Ready to focus</span>
      <span class="detail">Click to plant a seed</span>
    {/if}
  </span>
  {#if session}
    <span class="mode" class:strict>{strict ? 'Strict' : 'Gentle'}</span>
  {/if}
</button>
<span class="visually-hidden" role="status">{pinging ? `Niwa closed ${closed.text}` : ''}</span>

<style>
  .mini {
    width: 252px;
    height: 60px;
    box-sizing: border-box;
    padding: 0 12px 0 8px;
    display: flex;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--line);
    border-radius: 16px;
    background: var(--card);
    box-shadow:
      0 6px 16px rgba(62, 50, 38, 0.2),
      0 1px 3px rgba(62, 50, 38, 0.12);
    text-align: left;
  }
  .mini {
    animation: mini-in 0.35s var(--ease) both;
  }
  @keyframes mini-in {
    from {
      opacity: 0;
      transform: translateY(calc(6px * var(--travel)));
    }
  }
  .mini:hover {
    border-color: var(--line-strong);
  }
  .plant {
    position: relative;
    flex-shrink: 0;
  }
  .mini.pinging {
    border-color: #dcc0ab;
  }
  .ring {
    position: absolute;
    left: 50%;
    top: 50%;
    width: 40px;
    height: 40px;
    margin: -20px 0 0 -20px;
    border-radius: 50%;
    border: 1.5px solid var(--clay);
    pointer-events: none;
    opacity: 0;
    animation: ring 1.4s var(--ease-settle);
  }
  @keyframes ring {
    from {
      opacity: 0.7;
      transform: scale(calc(1 - 0.4 * var(--travel)));
    }
    to {
      opacity: 0;
      transform: scale(calc(1 + 0.25 * var(--travel)));
    }
  }
  .detail.closed {
    color: var(--clay-deep);
  }
  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
  .words {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .time {
    font-size: 22px;
    font-weight: 350;
    line-height: 1.1;
    letter-spacing: -0.01em;
    font-variant-numeric: tabular-nums;
  }
  .idle {
    font-size: 17px;
    line-height: 1.2;
  }
  .detail {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-variant-numeric: tabular-nums;
  }
  .detail.ending {
    color: var(--clay-deep);
  }
  .mode {
    flex-shrink: 0;
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
</style>
