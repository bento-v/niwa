<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import { backend, type AppEntry, type BlockList, type BlockListConfig } from '../lib/api';
  import { settle, travel } from '../lib/motion.svelte';
  import { radioKeys } from '../lib/radiogroup';
  import Sheet from '../lib/Sheet.svelte';

  interface Props {
    list: BlockList;
    /** True while a session runs: the list can be read but not changed. */
    locked: boolean;
    /** Whether the Strict service is installed, which Strict mode needs. */
    strictAvailable: boolean;
    onchange: (list: BlockList) => void;
  }

  let { list, locked, strictAvailable, onchange }: Props = $props();

  const config = $derived(list.config);
  const strict = $derived(config.mode === 'strict');
  const allowOnly = $derived(config.kind === 'allowOnly');
  let editingAllowed = $state(false);
  let error = $state('');

  async function save(changes: Partial<BlockListConfig>) {
    error = '';
    try {
      onchange(await backend.saveList({ ...config, ...changes }));
    } catch (reason) {
      error = String(reason);
    }
  }

  /** A switch flips at once; if the list can't be saved, it slides back. */
  async function flip(input: HTMLInputElement, key: 'steam' | 'epic' | 'riot') {
    await save({ [key]: input.checked });
    input.checked = config[key];
  }
  async function flipCategory(input: HTMLInputElement, id: string) {
    const others = config.categories.filter((other) => other !== id);
    await save({ categories: input.checked ? [...others, id] : others });
    input.checked = config.categories.includes(id);
  }
  /** Allow-only starts from the work apps installed here, for the user to trim. */
  function chooseKind(kind: BlockListConfig['kind']) {
    if (locked || kind === config.kind) return;
    const allowed = kind === 'allowOnly' && config.allowed.length === 0 ? list.suggested : config.allowed;
    save({ kind, allowed });
  }

  const samePath = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();
  const sameApp = (a: AppEntry, b: AppEntry) =>
    a.known || b.known ? a.known === b.known : samePath(a.path, b.path);
  const key = (app: AppEntry) => app.known ?? app.path;

  async function addApp() {
    const app = await backend.pickApp(allowOnly);
    if (!app) return;
    const field = allowOnly ? 'allowed' : 'apps';
    if (!config[field].some((existing) => sameApp(existing, app))) {
      await save({ [field]: [...config[field], app] });
    }
  }
  function removeApp(app: AppEntry) {
    const field = allowOnly ? 'allowed' : 'apps';
    save({ [field]: config[field].filter((other) => other !== app) });
  }
  async function addAllowed(folder: boolean) {
    const entry = await backend.pickException(folder);
    if (entry && !config.exceptions.some((existing) => samePath(existing.path, entry.path))) {
      await save({ exceptions: [...config.exceptions, entry] });
    }
  }

  const found = (count: number) => (count === 0 ? 'none found' : `${count} found`);
  // Games are one category: Steam, Epic and Riot switch on and off together.
  const games = $derived(config.steam || config.epic || config.riot);
  const gamesFound = $derived(list.found.steam + list.found.epic + list.found.riot);
  async function flipGames(input: HTMLInputElement) {
    const on = input.checked;
    await save({ steam: on, epic: on, riot: on });
    input.checked = games;
  }
  const exceptions = $derived(config.exceptions.map((entry) => entry.name).join(', ') || 'nothing');
  const shownApps = $derived(allowOnly ? config.allowed : config.apps);
  /** Suggested work apps not on the allow list yet. */
  const suggestions = $derived(
    list.suggested.filter((app) => !config.allowed.some((existing) => sameApp(existing, app))),
  );
  /** One or two lines: what the list closes, then how firmly. */
  const note = $derived(
    (allowOnly ? 'Closes every app with a window that isn’t allowed. ' : '') +
      (strict ? 'Ending early takes a third of the session.' : 'Ending early takes 5 minutes.'),
  );

  // What would close if a session started now. It only looks; it's refreshed
  // when the list changes and every few seconds while the screen is in view.
  let closingNow = $state<string[] | null>(null);
  $effect(() => {
    if (locked) return;
    const snapshot = $state.snapshot(config) as BlockListConfig;
    let alive = true;
    const look = () => {
      // The card is hidden behind the mini timer: nobody's reading it.
      if (document.hidden) return;
      backend.previewClosing(snapshot).then((labels) => {
        if (alive) closingNow = labels;
      });
    };
    const soon = setTimeout(look, 250);
    const every = setInterval(look, 5000);
    return () => {
      alive = false;
      clearTimeout(soon);
      clearInterval(every);
    };
  });
  const closingText = $derived(
    closingNow === null
      ? ''
      : closingNow.length === 0
        ? 'Nothing open right now would close.'
        : `${closingNow.slice(0, 4).join(', ')}${closingNow.length > 4 ? ` and ${closingNow.length - 4} more` : ''}`,
  );
</script>

<div class="screen">
  <div class="title">
    <h1 class="display">{list.name}</h1>
    {#if locked}
      <span class="lock">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V8a4 4 0 0 1 8 0v3" /></svg>
        Locked during a session
      </span>
    {/if}
  </div>

  <div class="segmented modes" class:strict role="radiogroup" aria-label="Strictness" use:radioKeys>
    <div class="thumb" aria-hidden="true"></div>
    <button
      role="radio"
      aria-checked={!strict}
      aria-disabled={locked}
      tabindex={strict ? -1 : 0}
      onclick={() => !locked && strict && save({ mode: 'gentle' })}
    >
      Gentle
    </button>
    <button
      role="radio"
      aria-checked={strict}
      aria-disabled={locked || !strictAvailable}
      tabindex={strict ? 0 : -1}
      title={strictAvailable ? undefined : 'Strict mode needs the Niwa background service to be installed'}
      onclick={() => !locked && strictAvailable && !strict && save({ mode: 'strict' })}
    >
      Strict
      {#if !strictAvailable}
        <span class="soon">needs setup</span>
      {/if}
    </button>
  </div>

  <div class="segmented kinds" class:allow={allowOnly} role="radiogroup" aria-label="What the list does" use:radioKeys>
    <div class="thumb" aria-hidden="true"></div>
    <button
      role="radio"
      aria-checked={!allowOnly}
      aria-disabled={locked}
      tabindex={allowOnly ? -1 : 0}
      onclick={() => chooseKind('block')}
    >
      Block list
    </button>
    <button
      role="radio"
      aria-checked={allowOnly}
      aria-disabled={locked}
      tabindex={allowOnly ? 0 : -1}
      onclick={() => chooseKind('allowOnly')}
    >
      Allow only
    </button>
  </div>
  <p class="mode-note">{note}</p>

  <div class="body scroll">
    {#if !locked && closingText}
      <p class="preview" class:calm={closingNow?.length === 0} role="status" in:fade={{ duration: 300 }}>
        {#if closingNow?.length}
          <span>Would close now:</span>
          {closingText}
        {:else}
          {closingText}
        {/if}
      </p>
    {/if}

    {#key allowOnly}
      <div class="kind-body" in:fly={{ x: travel(allowOnly ? 12 : -12), duration: 360, easing: settle }}>
        {#if !allowOnly}
          <div class="eyebrow section">Categories</div>
          <div class="group">
            <label class="two-line" title="Steam, Epic and Riot games">
              <span>
                Games
                <small>Steam, Epic, Riot</small>
              </span>
              <small>{found(gamesFound)}</small>
              <input class="toggle" type="checkbox" checked={games} disabled={locked} onchange={(e) => flipGames(e.currentTarget)} />
            </label>
            {#each list.categories as category (category.id)}
              <label class="two-line" title={category.apps.join(', ')}>
                <span>
                  {category.label}
                  <small>{category.apps.join(', ')}</small>
                </span>
                <small>{found(category.found)}</small>
                <input
                  class="toggle"
                  type="checkbox"
                  checked={config.categories.includes(category.id)}
                  disabled={locked}
                  onchange={(e) => flipCategory(e.currentTarget, category.id)}
                />
              </label>
            {/each}
          </div>
        {/if}

        <div class="eyebrow section">{allowOnly ? 'Allowed apps' : 'Apps'}</div>
        <div class="apps">
          {#each shownApps as app (key(app))}
            <span class="app" class:keep={allowOnly} title={app.path || app.name} in:fade={{ duration: 220 }}>
              <span class="app-name">{app.name}</span>
              <button aria-label="Remove {app.name}" disabled={locked} onclick={() => removeApp(app)}>
                <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17" /></svg>
              </button>
            </span>
          {/each}
          <button class="add" disabled={locked} onclick={addApp}>+ Add app</button>
        </div>

        {#if allowOnly && suggestions.length > 0}
          <div class="eyebrow section">Suggested</div>
          <div class="apps">
            {#each suggestions as app (key(app))}
              <button
                class="suggestion"
                disabled={locked}
                aria-label="Allow {app.name}"
                onclick={() => save({ allowed: [...config.allowed, app] })}
              >
                + {app.name}
              </button>
            {/each}
          </div>
        {/if}

        {#if allowOnly}
          <p class="always">
            Windows, Niwa, game launchers and tray-only apps always stay open.
          </p>
        {/if}
      </div>
    {/key}

    {#if error}
      <p class="error" role="alert">{error}</p>
    {/if}

    <div class="allowed">
      <span>Always allowed: {exceptions}</span>
      <button class="text-button" disabled={locked} onclick={() => (editingAllowed = true)}>Edit</button>
    </div>
  </div>
</div>

{#if editingAllowed}
  <Sheet title="Always allowed" onclose={() => (editingAllowed = false)}>
    <p class="hint">These stay open even when they sit inside something you block.</p>
    <ul class="scroll">
      {#each config.exceptions as entry (entry.path)}
        <li>
          <span>
            <strong>{entry.name}</strong>
            <small title={entry.path}>{entry.path}</small>
          </span>
          <button
            aria-label="Stop allowing {entry.name}"
            onclick={() => save({ exceptions: config.exceptions.filter((other) => other !== entry) })}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17" /></svg>
          </button>
        </li>
      {:else}
        <li class="none">Nothing is allowed through yet.</li>
      {/each}
    </ul>
    <div class="sheet-actions">
      <button class="btn" onclick={() => addAllowed(false)}>+ App</button>
      <button class="btn" onclick={() => addAllowed(true)}>+ Folder</button>
      <button class="btn primary" data-autofocus onclick={() => (editingAllowed = false)}>Done</button>
    </div>
  </Sheet>
{/if}

<style>
  .title {
    padding: 12px 24px 0;
    min-height: 43px;
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  .title h1 {
    font-size: 24px;
    letter-spacing: 0;
  }
  .lock {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    color: var(--clay-deep);
  }

  /* Two-way switches: Gentle or Strict, and Block list or Allow only. */
  .segmented {
    position: relative;
    margin: 6px 16px 0;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 4px;
    padding: 4px;
    border: 1px solid var(--line);
    border-radius: 12px;
    flex-shrink: 0;
  }
  .segmented .thumb {
    position: absolute;
    top: 4px;
    left: 4px;
    width: calc((100% - 12px) / 2);
    height: calc(100% - 8px);
    border-radius: 9px;
    background: var(--matcha-tint);
    transition:
      transform 0.38s var(--ease-settle),
      background-color 0.35s ease;
  }
  .modes.strict .thumb,
  .kinds.allow .thumb {
    transform: translateX(calc(100% + 4px));
  }
  .modes.strict .thumb {
    background: var(--clay-tint);
  }
  .modes.strict button[aria-checked='true'] {
    color: var(--clay-deep);
  }
  /* The second switch is quieter: it shapes the list, not the session. */
  .kinds {
    margin-top: 6px;
    border-color: transparent;
    background: var(--panel);
  }
  .kinds .thumb {
    background: var(--surface);
    box-shadow: 0 1px 2px rgba(62, 50, 38, 0.12);
  }
  .mode-note {
    margin: 6px 24px 0;
    font-size: 12px;
    line-height: 1.4;
    color: var(--muted);
    min-height: 34px;
    flex-shrink: 0;
  }
  .segmented button {
    position: relative;
    height: 36px;
    border: 0;
    border-radius: 9px;
    background: transparent;
    color: var(--muted);
    font-size: 13px;
    font-weight: 600;
  }
  .kinds button {
    height: 32px;
  }
  .segmented button[aria-checked='true'] {
    color: var(--matcha-deep);
  }
  .segmented button[aria-disabled='true'] {
    cursor: not-allowed;
  }
  .segmented button[aria-disabled='true'][aria-checked='false'] {
    color: #9a8a76;
  }

  /* Everything under the switches scrolls; it fades out at the bottom edge. */
  .body {
    flex: 1;
    min-height: 0;
    margin-top: 4px;
    padding-bottom: 16px;
    mask-image: linear-gradient(to bottom, transparent, #000 8px, #000 calc(100% - 18px), transparent);
  }
  .preview {
    margin: 6px 16px 0;
    padding: 7px 12px;
    border-radius: 10px;
    background: var(--clay-tint);
    color: var(--clay-deep);
    font-size: 12px;
    line-height: 1.4;
  }
  .preview span {
    font-weight: 600;
  }
  .preview.calm {
    background: #efe6d6;
    color: #5a4836;
  }
  .always {
    margin: 10px 24px 0;
    font-size: 12px;
    line-height: 1.4;
    color: var(--muted);
  }
  .soon {
    margin-left: 4px;
    padding: 2px 6px;
    border-radius: 999px;
    background: var(--panel);
    font-size: 10px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .section {
    padding: 10px 24px 6px;
  }
  .group {
    margin: 0 16px;
    background: var(--surface);
    border: 1px solid var(--line-soft);
    border-radius: 16px;
    display: flex;
    flex-direction: column;
  }
  label {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    height: 44px;
    font-size: 14px;
    border-bottom: 1px solid #efe6d6;
    cursor: pointer;
  }
  label:last-child {
    border-bottom: 0;
  }
  label span {
    flex: 1;
    min-width: 0;
  }
  label.two-line {
    height: auto;
    min-height: 44px;
    padding-top: 7px;
    padding-bottom: 7px;
    box-sizing: border-box;
  }
  label.two-line small {
    display: block;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  label small {
    font-size: 12px;
    color: var(--muted);
  }
  .apps {
    padding: 0 16px;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .app {
    max-width: 100%;
    min-width: 0;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    gap: 2px;
    font-size: 13px;
    padding: 0 2px 0 12px;
    height: 34px;
    border-radius: 999px;
    background: var(--panel);
  }
  .app-name {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .app button {
    flex-shrink: 0;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .app.keep {
    background: var(--matcha-tint);
    color: var(--matcha-deep);
  }
  .suggestion {
    height: 30px;
    padding: 0 12px;
    border-radius: 999px;
    border: 1px dashed var(--line-strong);
    background: transparent;
    color: var(--muted);
    font-size: 12px;
    font-weight: 500;
  }
  .suggestion:not(:disabled):hover {
    border-color: var(--matcha);
    color: var(--matcha-deep);
  }
  .add {
    height: 34px;
    padding: 0 14px;
    border-radius: 999px;
    border: 1px dashed #b9a68c;
    background: transparent;
    color: var(--matcha);
    font-size: 13px;
    font-weight: 600;
  }
  button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .error {
    margin: 8px 20px 0;
    font-size: 12px;
    color: var(--clay-deep);
  }

  .allowed {
    margin: 10px 16px 0;
    padding: 4px 14px;
    border-radius: 12px;
    background: #efe6d6;
    font-size: 12px;
    color: #5a4836;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }
  .allowed span {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .allowed .text-button {
    font-size: 12px;
    padding: 6px 0;
    flex-shrink: 0;
  }

  .hint {
    margin: 6px 4px 12px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--muted);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 190px;
    background: var(--surface);
    border: 1px solid var(--line-soft);
    border-radius: 14px;
  }
  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 6px 8px 14px;
    border-bottom: 1px solid #efe6d6;
  }
  li:last-child {
    border-bottom: 0;
  }
  li span {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  li strong {
    font-size: 14px;
    font-weight: 600;
  }
  li small {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  li button {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    border: 0;
    border-radius: 50%;
    background: transparent;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  li.none {
    padding: 14px;
    font-size: 13px;
    color: var(--muted);
  }
  .sheet-actions {
    margin-top: 14px;
    display: flex;
    gap: 8px;
  }
  .sheet-actions .btn {
    flex: 1;
    height: 44px;
  }
</style>
