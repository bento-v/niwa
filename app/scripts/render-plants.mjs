// Renders plant drawings to a PNG sheet for review, one row of five stages per species:
//   node scripts/render-plants.mjs succulent,fern out.png [cell size]
// Useful when the browser can't take screenshots. 'withered' draws the withered plant.
import { writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { Resvg } = require('@resvg/resvg-js');
const m = await import(new URL('../src/lib/plants.ts', import.meta.url).href);
const [ids, out, cellArg] = process.argv.slice(2);
const cell = Number(cellArg ?? 200);
const list = ids.split(',');
const w = cell, h = Math.round((cell * 72) / 80), gap = 8;
let body = '';
list.forEach((id, row) => {
  for (let s = 1; s <= 5; s++) {
    const x = gap + (s - 1) * (w + gap), y = gap + row * (h + gap);
    const inner = id === 'withered' ? m.plantMarkup('pine', 5, { withered: true }) : m.plantMarkup(id, s, { detailed: true });
    body += `<rect x="${x}" y="${y}" width="${w}" height="${h}" rx="10" fill="#EFE7D9"/><svg x="${x}" y="${y}" width="${w}" height="${h}" viewBox="0 0 80 72">${inner}</svg>`;
  }
});
const W = gap + 5 * (w + gap), H = gap + list.length * (h + gap);
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}"><rect width="${W}" height="${H}" fill="#F6F0E4"/>${body}</svg>`;
writeFileSync(out, new Resvg(svg).render().asPng());
console.log('wrote', out, W, H);
