<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { backend, type Ended, type Overview, type Schedule, type Session, type Settings, type Update } from './lib/api';
  import { playChime } from './lib/chime';
  import { closedFeed } from './lib/closed.svelte';
  import { clock, exitWaitMinutes, lengthInWords } from './lib/format';
  import Header from './lib/Header.svelte';
  import ResizeGrips from './lib/ResizeGrips.svelte';
  import { reduced, setReduced, settle, travel } from './lib/motion.svelte';
  import type { SpeciesId } from './lib/plants';
  import Sheet from './lib/Sheet.svelte';
  import Tabs, { TABS, type Tab } from './lib/Tabs.svelte';
  import ArtSheet from './screens/ArtSheet.svelte';
  import Blocks from './screens/Blocks.svelte';
  import Focus from './screens/Focus.svelte';
  import Garden from './screens/Garden.svelte';
  import Result from './screens/Result.svelte';
  import ScheduleSheet from './screens/ScheduleSheet.svelte';
  import Start from './screens/Start.svelte';
  import Stats from './screens/Stats.svelte';

  let overview = $state<Overview | null>(null);
  let session = $state<Session | null>(null);
  let ended = $state<Ended | null>(null);
  let tab = $state<Tab>('focus');
  /** Which side the next screen drifts in from: the side of the tab moved toward. */
  let enterX = $state(0);

  function go(next: Tab) {
    enterX = Math.sign(TABS.indexOf(next) - TABS.indexOf(tab)) * 10;
    tab = next;
    if (next === 'focus') unseen = false;
  }

  // When Niwa closes an app: the Focus screen notes it in the scene; on any
  // other tab a note rises from the bottom, and the Focus tab keeps a dot
  // until it's next visited.
  let unseen = $state(false);
  const closed = closedFeed(() => {
    if (tab !== 'focus') unseen = true;
  });
  const noting = $derived(session !== null && !ended && tab !== 'focus' && closed.labels.length > 0);
  let startError = $state('');
  /** The confirmation for ending the session early, when it's open. */
  let asking = $state<'request-exit' | null>(null);
  /** The session waiting to be confirmed before it starts. */
  let confirming = $state<{ minutes: number; species: SpeciesId } | null>(null);
  let actionError = $state('');
  let settings = $state<Settings | null>(null);
  let showingSettings = $state(false);
  backend.settings().then((loaded) => (settings = loaded));

  async function change(key: 'sound' | 'startWithWindows' | 'checkUpdates', on: boolean) {
    if (!settings) return;
    const before = settings[key];
    settings[key] = on;
    try {
      if (key === 'sound') await backend.setSound(on);
      else if (key === 'startWithWindows') await backend.setStartWithWindows(on);
      else await backend.setCheckUpdates(on);
    } catch {
      settings[key] = before;
    }
  }

  let schedules = $state<Schedule[]>([]);
  let showingSchedule = $state(false);
  backend.schedules().then((loaded) => (schedules = loaded));

  // A newer version, offered outside sessions only.
  let update = $state<Update | null>(null);
  let showingUpdate = $state(false);
  let updating = $state(false);
  let updateError = $state('');
  backend.update().then((found) => (update = found));
  backend.on('update-ready', (found) => (update = found));

  async function installUpdate() {
    updating = true;
    updateError = '';
    try {
      // On success Niwa exits here and the installer takes over.
      await backend.installUpdate();
    } catch (reason) {
      updateError = String(reason);
    }
    updating = false;
  }

  async function openSettings() {
    // "Start with Windows" can be changed outside Niwa too, so read the settings fresh.
    settings = await backend.settings();
    showingSettings = true;
  }

  /** Loads the overview. If that fails (Niwa still starting up, say), it
   *  tries again, a little later each time, rather than leave the card empty. */
  async function refresh(attempt = 0) {
    try {
      overview = await backend.overview();
      session = overview.session;
    } catch {
      setTimeout(() => refresh(attempt + 1), Math.min(4000, 250 * 2 ** attempt));
    }
  }
  refresh();

  backend.on('session-tick', (tick) => (session = tick));
  backend.on('session-ended', (result) => {
    session = null;
    asking = null;
    closed.clear();
    unseen = false;
    ended = result;
    enterX = 0;
    tab = 'focus';
    if (result.outcome === 'grown' && settings?.sound !== false) playChime();
    refresh();
  });
  backend.on('close-requested', close);

  /** "Plant & start" asks first; the session starts once it's confirmed. */
  function start(minutes: number, species: SpeciesId) {
    startError = '';
    confirming = { minutes, species };
  }

  async function confirmedStart() {
    if (!confirming) return;
    const { minutes, species } = confirming;
    confirming = null;
    try {
      session = await backend.start(minutes, species);
    } catch (reason) {
      startError = String(reason);
    }
  }

  function close() {
    // A session only ends early through its wait, so closing mid-session just
    // tucks the card away; the session carries on.
    if (session) backend.hide();
    else backend.quit();
  }

  async function confirmed() {
    asking = null;
    actionError = '';
    try {
      await backend.requestExit();
    } catch (reason) {
      actionError = String(reason);
    }
  }

  async function cancelExit() {
    actionError = '';
    try {
      await backend.cancelExit();
    } catch (reason) {
      actionError = String(reason);
    }
  }

  function leaveResult(next: Tab) {
    ended = null;
    enterX = 0;
    tab = next;
  }

  const plant = $derived(session?.speciesName.toLowerCase() ?? 'plant');
  // The art sheet is a design aid, only reachable in development builds.
  const showArt = import.meta.env.DEV && location.hash === '#art';
</script>

{#if showArt}
  <ArtSheet />
{:else}
  <main
    class="card"
    style:background={ended?.outcome === 'withered' ? '#F3EEE6' : undefined}
    style:--enter-x="{enterX}px"
  >
    <Header
      muted={ended?.outcome === 'withered'}
      update={!session && !ended ? update : null}
      onupdate={() => (showingUpdate = true)}
      onsettings={openSettings}
      onminimize={() => backend.hide()}
      onclose={close}
    />

    {#if !overview}
      <!-- The first load takes a moment; an empty card is calmer than a spinner. -->
    {:else if ended}
      <Result {ended} ongarden={() => leaveResult('garden')} onagain={() => leaveResult('focus')} />
    {:else}
      <Tabs {tab} marked={unseen && session !== null ? 'focus' : null} onselect={go} />
      {#if tab === 'focus' && session}
        <Focus
          {session}
          nudge={closed.labels}
          error={actionError}
          onrequestexit={() => (asking = 'request-exit')}
          oncancelexit={cancelExit}
        />
      {:else if tab === 'focus'}
        <Start
          {overview}
          error={startError}
          onstart={start}
          onblocks={() => go('blocks')}
          {schedules}
          onschedule={() => (showingSchedule = true)}
        />
      {:else if tab === 'garden'}
        <Garden onplant={() => go('focus')} />
      {:else if tab === 'stats'}
        <Stats />
      {:else}
        <Blocks
          list={overview.list}
          locked={session !== null}
          strictAvailable={overview.strictAvailable}
          onchange={(list) => overview && (overview.list = list)}
        />
      {/if}
    {/if}

    {#if noting}
      <div
        class="closed-note"
        role="status"
        in:fly={{ y: travel(12), duration: 380, easing: settle }}
        out:fade={{ duration: 260 }}
      >
        <span class="dot" aria-hidden="true"></span>
        <span><strong>{closed.text}</strong> {closed.labels.length === 1 ? 'was' : 'were'} closed</span>
      </div>
    {/if}

    {#if showingSettings && settings}
      <Sheet title="Settings" onclose={() => (showingSettings = false)}>
        <div class="settings">
          <label>
            <span>
              <strong>Start with Windows</strong>
              <small>Opens to the mini timer when you sign in</small>
            </span>
            <input class="toggle" type="checkbox" checked={settings.startWithWindows} onchange={(e) => change('startWithWindows', e.currentTarget.checked)} />
          </label>
          <label>
            <span>
              <strong>Sound</strong>
              <small>A soft chime when a plant is fully grown</small>
            </span>
            <input class="toggle" type="checkbox" checked={settings.sound} onchange={(e) => change('sound', e.currentTarget.checked)} />
          </label>
          <label>
            <span>
              <strong>Check for updates</strong>
              <small>Once a day</small>
            </span>
            <input class="toggle" type="checkbox" checked={settings.checkUpdates} onchange={(e) => change('checkUpdates', e.currentTarget.checked)} />
          </label>
          <label>
            <span>
              <strong>Reduce motion</strong>
              <small>Fades in place of movement</small>
            </span>
            <input class="toggle" type="checkbox" checked={reduced()} onchange={(e) => setReduced(e.currentTarget.checked)} />
          </label>
        </div>
        <div class="choices">
          <button class="btn primary" data-autofocus onclick={() => (showingSettings = false)}>Done</button>
        </div>
      </Sheet>
    {:else if showingSchedule}
      <ScheduleSheet {schedules} onchange={(changed) => (schedules = changed)} onclose={() => (showingSchedule = false)} />
    {:else if showingUpdate && update && !session}
      <Sheet title="Update to Niwa {update.version}?" onclose={() => !updating && (showingUpdate = false)}>
        <p class="consequence">
          Niwa closes while it updates and opens again after.{update.asksAdmin ? ' Windows asks to allow the change.' : ''}
        </p>
        {#if updateError}
          <p class="consequence failed" role="alert">{updateError}</p>
        {/if}
        <div class="choices">
          <button class="btn" disabled={updating} onclick={() => (showingUpdate = false)}>Later</button>
          <button class="btn primary" data-autofocus disabled={updating} onclick={installUpdate}>
            {updating ? 'Downloading…' : 'Update'}
          </button>
        </div>
      </Sheet>
    {:else if asking === 'request-exit' && session}
      <Sheet title="End this session early?" onclose={() => (asking = null)}>
        <p class="consequence">
          {session.mode === 'strict' ? 'Strict' : 'Gentle'} mode asks you to wait
          {lengthInWords(exitWaitMinutes(session.mode, session.plannedMinutes))} first. Blocking carries on
          until then, and you can change your mind. When the wait ends, your {plant} withers.
        </p>
        <div class="choices">
          <button class="btn danger" onclick={confirmed}>Start the wait</button>
          <button class="btn primary" data-autofocus onclick={() => (asking = null)}>Keep growing</button>
        </div>
      </Sheet>
    {:else if confirming && overview}
      {@const seed = overview.species.find((s) => s.id === confirming?.species)?.name.toLowerCase() ?? 'plant'}
      {@const list = overview.list}
      <Sheet title="Plant a {seed} for {lengthInWords(confirming.minutes)}?" onclose={() => (confirming = null)}>
        <p class="consequence">
          <strong>{list.name}</strong> blocks {list.summary.replace(/^Strict · /, '')}.
          Ending early takes {lengthInWords(exitWaitMinutes(list.config.mode, confirming.minutes))}, and your
          {seed} withers.
        </p>
        <div class="choices">
          <button class="btn" onclick={() => (confirming = null)}>Not yet</button>
          <button class="btn primary" data-autofocus onclick={confirmedStart}>Plant &amp; start</button>
        </div>
      </Sheet>
    {/if}
    <ResizeGrips />
  </main>
{/if}

<style>
  .settings {
    margin-top: 12px;
    background: var(--surface);
    border: 1px solid var(--line-soft);
    border-radius: 16px;
  }
  .settings label {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
    border-bottom: 1px solid #efe6d6;
    cursor: pointer;
  }
  .settings label:last-child {
    border-bottom: 0;
  }
  .settings span {
    flex: 1;
    display: flex;
    flex-direction: column;
  }
  .settings strong {
    font-size: 14px;
    font-weight: 500;
  }
  .settings small {
    font-size: 12px;
    color: var(--muted);
  }
  /* A note that rises from the bottom of the card when Niwa closes an app. */
  .closed-note {
    position: absolute;
    left: 24px;
    right: 24px;
    bottom: 18px;
    z-index: 5;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 10px 14px;
    border-radius: 14px;
    background: var(--surface);
    box-shadow:
      0 8px 22px rgba(62, 50, 38, 0.16),
      0 1px 3px rgba(62, 50, 38, 0.1);
    font-size: 12px;
    line-height: 1.4;
    color: #5a4836;
    pointer-events: none;
  }
  .closed-note strong {
    color: var(--text);
    font-weight: 600;
  }
  .closed-note .dot {
    width: 7px;
    height: 7px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--clay);
  }
  .consequence {
    margin: 8px 4px 0;
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .consequence.failed {
    color: var(--clay-deep);
  }
  .choices {
    margin-top: 18px;
    display: flex;
    gap: 8px;
  }
  .choices .btn {
    flex: 1;
  }
  .choices .btn.primary {
    flex: 1.4;
  }
</style>
