import { chromium } from "@playwright/test";
import { readFile, writeFile } from "node:fs/promises";
const browser = await chromium.launch();
try {
  const page = await browser.newPage();
  await page.setContent(
    `<style>${await readFile("src/tokens.css", "utf8")}</style><canvas width="1" height="1"></canvas>`,
  );
  const results = [];
  const palettes = [];
  for (const theme of ["ember", "paper"])
    for (const mode of ["light", "dark"]) {
      const { pairs, palette } = await page.evaluate(
        ({ theme, mode }) => {
          document.documentElement.dataset.theme = theme;
          document.documentElement.dataset.mode = mode;
          const canvas = document.querySelector("canvas");
          const ctx = canvas.getContext("2d", { willReadFrequently: true });
          const keycaps = {
            "keycap-primary": ["on-primary", "primary"],
            "keycap-overlay": ["overlay-text", "overlay"],
            "keycap-error": ["overlay-danger", "overlay"],
          };
          const resolve = (token) => {
            if (token in keycaps) {
              const [fg, bg] = keycaps[token];
              return resolve(fg).map((c, i) =>
                Math.round(c * 0.08 + resolve(bg)[i] * 0.92),
              );
            }
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
          for (const bg of [
            "canvas",
            "sidebar",
            "surface",
            "control",
            "control-hover",
            "control-pressed",
            "selected",
          ])
            for (const fg of ["text", "muted"]) pairs.push([fg, bg, 4.5]);
          for (const foreground of [
            "success-text",
            "warning",
            "danger",
            "info",
          ])
            for (const background of [
              "selected",
              "control-hover",
              "control-pressed",
            ])
              pairs.push([foreground, background, 4.5]);
          pairs.push(
            ["on-primary", "primary", 4.5],
            ["on-primary", "keycap-primary", 4.5],
            ["overlay-text", "keycap-overlay", 4.5],
            ["overlay-danger", "keycap-error", 4.5],
            ["on-primary", "primary-hover", 4.5],
            ["on-primary", "primary-pressed", 4.5],
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
          pairs.push(["recording", "overlay", 3]);
          const palette = Object.fromEntries(
            [
              "canvas",
              "sidebar",
              "surface",
              "control",
              "control-hover",
              "control-pressed",
              "selected",
              "text",
              "muted",
              "primary",
              "on-primary",
              "live",
              "recording",
              "toggle-off",
              "toggle-on",
              "success",
              "success-text",
              "warning",
              "warning-surface",
              "danger",
              "danger-surface",
              "info",
              "info-surface",
              "overlay",
              "overlay-text",
              "overlay-accent",
            ].map((token) => [token, `rgb(${resolve(token).join(" ")})`]),
          );
          return {
            palette,
            pairs: pairs.map(([foreground, background, minimum]) => {
              const fg = resolve(foreground),
                bg = resolve(background),
                a = luminance(fg),
                b = luminance(bg);
              const ratio = (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
              return {
                theme,
                mode,
                foreground,
                background,
                ratio: Number(ratio.toFixed(2)),
                minimum,
                pass: ratio >= minimum,
              };
            }),
          };
        },
        { theme, mode },
      );
      palettes.push({ theme, mode, colors: palette });
      results.push(...pairs);
    }
  await writeFile(
    "src/ui/contrast.generated.json",
    JSON.stringify(results, null, 2) + "\n",
  );
  await writeFile(
    "src/ui/palettes.generated.json",
    JSON.stringify(palettes, null, 2) + "\n",
  );
  console.table(results.filter((p) => !p.pass));
  console.log(
    `${results.length} measured pairs; minimum text contrast ${Math.min(...results.filter((p) => p.minimum === 4.5).map((p) => p.ratio))}:1`,
  );
  if (results.some((p) => !p.pass))
    throw new Error("Contrast requirement failed");
} finally {
  await browser.close();
}
