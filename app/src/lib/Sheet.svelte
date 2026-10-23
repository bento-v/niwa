<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fade, fly } from 'svelte/transition';
  import { cubicIn } from 'svelte/easing';
  import { settle, travel } from './motion.svelte';

  interface Props {
    title: string;
    /** Called when the sheet is dismissed without choosing (scrim or Escape). */
    onclose: () => void;
    children: Snippet;
  }

  let { title, onclose, children }: Props = $props();

  let panel: HTMLDivElement;
  const uid = $props.id();

  // Keep the keyboard inside the sheet while it is open, and put focus back
  // where it was afterwards.
  $effect(() => {
    const before = document.activeElement as HTMLElement | null;
    panel.querySelector<HTMLElement>('[data-autofocus], button')?.focus();
    return () => before?.focus();
  });

  function onkeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      onclose();
    }
    if (event.key === 'Tab') {
      const stops = [...panel.querySelectorAll<HTMLElement>('button:not(:disabled), input')];
      const first = stops[0];
      const last = stops[stops.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }
  }
</script>

<!-- A sheet rises from the bottom of the card and leaves the same way, a little
     quicker on the way out. With motion reduced it fades in place. -->
<div class="scrim" in:fade={{ duration: 260 }} out:fade={{ duration: 200 }} onclick={onclose} aria-hidden="true"></div>
<div
  class="sheet"
  role="dialog"
  aria-modal="true"
  aria-labelledby="{uid}-title"
  tabindex="-1"
  bind:this={panel}
  {onkeydown}
  in:fly={{ y: travel(280), duration: 420, easing: settle, opacity: travel(1) ? 0.6 : 0 }}
  out:fly={{ y: travel(280), duration: 220, easing: cubicIn, opacity: travel(1) ? 1 : 0 }}
>
  <h1 id="{uid}-title" class="display">{title}</h1>
  {@render children()}
</div>

<style>
  .scrim {
    position: absolute;
    inset: 0;
    background: rgba(62, 50, 38, 0.32);
    z-index: 10;
  }
  .sheet {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 11;
    padding: 22px 20px 16px;
    background: var(--card);
    border-radius: 22px 22px 0 0;
    box-shadow: 0 -8px 24px rgba(62, 50, 38, 0.16);
    outline: none;
  }
  h1 {
    font-size: 22px;
    padding: 0 4px;
  }
</style>
