import { chromium } from "playwright-core";

export async function launch() {
  for (const options of [{ channel: "chrome" }, {}]) {
    try {
      return await chromium.launch(options);
    } catch {}
  }
  throw new Error("No browser found. Install Google Chrome, or run: pnpm exec playwright-core install chromium");
}
