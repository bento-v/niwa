<script lang="ts">
  import { backend, type Schedule } from '../lib/api';
  import { lengthInWords } from '../lib/format';
  import { DAY_LETTERS, DAY_NAMES, daysText, lengthMinutes, timeText } from '../lib/schedule';
  import Sheet from '../lib/Sheet.svelte';

  interface Props {
    schedules: Schedule[];
    onchange: (schedules: Schedule[]) => void;
    onclose: () => void;
  }

  let { schedules, onchange, onclose }: Props = $props();

  /** The focus time being added or changed, if any. */
  let editing = $state<Schedule | null>(null);
  let error = $state('');

  const WEEKDAYS = 0b001_1111;
  function add() {
    error = '';
    editing = { id: 0, days: WEEKDAYS, start: 9 * 60, end: 12 * 60, enabled: true };
  }
  function edit(schedule: Schedule) {
    error = '';
    editing = { ...schedule };
  }

  const clock = (minutes: number) =>
    `${String(Math.floor(minutes / 60)).padStart(2, '0')}:${String(minutes % 60).padStart(2, '0')}`;
  const minutesOf = (value: string) => {
    const [hours, minutes] = value.split(':').map(Number);
    return hours * 60 + minutes;
  };

  async function run(task: Promise<Schedule[]>) {
    error = '';
    try {
      onchange(await task);
      editing = null;
    } catch (reason) {
      error = String(reason);
    }
  }
  async function toggle(schedule: Schedule, enabled: boolean) {
    try {
      onchange(await backend.saveSchedule({ ...schedule, enabled }));
    } catch {
      onchange(schedules);
    }
  }
</script>

{#if editing}
  {@const length = lengthMinutes(editing)}
  <Sheet title={editing.id ? 'Focus time' : 'New focus time'} onclose={() => (editing = null)}>
    <div class="days" role="group" aria-label="Days">
      {#each DAY_LETTERS as letter, i}
        <button
          class="day"
          aria-pressed={(editing.days & (1 << i)) !== 0}
          aria-label={DAY_NAMES[i]}
          onclick={() => editing && (editing.days ^= 1 << i)}
        >
          {letter}
        </button>
      {/each}
    </div>
    <div class="times">
      <label>
        <span class="eyebrow">From</span>
        <input type="time" value={clock(editing.start)} onchange={(e) => editing && (editing.start = minutesOf(e.currentTarget.value))} />
      </label>
      <label>
        <span class="eyebrow">Until</span>
        <input type="time" value={clock(editing.end)} onchange={(e) => editing && (editing.end = minutesOf(e.currentTarget.value))} />
      </label>
    </div>
    <p class="length">
      {lengthInWords(length)}{editing.end <= editing.start ? ', ending the next day' : ''}
    </p>
    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}
    <div class="actions">
      {#if editing.id}
        <button class="btn" onclick={() => editing && run(backend.deleteSchedule(editing.id))}>Delete</button>
      {:else}
        <button class="btn" onclick={() => (editing = null)}>Cancel</button>
      {/if}
      <button class="btn primary" onclick={() => editing && run(backend.saveSchedule(editing))}>Save</button>
    </div>
  </Sheet>
{:else}
  <Sheet title="Schedule" {onclose}>
    <div class="list">
      {#each schedules as schedule (schedule.id)}
        <div class="row" class:off={!schedule.enabled}>
          <button class="what" onclick={() => edit(schedule)}>
            <strong>{timeText(schedule.start)} – {timeText(schedule.end)}</strong>
            <small>{daysText(schedule.days)}</small>
          </button>
          <input
            class="toggle"
            type="checkbox"
            aria-label="{timeText(schedule.start)} focus time on"
            checked={schedule.enabled}
            onchange={(e) => toggle(schedule, e.currentTarget.checked)}
          />
        </div>
      {:else}
        <p class="none">No focus times yet.</p>
      {/each}
    </div>
    <div class="actions">
      <button class="btn" onclick={add}>+ Focus time</button>
      <button class="btn primary" data-autofocus onclick={onclose}>Done</button>
    </div>
  </Sheet>
{/if}

<style>
  .list {
    margin-top: 12px;
    background: var(--surface);
    border: 1px solid var(--line-soft);
    border-radius: 16px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 14px 0 0;
    border-bottom: 1px solid #efe6d6;
  }
  .row:last-child {
    border-bottom: 0;
  }
  .what {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    padding: 10px 0 10px 14px;
    border: 0;
    background: none;
    font: inherit;
    color: inherit;
    text-align: left;
  }
  .what strong {
    font-size: 14px;
    font-weight: 500;
    font-variant-numeric: tabular-nums;
  }
  .what small {
    font-size: 12px;
    color: var(--muted);
  }
  .off strong {
    color: var(--muted);
  }
  .none {
    padding: 14px;
    font-size: 13px;
    color: var(--muted);
  }

  .days {
    margin-top: 14px;
    display: grid;
    grid-template-columns: repeat(7, minmax(0, 1fr));
    gap: 6px;
  }
  /* As the duration buttons on the Focus screen. */
  .day {
    height: 44px;
    border-radius: 12px;
    border: 1px solid var(--line);
    background: transparent;
    font: inherit;
    font-size: 14px;
    color: var(--text);
  }
  .day[aria-pressed='true'] {
    border-color: transparent;
    box-shadow: inset 0 0 0 1.5px var(--matcha);
    background: var(--matcha-tint);
    color: var(--matcha-deep);
    font-weight: 600;
  }
  .times {
    margin-top: 12px;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .times label {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .times .eyebrow {
    padding-left: 4px;
  }
  input[type='time'] {
    height: 44px;
    box-sizing: border-box;
    padding: 0 12px;
    border: 1px solid var(--line-strong);
    border-radius: 12px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 15px;
    font-variant-numeric: tabular-nums;
  }
  /* The system's time field, in Niwa's colours. */
  input[type='time']::-webkit-datetime-edit-hour-field:focus,
  input[type='time']::-webkit-datetime-edit-minute-field:focus,
  input[type='time']::-webkit-datetime-edit-ampm-field:focus {
    background: var(--matcha-tint);
    color: var(--matcha-deep);
    border-radius: 4px;
  }
  input[type='time']::-webkit-calendar-picker-indicator {
    opacity: 0.45;
    cursor: pointer;
  }
  .length {
    margin: 8px 4px 0;
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
  }
  .error {
    margin: 6px 4px 0;
    font-size: 12px;
    color: var(--clay-deep);
  }
  /* As the Settings sheet's. */
  .actions {
    margin-top: 18px;
    display: flex;
    gap: 8px;
  }
  .actions .btn {
    flex: 1;
  }
  .actions .btn.primary {
    flex: 1.4;
  }
</style>
