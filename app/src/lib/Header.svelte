<script lang="ts">
  import { fade } from 'svelte/transition';
  import type { Update } from './api';

  interface Props {
    /** Greys the logo on the Withered screen. */
    muted?: boolean;
    /** A newer version to offer, if there is one. */
    update?: Update | null;
    onupdate?: () => void;
    onsettings: () => void;
    onminimize: () => void;
    onclose: () => void;
  }

  let { muted = false, update = null, onupdate, onsettings, onminimize, onclose }: Props = $props();
</script>

<!-- The header is the window's title bar: drag it to move the card. -->
<header data-tauri-drag-region>
  <div class="brand">
    <!-- The 庭 stamp, as on the app icon. -->
    <svg class="stamp" width="22" height="22" viewBox="0 0 24 24" aria-hidden="true">
      <g transform="rotate(-4 12 12)" fill={muted ? '#8A7560' : '#4E6A35'}>
        <path fill-rule="evenodd" d="M5.4 2.4 H18.6 Q21.6 2.4 21.6 5.4 V18.6 Q21.6 21.6 18.6 21.6 H5.4 Q2.4 21.6 2.4 18.6 V5.4 Q2.4 2.4 5.4 2.4 Z M6 4.1 H18 Q19.9 4.1 19.9 6 V18 Q19.9 19.9 18 19.9 H6 Q4.1 19.9 4.1 18 V6 Q4.1 4.1 6 4.1 Z" />
        <text x="12" y="16.9" text-anchor="middle">庭</text>
      </g>
    </svg>
    <span class="display">niwa</span>
  </div>
  <div class="controls">
    {#if update}
      <button class="update" aria-label="Update to Niwa {update.version}" onclick={onupdate} transition:fade={{ duration: 200 }}>
        <span class="dot" aria-hidden="true"></span>Update
      </button>
    {/if}
    <button aria-label="Settings" onclick={onsettings}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3" /><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" /></svg>
    </button>
    <button aria-label="Minimize to the mini timer" onclick={onminimize}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M6 12h12" /></svg>
    </button>
    <button aria-label="Close" onclick={onclose}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true"><path d="M7 7l10 10M17 7L7 17" /></svg>
    </button>
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 12px 10px 20px;
    flex-shrink: 0;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    /* Clicks fall through to the header so the whole bar drags the window. */
    pointer-events: none;
  }
  .stamp text {
    font-family: 'Yu Gothic', 'YuGothic', 'Meiryo', sans-serif;
    font-weight: 700;
    font-size: 13.5px;
  }
  .brand span {
    font-size: 21px;
    font-weight: 500;
  }
  .controls {
    display: flex;
    gap: 2px;
  }
  button {
    width: 40px;
    height: 40px;
    border: 0;
    background: transparent;
    border-radius: 10px;
    color: var(--muted);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  button:hover {
    background: var(--panel);
  }
  .update {
    width: auto;
    height: 28px;
    align-self: center;
    margin-right: 4px;
    padding: 0 11px 0 9px;
    gap: 6px;
    border-radius: 14px;
    background: var(--matcha-tint);
    color: var(--matcha-deep);
    font-size: 12px;
    font-weight: 600;
  }
  .update:hover {
    background: var(--matcha-tint);
    box-shadow: inset 0 0 0 1px var(--matcha);
  }
  .update .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--matcha);
  }
</style>
