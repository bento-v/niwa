import '@fontsource-variable/figtree';
import '@fontsource-variable/fraunces/full.css';
import { mount, unmount } from 'svelte';
import './app.css';
import App from './App.svelte';
import { inTauri, windowLabel } from './lib/api';
// Applies Settings > Reduce motion to whichever window this is.
import './lib/motion.svelte';
import Mini from './Mini.svelte';

// The same page runs in both windows: the card, and the mini timer it
// collapses into.
const mini = windowLabel === 'mini';
document.documentElement.classList.toggle('in-browser', !inTauri);
document.documentElement.classList.toggle('mini', mini);
// `?size=520x760` tries the card at another size in a browser.
const size = new URLSearchParams(location.search).get('size')?.match(/^(\d+)x(\d+)$/);
if (size) {
  document.documentElement.style.setProperty('--card-width', `${size[1]}px`);
  document.documentElement.style.setProperty('--card-height', `${size[2]}px`);
}

const target = document.getElementById('app')!;
// A development hot reload can run this file again; start from an empty page.
target.replaceChildren();
const app = mount(mini ? Mini : App, { target });

// During development, a hot reload of this file replaces the card rather than
// adding a second one beside it.
import.meta.hot?.dispose(() => unmount(app));
