import { defineConfig } from "vitest/config";
import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { playwright } from "@vitest/browser-playwright";
export default defineConfig({
  test: {
    projects: ["ember", "paper", "capy", "rosePine", "neutral"].flatMap(
      (theme, groupOrder) =>
        ["light", "dark"].map((mode) => ({
          plugins: [
            storybookTest({
              configDir: ".storybook",
              initialGlobals: { theme, mode },
            }),
          ],
          optimizeDeps: { include: ["react-dom"] },
          test: {
            name: `${theme}-${mode}`,
            sequence: { groupOrder },
            browser: {
              enabled: true,
              headless: true,
              provider: playwright(),
              instances: [
                {
                  browser: "chromium",
                  context: { colorScheme: mode === "dark" ? "dark" : "light" },
                },
              ],
            },
          },
        })),
    ),
  },
});
