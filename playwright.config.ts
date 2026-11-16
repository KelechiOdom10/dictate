import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 20_000,
  use: { viewport: { width: 820, height: 650 } },
  outputDir: "target/ui-test-results",
  reporter: "list",
});
