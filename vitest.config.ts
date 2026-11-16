import { defineConfig } from "vitest/config";
import { storybookTest } from "@storybook/addon-vitest/vitest-plugin";
import { playwright } from "@vitest/browser-playwright";
export default defineConfig({
  test: {
    projects: ["ember", "paper"].flatMap((theme) =>
      ["light", "dark"].map((mode) => ({
        plugins: [
          storybookTest({
            configDir: ".storybook",
            initialGlobals: { theme, mode },
          }),
        ],
        test: {
          name: `${theme}-${mode}`,
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
