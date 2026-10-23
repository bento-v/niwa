/**
 * Arrow-key movement for a row of `role="radio"` buttons, as a radio group
 * should have: left/right (or up/down) move to the next choice that isn't
 * disabled and select it, wrapping at the ends.
 */
export function radioKeys(group: HTMLElement) {
  function onkeydown(event: KeyboardEvent) {
    const step =
      event.key === 'ArrowRight' || event.key === 'ArrowDown'
        ? 1
        : event.key === 'ArrowLeft' || event.key === 'ArrowUp'
          ? -1
          : 0;
    if (step === 0) return;

    const radios = [...group.querySelectorAll<HTMLElement>('[role="radio"]')].filter(
      (radio) => radio.getAttribute('aria-disabled') !== 'true',
    );
    const current = radios.indexOf(document.activeElement as HTMLElement);
    if (current === -1) return;

    event.preventDefault();
    const next = radios[(current + step + radios.length) % radios.length];
    next.focus();
    next.click();
  }

  group.addEventListener('keydown', onkeydown);
  return { destroy: () => group.removeEventListener('keydown', onkeydown) };
}
