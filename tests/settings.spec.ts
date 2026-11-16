import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import path from "node:path";

async function openSettings(
  page: import("@playwright/test").Page,
  {
    failLoad = false,
    startupError = null,
    overlay = false,
  }: {
    failLoad?: boolean;
    startupError?: string | null;
    overlay?: boolean;
  } = {},
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
    ({ failLoad, startupError }) => {
      let status = { phase: "idle", error: startupError };
      const callbacks = new Map<number, (response: unknown) => void>();
      const listeners = new Map<string, { id: number; handler: number }[]>();
      let nextId = 0;
      function emit(event: string, payload: unknown) {
        for (const listener of listeners.get(event) ?? []) {
          callbacks.get(listener.handler)?.({
            event,
            id: listener.id,
            payload,
          });
        }
      }
      window.addEventListener("message", ({ data }) => {
        if (data.type !== "dictate-test") return;
        if (data.event === "settings-changed")
          Object.assign(settings, data.payload);
        else status = data.payload;
        emit(data.event, data.payload);
      });
      const settings = {
        version: 1,
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
          transformCallback: (callback: (response: unknown) => void) => {
            const id = ++nextId;
            callbacks.set(id, callback);
            return id;
          },
          invoke: async (
            command: string,
            args: {
              settings?: typeof settings;
              enabled?: boolean;
              event?: string;
              handler?: number;
            },
          ) => {
            if (command === "pipeline_status") return status;
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
            if (command === "dismiss_error") {
              status = { phase: "idle", error: null };
              emit("dictation-state", "idle");
            }
            if (
              command === "plugin:event|listen" &&
              args.event &&
              args.handler
            ) {
              const id = ++nextId;
              listeners.set(args.event, [
                ...(listeners.get(args.event) ?? []),
                { id, handler: args.handler },
              ]);
              return id;
            }
            return null;
          },
        },
      });
    },
    { failLoad, startupError },
  );
  await page.goto(
    overlay ? "http://dictate.test/" : "http://dictate.test/?window=settings",
  );
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
  ).toContainText("⌃⇧D");
  await page.getByRole("button", { name: "Shortcut", exact: true }).click();
  await page.keyboard.press("Meta+Q");
  await expect(page.getByRole("alert")).toHaveText("Shortcut unavailable");
  await expect(
    page.getByRole("button", { name: "Shortcut", exact: true }),
  ).toContainText("⌃⇧D");
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
  await openSettings(page, { failLoad: true });
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

test("startup recovery is visible after mount and dismissible with the keyboard", async ({
  page,
}) => {
  await openSettings(page, {
    startupError:
      "Read settings: invalid JSON. Using default settings. Original saved to settings.json.bad-123",
  });
  await expect(page.getByRole("alert")).toContainText("Using default settings");
  await expect(page.getByRole("switch")).toBeVisible();
  await page.screenshot({ path: "target/startup-recovery-browser.png" });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await page.getByRole("button", { name: "Dismiss", exact: true }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("overlay receives retained errors, live phases and changed shortcuts", async ({
  page,
}) => {
  await page.setViewportSize({ width: 460, height: 132 });
  await openSettings(page, {
    overlay: true,
    startupError: "Shortcut unavailable. Choose another shortcut in Settings.",
  });
  await expect(page.getByRole("alert")).toContainText("Shortcut unavailable");
  await page.screenshot({ path: "target/overlay-error-browser.png" });
  await page.evaluate(() =>
    window.postMessage(
      {
        type: "dictate-test",
        event: "settings-changed",
        payload: {
          version: 1,
          shortcut: "Control+KeyD",
          shortcutMode: "hold",
          microphone: null,
          launchAtLogin: false,
        },
      },
      "*",
    ),
  );
  await expect(page.locator("kbd")).toHaveText(["⌃", "D"]);
  await page.evaluate(() =>
    window.postMessage(
      {
        type: "dictate-test",
        event: "dictation-state",
        payload: { phase: "recording", error: null },
      },
      "*",
    ),
  );
  await expect(page.getByRole("status")).toHaveText("Recording");
  await page.evaluate(() =>
    window.postMessage(
      {
        type: "dictate-test",
        event: "dictation-state",
        payload: { phase: "transcribing", error: null },
      },
      "*",
    ),
  );
  await expect(page.getByRole("status")).toHaveText("Transcribing…");
  await page.evaluate(() =>
    window.postMessage(
      {
        type: "dictate-test",
        event: "dictation-error",
        payload: {
          phase: "idle",
          error: "Microphone unavailable. Using the default microphone.",
        },
      },
      "*",
    ),
  );
  await expect(page.getByRole("alert")).toContainText(
    "Using the default microphone",
  );
});

for (const colorScheme of ["light", "dark"] as const) {
  test(`microphone fallback preserves recording, timer and transcribing in ${colorScheme}`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme });
    await page.setViewportSize({ width: 460, height: 132 });
    await openSettings(page, { overlay: true });
    await expect(page.getByRole("status")).toContainText("Ready");
    for (const phase of ["recording", "transcribing", "idle"]) {
      await page.evaluate(
        (phase) =>
          window.postMessage(
            {
              type: "dictate-test",
              event: "dictation-state",
              payload: {
                phase,
                error: null,
                notice: "Using the default microphone",
              },
            },
            "*",
          ),
        phase,
      );
      await expect(page.getByRole("alert")).toHaveCount(0);
      if (phase === "idle") {
        await expect(
          page.getByText("Using the default microphone"),
        ).toHaveCount(0);
      } else {
        await expect(page.getByRole("status")).toContainText(
          phase === "recording" ? "Recording" : "Transcribing…",
        );
        await expect(
          page.getByText("Using the default microphone"),
        ).toBeVisible();
        if (phase === "recording") {
          await expect(page.locator(".ui-timer")).toContainText("00:01");
          await page.screenshot({
            path: `target/overlay-notice-${colorScheme}.png`,
          });
          await page.setViewportSize({ width: 390, height: 132 });
          expect(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          ).toBe(true);
        }
      }
    }
  });
}
