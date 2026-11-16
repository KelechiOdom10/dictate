import { chromium } from "@playwright/test";
import { readFile, mkdir } from "node:fs/promises";
import path from "node:path";

const output = path.resolve("target/design-system");
await mkdir(output, { recursive: true });
const browser = await chromium.launch();
try {
  const page = await browser.newPage({
    viewport: { width: 1000, height: 820 },
    reducedMotion: "reduce",
  });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.route("http://localhost:6007/**", async (route) => {
    const pathname = new URL(route.request().url()).pathname;
    const file = path.join(
      process.cwd(),
      "target/storybook",
      pathname === "/" ? "index.html" : pathname,
    );
    const types = {
      ".js": "application/javascript",
      ".css": "text/css",
      ".html": "text/html",
      ".json": "application/json",
      ".svg": "image/svg+xml",
      ".woff2": "font/woff2",
    };
    try {
      await route.fulfill({
        body: await readFile(file),
        contentType: types[path.extname(file)] ?? "application/octet-stream",
      });
    } catch {
      await route.fulfill({ status: 404, body: "Not found" });
    }
  });
  const index = JSON.parse(
    await readFile("target/storybook/index.json", "utf8"),
  );
  const catalog = [];
  for (const theme of ["ember", "paper"])
    for (const mode of ["light", "dark"]) {
      for (const entry of Object.values(index.entries).filter(
        (entry) => entry.type === "story",
      )) {
        await page.setViewportSize({
          width: entry.id.includes("controls-side-by-side")
            ? 1440
            : entry.id === "pages--general"
              ? 820
              : 1000,
          height: entry.id === "pages--general" ? 1060 : 820,
        });
        await page.goto(
          `http://localhost:6007/iframe.html?id=${entry.id}&viewMode=story&globals=theme:${theme};mode:${mode}`,
        );
        await page
          .locator("#storybook-root > *")
          .first()
          .waitFor({ state: "visible" });
        if (
          entry.id.startsWith("pages--general") &&
          !entry.id.includes("loading") &&
          !entry.id.includes("error")
        )
          await page.locator("#login").waitFor();
        await page.evaluate(async () => {
          await document.fonts.ready;
          for (const weight of [400, 500, 600]) {
            await document.fonts.load(`${weight} 13px "Instrument Sans"`);
            if (!document.fonts.check(`${weight} 13px "Instrument Sans"`))
              throw new Error(`Font weight ${weight} unavailable`);
          }
          if (
            ![...document.fonts].some(
              (face) =>
                face.family === "Instrument Sans" && face.status === "loaded",
            )
          )
            throw new Error("Bundled font did not load");
        });
        const filename = `${entry.id}-${theme}-${mode}.png`;
        await page.screenshot({
          path: path.join(output, filename),
          fullPage: true,
        });
        catalog.push({ id: entry.id, theme, mode, filename });
        if (entry.id === "components-recordingcapsule--recording") {
          await page.locator(".ui-capsule").screenshot({
            path: path.join(output, `capsule-${theme}-${mode}.png`),
          });
        }
        if (entry.id === "components-controls--controls-side-by-side") {
          const button = page.locator(".ui-button-secondary").first();
          const before = await button.evaluate(
            (el) => getComputedStyle(el).backgroundColor,
          );
          await button.hover();
          await page.waitForTimeout(150);
          if (
            before ===
            (await button.evaluate(
              (el) => getComputedStyle(el).backgroundColor,
            ))
          )
            throw new Error("Secondary button has no hover change");
          await page.locator(".control-comparison").screenshot({
            path: path.join(output, `controls-hover-${theme}-${mode}.png`),
          });
          await button.focus();
          await page.keyboard.down(" ");
          await page.locator(".control-comparison").screenshot({
            path: path.join(output, `controls-pressed-${theme}-${mode}.png`),
          });
          await page.keyboard.up(" ");
          const toggle = page.locator(".ui-toggle").first();
          await toggle.focus();
          await page.keyboard.press("Tab");
          await page.keyboard.press("Shift+Tab");
          const ring = await toggle.evaluate(
            (el) => getComputedStyle(el, "::before").outlineWidth,
          );
          if (ring !== "2px")
            throw new Error(`Missing track focus ring: ${ring}`);
          await page.locator(".control-comparison").screenshot({
            path: path.join(output, `controls-focus-${theme}-${mode}.png`),
          });
        }
      }
    }
  await import("node:fs/promises").then((fs) =>
    fs.writeFile(
      path.join(output, "catalog.json"),
      JSON.stringify(catalog, null, 2),
    ),
  );
  console.log(`Captured all ${catalog.length} story/theme/mode combinations`);
  await page.setViewportSize({ width: 320, height: 844 });
  for (const theme of ["ember", "paper"])
    for (const mode of ["light", "dark"])
      for (const id of [
        "pages--general",
        "pages--history",
        "pages--models",
        "components-recordingcapsule--all-states",
      ]) {
        await page.goto(
          `http://localhost:6007/iframe.html?id=${id}&viewMode=story&globals=theme:${theme};mode:${mode}`,
        );
        await page
          .locator("#storybook-root > *")
          .first()
          .waitFor({ state: "visible" });
        if (
          await page.evaluate(
            () => document.documentElement.scrollWidth > innerWidth,
          )
        )
          throw new Error(`${id} overflows at 320px`);
        await page.screenshot({
          path: path.join(output, `${id}-${theme}-${mode}-narrow.png`),
          fullPage: true,
        });
      }
  if (errors.length) throw new Error(errors.join("\n"));
} finally {
  await browser.close();
}
