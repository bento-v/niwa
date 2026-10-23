<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import type { Session } from '../lib/api';
  import { clock, duration } from '../lib/format';
  import { settle, travel } from '../lib/motion.svelte';
  import { STAGE_NAMES } from '../lib/plants';
  import Scene from '../lib/Scene.svelte';

  interface Props {
    session: Session;
    /** The apps Niwa just closed, shown briefly as a gentle nudge. */
    nudge: string[];
    /** Why the last request to end, or to cancel ending, failed. */
    error: string;
    /** Strict: ask to end early, which starts the 15-minute wait. */
    onrequestexit: () => void;
    oncancelexit: () => void;
  }

  let { session, nudge, error, onrequestexit, oncancelexit }: Props = $props();
  const strict = $derived(session.mode === 'strict');
  const leaving = $derived(session.exitSecsLeft !== null);

  /** How much of the session has passed, 0 to 1. The bar draws it unrounded so it creeps on every tick. */
  const elapsed = $derived((session.totalSecs - session.remainingSecs) / session.totalSecs);
  const progress = $derived(Math.round(elapsed * 100));
  const minutesLeft = $derived(Math.max(1, Math.ceil(session.remainingSecs / 60)));
  const mode = $derived(session.mode === 'strict' ? 'Strict' : 'Gentle');
  const article = (word: string) => (/^[aeiou]/i.test(word) ? 'an' : 'a');
</script>

<div class="screen">
  <div class="timer">
    <div class="eyebrow">{session.listName} · {mode}{session.scheduled ? ' · Scheduled' : ''}</div>
    <div class="display time" role="timer" aria-label="{clock(session.remainingSecs)} left">
      {clock(session.remainingSecs)}
    </div>
    <div class="of">left of {article(duration(session.plannedMinutes))} {duration(session.plannedMinutes)} session</div>
    <div class="bar" role="progressbar" aria-label="Session progress" aria-valuenow={progress} aria-valuemin="0" aria-valuemax="100">
      <div style:transform="scaleX({elapsed})"></div>
    </div>
  </div>

  <!-- Apps that were open at the start get a minute to be saved and quit. -->
  {#if session.closing.length > 0}
    <div class="closing" role="status" in:fade={{ duration: 300 }}>
      <strong>Save and quit:</strong>
      {session.closing.map((app) => app.label).join(', ')}
      {session.closing.length === 1 ? 'closes' : 'close'} in {session.closing[0].secsLeft} s.
    </div>
  {:else if session.exitSecsLeft !== null}
    <!-- Strict: the wait before an early end. Blocking carries on throughout. -->
    <div class="closing" role="status" in:fade={{ duration: 300 }}>
      <strong>Ending in {clock(session.exitSecsLeft)}.</strong>
      Still blocking until then.
    </div>
  {:else}
    <div class="blocking" in:fade={{ duration: 300 }}>
      <span class="label">Blocking</span>
      {#each session.blocking as name}
        <span class="chip" class:green={!name.startsWith('+')}>{name}</span>
      {:else}
        <span class="chip">Nothing</span>
      {/each}
    </div>
  {/if}

  <div class="plant">
    <Scene
      alive
      species={session.species}
      stage={session.stage}
      label="{session.speciesName}, {STAGE_NAMES[session.stage - 1]}, growing in a clay pot"
    />
    <div class="labels">
      <span><strong>{session.speciesName}</strong> · stage {session.stage} of 5</span>
      <span>Fully grown in {duration(minutesLeft)}</span>
    </div>
    {#if nudge.length > 0}
      <div class="nudge" role="status" in:fly={{ y: travel(-8), duration: 380, easing: settle }} out:fade={{ duration: 260 }}>
        <strong>{nudge.length > 2 ? `${nudge[0]} and ${nudge.length - 1} others` : nudge.join(' and ')}</strong>
        {nudge.length === 1 ? 'was' : 'were'} closed. Your {session.speciesName.toLowerCase()} is still growing.
      </div>
    {/if}
  </div>

  <div class="actions">
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    {#if leaving}
      <button class="btn primary" onclick={oncancelexit}>Cancel and keep growing</button>
    {:else}
      <!-- Both modes end early only after a wait: five minutes in Gentle, a third of the session in Strict. -->
      <button class="btn" onclick={onrequestexit}>Request to end</button>
    {/if}
  </div>
</div>

<style>
  .timer {
    padding: 22px 24px 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }
  .time {
    font-size: 64px;
    font-weight: 350;
    line-height: 1.05;
    letter-spacing: -0.02em;
    font-variant-numeric: tabular-nums;
  }
  .of {
    font-size: 13px;
    color: var(--muted);
  }
  .bar {
    width: 100%;
    height: 6px;
    margin-top: 14px;
    border-radius: 3px;
    background: var(--line-soft);
    overflow: hidden;
  }
  .bar div {
    height: 100%;
    border-radius: 3px;
    background: #6e8f4a;
    transform-origin: 0 50%;
  }

  .blocking,
  .closing {
    margin-top: 14px;
    min-height: 26px;
    flex-shrink: 0;
  }
  .blocking {
    padding: 0 24px;
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
  }
  .blocking .label {
    font-size: 12px;
    color: var(--muted);
    margin-right: 2px;
  }
  .closing {
    margin-left: 16px;
    margin-right: 16px;
    padding: 5px 12px;
    box-sizing: border-box;
    border-radius: 10px;
    background: var(--clay-tint);
    color: var(--clay-deep);
    font-size: 12px;
    line-height: 16px;
    font-variant-numeric: tabular-nums;
  }

  .plant {
    position: relative;
    margin-top: 16px;
  }
  .labels {
    position: absolute;
    left: 30px;
    right: 30px;
    top: 12px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: #5a4836;
  }
  .labels span {
    min-width: 0;
    background: var(--card);
    padding: 5px 10px;
    border-radius: 999px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .labels strong {
    font-weight: 500;
  }
  .nudge {
    position: absolute;
    left: 30px;
    right: 30px;
    top: 46px;
    padding: 9px 12px;
    border-radius: 12px;
    background: var(--surface);
    box-shadow: 0 4px 12px rgba(62, 50, 38, 0.14);
    font-size: 12px;
    line-height: 1.4;
    color: #5a4836;
  }
  .nudge strong {
    color: var(--text);
  }

  .actions {
    padding: 14px 16px 16px;
    margin-top: auto;
  }
  .error {
    margin: 0 4px 8px;
    font-size: 12px;
    color: var(--clay-deep);
  }
  .actions .btn {
    width: 100%;
    height: 44px;
    border-radius: 12px;
  }
</style>
