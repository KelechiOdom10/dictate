import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import path from "node:path";

async function openSettings(
  page: import("@playwright/test").Page,
  failLoad = false,
) {
  await page.route("http://dictate.test/**", async (route) => {
    const pathname = new URL(route.request().url()).pathname;
    const file = path.join(
      process.cwd(),
      "dist",
      pathname === "/" ? "index.html" : pathname,
    );
    const type = file.endsWith(".js")
      ? "application/javascript"
      : file.endsWith(".css")
        ? "text/css"
        : "text/html";
    await route.fulfill({ body: await readFile(file), contentType: type });
  });
  await page.addInitScript(
    ({ failLoad }) => {
      const settings = {
        shortcut: "Alt+Shift+Space",
        shortcutMode: "hold",
        microphone: null,
        launchAtLogin: false,
      };
      Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
        value: { unregisterListener: () => {} },
      });
      Object.defineProperty(window, "__TAURI_INTERNALS__", {
        value: {
          metadata: {
            currentWindow: { label: "settings" },
            currentWebview: { label: "settings" },
          },
          transformCallback: () => 1,
          invoke: async (
            command: string,
            args: { settings?: typeof settings; enabled?: boolean },
          ) => {
            if (command === "get_settings") {
              if (failLoad) throw "Settings file could not be read";
              return settings;
            }
            if (command === "save_settings") {
              if (args.settings?.shortcut === "Super+KeyQ")
                throw "Shortcut unavailable";
              Object.assign(settings, args.settings);
              return settings;
            }
            if (command === "input_devices")
              return ["Built-in Microphone", "USB Microphone"];
            if (command === "microphone_level") return 0.12;
            if (command === "plugin:app|version") return "0.1.0";
            if (command === "plugin:event|listen") return 1;
            return null;
          },
        },
      });
    },
    { failLoad },
  );
  await page.goto("http://dictate.test/?window=settings");
}

test("preferences save, shortcut failures preserve the old value, and placeholders navigate", async ({
  page,
}) => {
  await openSettings(page);
  await expect(
    page.getByRole("heading", { name: "General", exact: true }),
  ).toBeVisible();
  await page
    .getByLabel("Shortcut mode", { exact: true })
    .selectOption("toggle");
  await expect(
    page.getByText("Tap to record. Tap again to insert your words."),
  ).toBeVisible();
  await page
    .getByLabel("Microphone", { exact: true })
    .selectOption("USB Microphone");
  await page.getByRole("button", { name: "Test microphone" }).click();
  await expect(page.getByRole("meter")).toHaveAttribute("value", "0.6");
  await page.getByRole("button", { name: "Stop test" }).click();
  await page.getByRole("switch", { name: "Launch at login" }).click();
  await expect(page.getByRole("switch")).toBeChecked();
  await page.getByRole("button", { name: "Shortcut", exact: true }).click();
  await page.keyboard.press("Control+Shift+D");
  await expect(
    page.getByRole("button", { name: "Shortcut", exact: true }),
  ).toContainText("⌃ ⇧ D");
  await page.getByRole("button", { name: "Shortcut", exact: true }).click();
  await page.keyboard.press("Meta+Q");
  await expect(page.getByRole("alert")).toHaveText("Shortcut unavailable");
  await expect(
    page.getByRole("button", { name: "Shortcut", exact: true }),
  ).toContainText("⌃ ⇧ D");
  await page.getByRole("button", { name: "Models", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Coming soon" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Back to General" }).click();
  await expect(page.getByLabel("Microphone", { exact: true })).toHaveValue(
    "USB Microphone",
  );
});

test("load failure offers recovery and controls never appear with unknown settings", async ({
  page,
}) => {
  await openSettings(page, true);
  await expect(page.getByRole("alert")).toContainText(
    "Settings file could not be read",
  );
  await expect(page.getByRole("button", { name: "Try again" })).toBeVisible();
  await expect(page.getByRole("switch")).toHaveCount(0);
});

for (const colorScheme of ["light", "dark"] as const) {
  test(`General fits ${colorScheme} and narrow widths`, async ({ page }) => {
    await page.emulateMedia({ colorScheme });
    await openSettings(page);
    await expect(page.getByRole("switch")).toBeVisible();
    await page.screenshot({
      path: `target/settings-${colorScheme}-browser.png`,
    });
    await page.setViewportSize({ width: 390, height: 844 });
    await expect(
      page.getByRole("button", { name: "General", exact: true }),
    ).toBeVisible();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await page.getByRole("button", { name: "History", exact: true }).focus();
    await page.keyboard.press("Enter");
    await expect(
      page.getByRole("heading", { name: "History", exact: true }),
    ).toBeVisible();
  });
}
