import { chromium } from "@playwright/test";
import { readFile, writeFile, mkdir } from "node:fs/promises";
import path from "node:path";

const source = process.argv[2];
if (!source) throw new Error("Pass the original theme comparison HTML path");
const output = path.resolve("target/design-system");
await mkdir(output, { recursive: true });
const font = (await readFile("src/assets/fonts/geist-latin.woff2")).toString(
  "base64",
);
const png = async (name) =>
  `data:image/png;base64,${(await readFile(path.join(output, name))).toString("base64")}`;
const html = (await readFile(source, "utf8")).replace(/<link[^>]+>/g, "");
const browser = await chromium.launch();
try {
  const page = await browser.newPage({
    viewport: { width: 1240, height: 1060 },
    reducedMotion: "reduce",
  });
  await page.setContent(html);
  await page.addStyleTag({
    content: `@font-face{font-family:"Geist";src:url(data:font/woff2;base64,${font}) format("woff2");font-weight:400 600} :root{--mock-font:"Geist",-apple-system,system-ui,sans-serif}`,
  });
  await page.evaluate(() => document.fonts.ready);
  for (const theme of ["ember", "paper"])
    for (const [index, mode] of ["light", "dark"].entries()) {
      await page
        .locator(`#${theme} .mock`)
        .nth(index)
        .screenshot({
          path: path.join(output, `reference-${theme}-${mode}.png`),
        });
    }
  for (const theme of ["ember", "paper"])
    for (const mode of ["light", "dark"]) {
      const sheet = `<!doctype html><html><style>*{box-sizing:border-box}body{margin:0;padding:24px;background:#f4f2ee;color:#1c1a17;font:14px system-ui}h1{font-size:22px;margin:0 0 16px}h2{font-size:14px;margin:0 0 12px}.pair{display:grid;grid-template-columns:600px 820px;gap:24px}.pair img{width:100%;display:block}figure{margin:0}.controls{margin-top:24px}.controls img{width:1440px;display:block}.caption{color:#655f58}.pair img.capsule{width:auto;max-width:100%} </style><h1>${theme} · ${mode} · Geist</h1><div class="pair"><figure><h2>Chosen comparison panel</h2><img src="${await png(`reference-${theme}-${mode}.png`)}"><p class="caption">Original comparison source with Instrument Sans selected. Paper uses ink for live/on in the implementation.</p><h2>Recording capsule · implementation</h2><img class="capsule" src="${await png(`capsule-${theme}-${mode}.png`)}"></figure><figure><h2>Settings › General · implementation</h2><img src="${await png(`pages--general-${theme}-${mode}.png`)}"></figure></div><figure class="controls"><h2>Controls · first secondary button hovered · off/on switches side by side</h2><img src="${await png(`controls-hover-${theme}-${mode}.png`)}"></figure></html>`;
      await writeFile(
        path.join(output, `comparison-${theme}-${mode}.html`),
        sheet,
      );
      await page.setViewportSize({ width: 1490, height: 1060 });
      await page.setContent(sheet);
      await page.screenshot({
        path: path.join(output, `comparison-${theme}-${mode}.png`),
        fullPage: true,
      });
    }
  console.log("Captured four reference panels and four comparison sheets");
} finally {
  await browser.close();
}
