<script lang="ts">
  import { cubicOut } from 'svelte/easing';
  import { Tween } from 'svelte/motion';
  import { backend, type Stats } from '../lib/api';
  import { reduced } from '../lib/motion.svelte';

  let stats = $state<Stats | null>(null);
  /** The tiles' figures count up from nothing as they settle in, 0 to 1. */
  const counted = new Tween(0, { duration: 1100, easing: cubicOut, delay: 260 });
  backend.stats().then((loaded) => {
    stats = loaded;
    counted.set(1, reduced() ? { duration: 0, delay: 0 } : undefined);
  });
  const upTo = (value: number) => Math.round(value * counted.current);

  const DAYS = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];
  const DATE = new Intl.DateTimeFormat(undefined, { weekday: 'long', month: 'short', day: 'numeric' });

  /** The day picked on the chart, which then takes the heading; `null` shows the week. */
  let picked = $state<number | null>(null);
  /** The date of a bar: today's place in the week says how far back it is. */
  function dateOf(day: number): Date {
    const date = new Date();
    date.setDate(date.getDate() - ((stats?.today ?? day) - day));
    return date;
  }
  function pick(day: number) {
    picked = picked === day ? null : day;
  }
  /** Clicking anywhere but a bar, or Escape, goes back to the week. */
  function unpick(event: MouseEvent) {
    if (!(event.target as Element).closest?.('button.day')) picked = null;
  }
  const DAY_NAMES = ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'];
  /** The tallest a bar can be inside the chart, in pixels. */
  const BAR_ROOM = 120;

  /** "8 h 10 m", "45 m" */
  function hoursMinutes(minutes: number): string {
    const h = Math.floor(minutes / 60);
    const m = minutes % 60;
    if (h === 0) return `${m} m`;
    return m === 0 ? `${h} h` : `${h} h ${m} m`;
  }
  /** "23.5h", "2h" */
  function hours(minutes: number): string {
    const value = minutes / 60;
    return `${Number.isInteger(value) ? value : value.toFixed(1)}h`;
  }

  // Bars are scaled to the week's best day, but never so far that half an
  // hour fills the chart.
  const scale = $derived(stats ? BAR_ROOM / Math.max(60, ...stats.days) : 1);
  const mostTempting = $derived(stats ? Math.max(1, ...stats.tempting.map((app) => app.count)) : 1);
  const chartLabel = $derived(
    stats
      ? `Focus minutes per day: ${stats.days
          .slice(0, stats.today + 1)
          .map((minutes, i) => `${DAY_NAMES[i]} ${minutes}`)
          .join(', ')}`
      : '',
  );
</script>

<svelte:document onclick={unpick} onkeydown={(e) => e.key === 'Escape' && (picked = null)} />

<div class="screen">
  {#if stats}
    <div class="title" aria-live="polite">
      <div class="eyebrow">{picked === null ? 'This week' : DATE.format(dateOf(picked))}</div>
      <div class="display total">
        {hoursMinutes(picked === null ? stats.weekMinutes : stats.days[picked])}
        {#if picked === null && stats.changePercent !== null}
          <span class="change" class:down={stats.changePercent < 0}>
            {stats.changePercent < 0 ? '−' : '+'}{Math.abs(stats.changePercent)}%
          </span>
        {/if}
      </div>
    </div>

    <!-- Each bar is a button: picking one shows that day's total in the heading. -->
    <div class="chart" role="group" aria-label={chartLabel}>
      {#each stats.days as minutes, i}
        {@const ahead = i > stats.today}
        <button
          class="day"
          class:today={i === stats.today}
          class:lit={i === stats.today && picked === null}
          class:picked={picked === i}
          style:--i={i}
          disabled={ahead}
          aria-pressed={picked === i}
          aria-label="{DATE.format(dateOf(i))}: {minutes === 0 ? 'no focus' : hoursMinutes(minutes)}"
          onclick={() => pick(i)}
        >
          {#if ahead || minutes === 0}
            <div class="bar empty"></div>
          {:else}
            <div class="bar" style:height="{Math.max(6, Math.round(minutes * scale))}px"></div>
          {/if}
          <span>{DAYS[i]}</span>
        </button>
      {/each}
    </div>

    <div class="tiles">
      <div class="tile" style:--i={0}>
        <div class="display">{upTo(stats.streakDays)}</div>
        <div class="caption">day streak</div>
      </div>
      <div class="tile" style:--i={1}>
        <div class="display">{hours(upTo(stats.monthMinutes))}</div>
        <div class="caption">this month</div>
      </div>
      <div class="tile" style:--i={2}>
        <div class="display">{stats.completionPercent === null ? '–' : `${upTo(stats.completionPercent)}%`}</div>
        <div class="caption">completed</div>
      </div>
      <div class="tile" style:--i={3}>
        <div class="display">{hours(upTo(stats.totalMinutes))}</div>
        <div class="caption">in total</div>
      </div>
    </div>

    <div class="tempting">
      <div class="eyebrow">Most tempting</div>
      {#each stats.tempting as app, i}
        <div class="row" style:--i={i}>
          <span class="name" title={app.name}>{app.name}</span>
          <div class="track"><div style:width="{(app.count / mostTempting) * 100}%"></div></div>
          <span class="count">{app.count}</span>
        </div>
      {:else}
        <p class="none">Nothing has tempted you this week.</p>
      {/each}
    </div>
  {/if}
</div>

<style>
  .title {
    padding: 20px 24px 0;
  }
  .total {
    margin-top: 2px;
    font-size: 34px;
  }
  .change {
    font-family: var(--body);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0;
    color: var(--matcha);
  }
  .change.down {
    color: var(--muted);
  }

  .chart {
    margin: 14px 16px 0;
    padding: 14px 14px 10px;
    height: 168px;
    box-sizing: border-box;
    border-radius: 18px;
    background: var(--panel);
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 8px;
    flex-shrink: 0;
  }
  .day {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  .day:disabled {
    cursor: default;
  }
  .day.picked span {
    color: var(--text);
    font-weight: 600;
  }
  .picked .bar {
    background: var(--matcha-deep);
  }
  .picked .bar.empty {
    background: #c4b393;
  }
  .day span {
    font-size: 11px;
    color: var(--muted);
  }
  .day.today span {
    color: var(--matcha-deep);
    font-weight: 600;
  }
  .bar {
    width: 100%;
    border-radius: 6px 6px 3px 3px;
    background: #c3d1a3;
    transform-origin: 50% 100%;
    animation: grow-up 0.7s var(--ease-settle) both;
    animation-delay: calc(var(--i) * 45ms + 80ms);
  }
  @keyframes grow-up {
    from {
      opacity: 0;
      transform: scaleY(calc(1 - var(--travel)));
    }
  }
  /* Today's bar stands out only while no day is picked: picking a day gives
     that bar the colour, today's included. */
  .lit .bar {
    background: var(--matcha);
  }
  .bar.empty {
    height: 6px;
    border-radius: 3px;
    background: #dccdb2;
  }

  .tiles {
    padding: 12px 16px 0;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }
  .tile {
    animation: settle 0.4s var(--ease) both;
    animation-delay: calc(var(--i) * 60ms + 260ms);
    background: var(--surface);
    border: 1px solid var(--line-soft);
    border-radius: 14px;
    padding: 10px 10px;
  }
  .tile .display {
    font-size: 22px;
    font-variant-numeric: tabular-nums;
  }
  .caption {
    font-size: 11px;
    color: var(--muted);
  }

  .tempting {
    padding: 16px 24px 0;
  }
  .tempting .eyebrow {
    margin-bottom: 10px;
  }
  .row {
    display: grid;
    grid-template-columns: 96px minmax(0, 1fr) 28px;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    margin-bottom: 10px;
  }
  .name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .track {
    height: 8px;
    border-radius: 4px;
    background: var(--line-soft);
  }
  .track div {
    height: 100%;
    border-radius: 4px;
    background: var(--clay);
    transform-origin: 0 50%;
    animation: fill 0.7s var(--ease-settle) both;
    animation-delay: calc(var(--i) * 70ms + 420ms);
  }
  @keyframes fill {
    from {
      opacity: 0;
      transform: scaleX(calc(1 - var(--travel)));
    }
  }
  @keyframes settle {
    from {
      opacity: 0;
      transform: translateY(calc(6px * var(--travel)));
    }
  }
  .count {
    text-align: right;
    color: var(--muted);
  }
  .none {
    font-size: 13px;
    color: var(--muted);
  }
</style>
