import { chromium } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  await page.setContent(
    `<style>${await readFile("src/tokens.css", "utf8")}</style><canvas width="1" height="1"></canvas>`,
  );
  const results = [];
  for (const theme of ["light", "dark"]) {
    const pairs = await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
      const canvas = document.querySelector("canvas");
      const ctx = canvas.getContext("2d", { willReadFrequently: true });
      const resolve = (token) => {
        canvas.style.color = `var(--${token})`;
        ctx.fillStyle = getComputedStyle(canvas).color;
        ctx.fillRect(0, 0, 1, 1);
        return Array.from(ctx.getImageData(0, 0, 1, 1).data).slice(0, 3);
      };
      const luminance = (rgb) =>
        rgb
          .map((v) => {
            const c = v / 255;
            return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
          })
          .reduce((sum, c, i) => sum + c * [0.2126, 0.7152, 0.0722][i], 0);
      const pairs = [];
      for (const bg of ["canvas", "sidebar", "surface", "control", "selected"])
        for (const fg of ["text", "muted"]) pairs.push([fg, bg, 4.5]);
      for (const foreground of ["success-text", "warning", "danger", "info"])
        pairs.push([foreground, "selected", 4.5]);
      pairs.push(
        ["on-primary", "primary", 4.5],
        ["success-text", "success", 4.5],
        ["warning", "warning-surface", 4.5],
        ["danger", "danger-surface", 4.5],
        ["info", "info-surface", 4.5],
        ["error", "canvas", 4.5],
        ["error", "surface", 4.5],
      );
      for (const fg of [
        "overlay-text",
        "overlay-muted",
        "overlay-success",
        "overlay-danger",
      ])
        pairs.push([fg, "overlay", 4.5]);
      for (const bg of ["canvas", "surface", "control", "selected"])
        pairs.push(["accent", bg, 3]);
      return pairs.map(([foreground, background, minimum]) => {
        const fg = resolve(foreground),
          bg = resolve(background),
          a = luminance(fg),
          b = luminance(bg);
        const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
        return {
          theme,
          foreground,
          background,
          ratio: Number(ratio.toFixed(2)),
          minimum,
          pass: ratio >= minimum,
        };
      });
    }, theme);
    results.push(...pairs);
  }
  await writeFile(
    "src/ui/contrast.generated.json",
    JSON.stringify(results, null, 2) + "\n",
  );
  console.table(results);
  if (results.some((p) => !p.pass))
    throw new Error("Contrast requirement failed");
} finally {
  await browser.close();
}
