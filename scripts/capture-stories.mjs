import { chromium } from "@playwright/test";
import { readFile, mkdir } from "node:fs/promises";
import path from "node:path";

const output = path.resolve("target/design-system");
await mkdir(output, { recursive: true });
const historyReady = {
  "pages--history": (page) =>
    page.locator(".ui-detail", { hasText: "bakery" }).waitFor(),
  "pages--history-empty": (page) =>
    page.getByRole("heading", { name: "Nothing here yet" }).waitFor(),
  "pages--history-search": (page) =>
    page
      .locator(".ui-list-item-button", { hasText: "Slack" })
      .and(page.locator(".ui-list-item-button").last())
      .and(page.locator(".ui-list-item-button").first())
      .waitFor(),
  "pages--history-retention": (page) =>
    page
      .getByRole("group", { name: "Delete dictations older than 1 day?" })
      .waitFor(),
  "pages--history-delete": (page) =>
    page
      .locator(".ui-detail", { hasText: "Then hand it to review." })
      .waitFor(),
  "pages--history-annotations": (page) =>
    page
      .locator(".ui-detail", { hasText: "with a contact sheet" })
      .and(page.locator(".ui-detail", { hasText: "This toggle" }))
      .waitFor(),
  "pages--history-annotations-empty": (page) =>
    page.getByRole("heading", { name: "No annotations yet" }).waitFor(),
  "pages--history-no-matches": (page) =>
    page.getByRole("heading", { name: "No matches for “zebra”" }).waitFor(),
};
const modelsReady = {
  "pages--models-idle": (page) =>
    page.getByText(/Download a model to start dictating/).waitFor(),
  "pages--models-downloading": (page) =>
    page.getByText("205 MB of 488 MB").waitFor(),
  "pages--models-preparing": (page) =>
    page.getByText("Preparing your model…").waitFor(),
  "pages--models-installed": (page) =>
    page.getByRole("button", { name: "Use Small English" }).waitFor(),
  "pages--models-error": (page) =>
    page.getByText("brew install whisper.cpp").waitFor(),
};
const settingsReady = {
  "pages--output": (page) =>
    page.locator("#capitalize-first:checked:enabled").waitFor(),
  "pages--output-copy-only": (page) =>
    page
      .locator("#paste-method")
      .filter({ has: page.locator('option:checked[value="copyOnly"]') })
      .waitFor(),
  "pages--advanced": (page) =>
    page.locator("#mic-warm:checked:enabled").waitFor(),
  "pages--about": (page) =>
    page.locator("output#version", { hasText: "0.1.1" }).waitFor(),
  "pages--annotate": (page) =>
    page
      .getByText("Allowed", { exact: true })
      .and(page.locator("output"))
      .waitFor(),
  "pages--annotate-not-allowed": (page) =>
    page.getByText("Not allowed", { exact: true }).waitFor(),
  "pages--models-fresh-profile": (page) =>
    page.getByText(/Parakeet English is a good first choice/).waitFor(),
  "pages--general-no-model": (page) =>
    page.getByRole("button", { name: "Download a model" }).waitFor(),
};
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
            : entry.id === "pages--general" ||
                entry.id in historyReady ||
                entry.id in modelsReady ||
                entry.id in settingsReady
              ? 820
              : 1000,
          height:
            entry.id === "pages--general"
              ? 1060
              : entry.id in historyReady
                ? 680
                : 820,
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
          await page.getByRole("switch", { name: "Launch at login" }).waitFor();
        if (entry.id in historyReady) await historyReady[entry.id](page);
        if (entry.id in modelsReady) await modelsReady[entry.id](page);
        if (entry.id in settingsReady) await settingsReady[entry.id](page);
        await page.evaluate(async () => {
          await document.fonts.ready;
          for (const weight of [400, 500, 600]) {
            await document.fonts.load(`${weight} 13px "Geist"`);
            if (!document.fonts.check(`${weight} 13px "Geist"`))
              throw new Error(`Font weight ${weight} unavailable`);
          }
          if (
            ![...document.fonts].some(
              (face) => face.family === "Geist" && face.status === "loaded",
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
        "pages--models-downloading",
        "pages--models-error",
        "pages--output",
        "pages--advanced",
        "pages--about",
        "components-recordingcapsule--all-states",
      ]) {
        await page.goto(
          `http://localhost:6007/iframe.html?id=${id}&viewMode=story&globals=theme:${theme};mode:${mode}`,
        );
        await page
          .locator("#storybook-root > *")
          .first()
          .waitFor({ state: "visible" });
        if (id in modelsReady) await modelsReady[id](page);
        if (id in settingsReady) await settingsReady[id](page);
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
