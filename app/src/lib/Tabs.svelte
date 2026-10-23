<script lang="ts" module>
  export const TABS = ['focus', 'garden', 'stats', 'blocks'] as const;
  export type Tab = (typeof TABS)[number];
</script>

<script lang="ts">
  interface Props {
    tab: Tab;
    /** A tab with news waiting, shown by a small dot: Focus, after Niwa closed an app. */
    marked?: Tab | null;
    onselect: (tab: Tab) => void;
  }

  let { tab, marked = null, onselect }: Props = $props();

  const LABELS: Record<Tab, string> = {
    focus: 'Focus',
    garden: 'Garden',
    stats: 'Stats',
    blocks: 'Blocks',
  };
  const index = $derived(TABS.indexOf(tab));
</script>

<nav aria-label="Sections">
  <div class="thumb" aria-hidden="true" style:transform="translateX(calc({index * 100}% + {index * 4}px))"></div>
  {#each TABS as name}
    <button
      aria-current={name === tab ? 'page' : undefined}
      aria-label={marked === name ? `${LABELS[name]}, an app was closed` : undefined}
      onclick={() => onselect(name)}
    >
      {LABELS[name]}
      {#if marked === name}
        <span class="mark" aria-hidden="true"></span>
      {/if}
    </button>
  {/each}
</nav>

<style>
  nav {
    position: relative;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 4px;
    margin: 0 16px;
    padding: 4px;
    background: var(--panel);
    border-radius: 14px;
    flex-shrink: 0;
  }
  .thumb {
    position: absolute;
    top: 4px;
    left: 4px;
    width: calc((100% - 20px) / 4);
    height: calc(100% - 8px);
    border-radius: 10px;
    background: var(--surface);
    box-shadow: 0 1px 2px rgba(62, 50, 38, 0.12);
    transition: transform 0.35s var(--ease);
  }
  button {
    position: relative;
    padding: 10px 0;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--muted);
    font-size: 13px;
    font-weight: 500;
  }
  .mark {
    position: absolute;
    top: 7px;
    right: 10px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--clay);
    animation: mark-in 0.5s var(--ease-settle);
  }
  @keyframes mark-in {
    from {
      opacity: 0;
      transform: scale(calc(1 - var(--travel)));
    }
  }
  button[aria-current='page'] {
    color: var(--matcha-deep);
    font-weight: 600;
  }
</style>
