// Draws the app icon and the tray icon.
//
//   npm run icons
//
// Writes src-tauri/icons/app-icon.png (then run `npx tauri icon` on it for
// the Windows sizes) and src-tauri/icons/tray/stamp.png.
//
// Both are a hanko-style stamp of 庭 ("niwa", garden). The character is set
// in Yu Gothic Bold, which ships with Windows, so this script needs to run on
// Windows.

import { existsSync, mkdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Resvg } from '@resvg/resvg-js';

const icons = join(dirname(fileURLToPath(import.meta.url)), '..', 'src-tauri', 'icons');
mkdirSync(join(icons, 'tray'), { recursive: true });

const FONT = join(process.env.SystemRoot ?? 'C:\\Windows', 'Fonts', 'YuGothB.ttc');
if (!existsSync(FONT)) {
  console.error(`The icons need Yu Gothic Bold (${FONT}), which comes with Windows.`);
  process.exit(1);
}

function png(svg, size) {
  return new Resvg(svg, {
    fitTo: { mode: 'width', value: size },
    font: { fontFiles: [FONT], loadSystemFonts: false, defaultFontFamily: 'Yu Gothic' },
  })
    .render()
    .asPng();
}

/** The matcha green from the design tokens. */
const INK = '#4E6A35';

const FRAME =
  'M262 182 H762 Q842 182 842 262 V762 Q842 842 762 842 H262 Q182 842 182 762 V262 Q182 182 262 182 Z M282 232 H742 Q792 232 792 282 V742 Q792 792 742 792 H282 Q232 792 232 742 V282 Q232 232 282 232 Z';
const BOLD_FRAME =
  'M240 150 H784 Q874 150 874 240 V784 Q874 874 784 874 H240 Q150 874 150 784 V240 Q150 150 240 150 Z M268 226 H756 Q798 226 798 268 V756 Q798 798 756 798 H268 Q226 798 226 756 V268 Q226 226 268 226 Z';
const character = (stroke) =>
  `<text x="512" y="700" text-anchor="middle" font-family="Yu Gothic" font-weight="700" font-size="540" stroke="${INK}" stroke-width="${stroke}" stroke-linejoin="round" transform="translate(512 512) scale(1.06 1) translate(-512 -512)">庭</text>`;

// The look of a real stamp: edges that wander a little where ink met paper,
// a scattering of specks where it didn't take, and uneven pressure.
const STAMP_FILTER = `
  <filter id="stamp" x="-10%" y="-10%" width="120%" height="120%" color-interpolation-filters="sRGB">
    <feTurbulence type="fractalNoise" baseFrequency="0.035" numOctaves="2" seed="7" result="warp"/>
    <feDisplacementMap in="SourceGraphic" in2="warp" scale="9" xChannelSelector="R" yChannelSelector="G" result="rough"/>
    <feTurbulence type="fractalNoise" baseFrequency="0.4" numOctaves="3" seed="3" result="grain"/>
    <feColorMatrix in="grain" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  14 0 0 0 -1.6" result="specks"/>
    <feComposite in="rough" in2="specks" operator="in" result="speckled"/>
    <feTurbulence type="fractalNoise" baseFrequency="0.009" numOctaves="2" seed="11" result="pressure"/>
    <feColorMatrix in="pressure" type="matrix" values="0 0 0 0 0  0 0 0 0 0  0 0 0 0 0  0.8 0 0 0 0.58" result="press"/>
    <feComposite in="speckled" in2="press" operator="in"/>
  </filter>`;

// The app icon: the stamp on white, tilted as if pressed by hand.
const appIcon = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <defs>${STAMP_FILTER}</defs>
  <rect width="1024" height="1024" rx="228" fill="#FFFFFF"/>
  <g transform="rotate(-4 512 512)" filter="url(#stamp)" fill="${INK}">
    <path fill-rule="evenodd" d="${FRAME}"/>
    ${character(20)}
  </g>
</svg>`;
writeFileSync(join(icons, 'app-icon.png'), png(appIcon, 1024));

// The tray icon: the same stamp, upright, plain and a little bolder, since
// specks would only blur at sixteen pixels. The white tile keeps it clear on
// dark and light taskbars alike.
const trayIcon = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <rect width="1024" height="1024" rx="228" fill="#FFFFFF"/>
  <g fill="${INK}">
    <path fill-rule="evenodd" d="${BOLD_FRAME}"/>
    ${character(44)}
  </g>
</svg>`;
writeFileSync(join(icons, 'tray', 'stamp.png'), png(trayIcon, 64));

console.log(`Icons written to ${icons}`);
