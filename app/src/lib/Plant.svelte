<script lang="ts">
  import { plantMarkup, type SpeciesId } from './plants';

  interface Props {
    species: SpeciesId;
    stage?: number;
    withered?: boolean;
    /** Adds the ground shadow and rim shine; for sizes where they'd show. */
    detailed?: boolean;
    width?: number;
    /** Read out by screen readers. Leave empty for decorative plants. */
    label?: string;
    /** Crop close around the pot and plant, for tight spots like the mini timer. */
    tight?: boolean;
  }

  let { species, stage = 5, withered = false, detailed = false, width = 56, label = '', tight = false }: Props =
    $props();

  // The drawing's box, or a crop of it that keeps every species' tallest stage.
  const box = $derived(tight ? { x: 12, y: 4, w: 56, h: 68 } : { x: 0, y: 0, w: 80, h: 72 });
</script>

<svg
  {width}
  height={Math.round((width * box.h) / box.w)}
  viewBox="{box.x} {box.y} {box.w} {box.h}"
  role={label ? 'img' : undefined}
  aria-label={label || undefined}
  aria-hidden={label ? undefined : 'true'}
>
  {@html plantMarkup(species, stage, { withered, detailed })}
</svg>

<style>
  svg {
    display: block;
    flex-shrink: 0;
  }
</style>
