/**
 * Builds the one-page comparison of the icon drafts, so the choice can be made
 * by looking rather than by reading descriptions.
 *
 *   pnpm icons:renders
 *   node scripts/icon-options-sheet.mjs            # -> target/icon-renders/options.png
 *
 * Each column shows one draft at the 1024 artboard, at one Dock tile on a
 * Retina display (128 px), at the 32 px size the Settings sidebar and Finder
 * lists use, and at real menu-bar scale. The dark strip is there because draft C
 * is a dark tile and draft A and B are paper.
 */
import { chromium } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const DIR = resolve(import.meta.dirname, "..", "target/icon-renders");
const OUT = resolve(DIR, "options.png");

const DRAFTS = [
  [
    "a",
    "A · Ember wave",
    "Five ink bars, the middle one ember. The same glyph as the menu-bar icon.",
  ],
  [
    "b",
    "B · Speech to caret",
    "An ink speech bubble ending in an ember caret. Speech in, text out.",
  ],
  [
    "c",
    "C · Ember d",
    "A lowercase d on charcoal: an ember bowl, a pen-stroke stem.",
  ],
];

const data = (file) =>
  `data:image/png;base64,${readFileSync(resolve(DIR, file)).toString("base64")}`;

const column = ([id, name, note]) => `
  <section>
    <h2>${name}</h2>
    <div class="light strip">
      <figure><img src="${data(`${id}-1024.png`)}" width="200"><figcaption>1024 · the artboard</figcaption></figure>
      <figure><img src="${data(`${id}-dock-128.png`)}" width="128"><figcaption>128 · Dock, Retina</figcaption></figure>
      <figure><img src="${data(`${id}-small-32.png`)}" width="32"><figcaption>32 · sidebar</figcaption></figure>
      <figure><img src="${data(`${id}-small-32.png`)}" width="16"><figcaption>16</figcaption></figure>
    </div>
    <div class="dark strip">
      <figure><img src="${data(`${id}-dock-128.png`)}" width="128"><figcaption>128 on charcoal</figcaption></figure>
      <figure><img src="${data(`${id}-small-32.png`)}" width="32"><figcaption>32</figcaption></figure>
    </div>
    <p>${note}</p>
  </section>`;

const html = `<!doctype html><meta charset="utf-8"><body>
<style>
  body { margin: 0; padding: 32px; font: 13px/1.5 -apple-system, system-ui, sans-serif;
         background: #f5f3f0; color: #1c1a18; }
  h1 { font-size: 20px; margin: 0 0 4px; }
  .sub { color: #635d54; margin: 0 0 28px; }
  main { display: grid; grid-template-columns: repeat(3, 1fr); gap: 24px; }
  section { background: #fffefe; border: 1px solid rgba(28,26,24,.1); border-radius: 14px;
            padding: 20px; }
  h2 { font-size: 14px; margin: 0 0 16px; }
  .strip { display: flex; align-items: end; gap: 20px; padding: 16px; border-radius: 10px; }
  .light { background: #fbfaf8; border: 1px solid rgba(28,26,24,.07); }
  .dark { background: #1a1816; margin-top: 12px; }
  .dark figcaption { color: #b9b3a9; }
  figure { margin: 0; text-align: center; }
  figcaption { font-size: 10px; color: #635d54; margin-top: 6px; white-space: nowrap; }
  p { margin: 14px 0 0; color: #635d54; font-size: 12px; }
</style>
<h1>dictate · Dock icon drafts</h1>
<p class="sub">Rendered from docs/icon-drafts through <code>tauri icon</code>, so these are the pixels the bundle gets.</p>
<main>${DRAFTS.map(column).join("")}</main>
</body>`;

const browser = await chromium.launch();
const page = await browser.newPage({
  viewport: { width: 1180, height: 720 },
  deviceScaleFactor: 2,
});
await page.setContent(html);
await page.screenshot({ path: OUT, fullPage: true });
await browser.close();
console.log(`icons: comparison sheet at ${OUT}`);
