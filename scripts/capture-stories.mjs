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
  const captures = [
    ["foundations--dark", "foundations-dark", "h1"],
    [
      "components-controls--controls-side-by-side",
      "controls-side-by-side",
      "h1",
    ],
    ["foundations--light", "foundations", "h1"],
    ["components-button--all-states", "buttons", "button"],
    ["components-controls--filters", "filter-chips", "button"],
    ["components-controls--toggle-control", "toggles", "input"],
    ["components-shortcutrecorder--capture", "shortcut-recorder", "button"],
    ["components-content--models", "model-cards", ".ui-card"],
    [
      "components-recordingcapsule--all-states",
      "recording-states",
      ".ui-capsule",
    ],
    ["pages--general", "general-light", "#login"],
    ["pages--general-dark", "general-dark", "#login"],
    ["pages--history", "history", ".ui-detail"],
    ["pages--models", "models", ".ui-card"],
  ];
  for (const [id, filename, selector] of captures) {
    await page.setViewportSize({
      width: id.includes("controls-side-by-side") ? 1440 : 1000,
      height: 820,
    });
    await page.goto(
      `http://localhost:6007/iframe.html?id=${id}&viewMode=story`,
    );
    await page
      .locator(`#storybook-root ${selector}`)
      .first()
      .waitFor({ state: "visible" });
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({
      path: path.join(output, `${filename}.png`),
      fullPage: true,
    });
    console.log(`Captured ${filename}`);
  }
  const index = JSON.parse(
    await readFile("target/storybook/index.json", "utf8"),
  );
  const catalog = [];
  for (const theme of ["light", "dark"]) {
    for (const entry of Object.values(index.entries).filter(
      (entry) => entry.type === "story",
    )) {
      await page.setViewportSize({
        width: entry.id.includes("controls-side-by-side") ? 1440 : 1000,
        height: 820,
      });
      await page.goto(
        `http://localhost:6007/iframe.html?id=${entry.id}&viewMode=story&globals=theme:${theme}`,
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
      await page.evaluate((theme) => {
        document.documentElement.dataset.theme = theme;
        return document.fonts.ready;
      }, theme);
      const filename = `${entry.id}-${theme}.png`;
      await page.screenshot({
        path: path.join(output, filename),
        fullPage: true,
      });
      catalog.push({ id: entry.id, theme, filename });
    }
  }
  await import("node:fs/promises").then((fs) =>
    fs.writeFile(
      path.join(output, "catalog.json"),
      JSON.stringify(catalog, null, 2),
    ),
  );
  console.log(`Captured all ${catalog.length} story/theme combinations`);
  await page.setViewportSize({ width: 320, height: 844 });
  for (const id of [
    "pages--general",
    "pages--general-dark",
    "pages--history",
    "pages--models",
    "components-recordingcapsule--all-states",
  ]) {
    await page.goto(
      `http://localhost:6007/iframe.html?id=${id}&viewMode=story`,
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
      path: path.join(output, `${id}-narrow.png`),
      fullPage: true,
    });
  }
  if (errors.length) throw new Error(errors.join("\n"));
} finally {
  await browser.close();
}
