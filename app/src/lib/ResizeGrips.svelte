<script lang="ts">
  import { backend, inTauri, type ResizeDirection } from './api';

  // Invisible strips along the card's edges and corners that resize the
  // window. The window's own edges sit out in the transparent margin around
  // the card (room for its shadow), where nobody would think to grab.

  const GRIPS: { direction: ResizeDirection; place: string }[] = [
    { direction: 'North', place: 'n' },
    { direction: 'South', place: 's' },
    { direction: 'East', place: 'e' },
    { direction: 'West', place: 'w' },
    { direction: 'NorthWest', place: 'nw' },
    { direction: 'NorthEast', place: 'ne' },
    { direction: 'SouthWest', place: 'sw' },
    { direction: 'SouthEast', place: 'se' },
  ];

  function onpointerdown(event: PointerEvent, direction: ResizeDirection) {
    if (event.button !== 0) return;
    event.preventDefault();
    backend.resizeCard(direction);
  }
</script>

{#if inTauri}
  {#each GRIPS as grip (grip.place)}
    <div class="grip {grip.place}" aria-hidden="true" onpointerdown={(e) => onpointerdown(e, grip.direction)}></div>
  {/each}
{/if}

<style>
  .grip {
    position: absolute;
    z-index: 50;
  }
  /* Edges: thin, so they don't take clicks from what's inside. The right one
     is thinnest, beside the scrollbars. */
  .n,
  .s {
    left: 18px;
    right: 18px;
    height: 5px;
  }
  .n {
    top: 0;
    cursor: ns-resize;
  }
  .s {
    bottom: 0;
    cursor: ns-resize;
  }
  .e,
  .w {
    top: 18px;
    bottom: 18px;
  }
  .e {
    right: 0;
    width: 3px;
    cursor: ew-resize;
  }
  .w {
    left: 0;
    width: 5px;
    cursor: ew-resize;
  }
  /* Corners: the rounded corners hold nothing else, so they can be generous. */
  .nw,
  .ne,
  .sw,
  .se {
    width: 18px;
    height: 18px;
  }
  .nw {
    top: 0;
    left: 0;
    cursor: nwse-resize;
  }
  .se {
    bottom: 0;
    right: 0;
    cursor: nwse-resize;
  }
  .ne {
    top: 0;
    right: 0;
    cursor: nesw-resize;
  }
  .sw {
    bottom: 0;
    left: 0;
    cursor: nesw-resize;
  }
</style>
