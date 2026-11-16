import { expect, test } from "@playwright/test";
import { readFile } from "node:fs/promises";
import path from "node:path";

type WindowLabel = "main" | "settings" | "ink" | "annotate-bar";

// The app's own commands each window's capability file grants, so the mock
// rejects a command the real webview would be denied.
async function grantedCommands() {
  const granted: Record<string, string[]> = {};
  for (const file of ["default.json", "annotate.json", "annotate-bar.json"]) {
    const capability = JSON.parse(
      await readFile(
        path.join(process.cwd(), "src-tauri/capabilities", file),
        "utf8",
      ),
    ) as { windows: string[]; permissions: unknown[] };
    const commands = capability.permissions
      .filter((p): p is string => typeof p === "string")
      .filter((p) => p.startsWith("allow-"))
      .map((p) => p.slice("allow-".length).replaceAll("-", "_"));
    for (const label of capability.windows)
      granted[label] = [...(granted[label] ?? []), ...commands];
  }
  return granted;
}

async function openSettings(
  page: import("@playwright/test").Page,
  {
    failLoad = false,
    startupError = null,
    overlay = false,
    window: label = overlay ? "main" : "settings",
    stored = {},
    pendingPage = null,
    delayCapture = false,
    suspendFrames = false,
  }: {
    failLoad?: boolean;
    startupError?: { title: string; fix: string; detail: string | null } | null;
    overlay?: boolean;
    window?: WindowLabel;
    stored?: Record<string, unknown>;
    pendingPage?: string | null;
    delayCapture?: boolean;
    suspendFrames?: boolean;
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
        : file.endsWith(".woff2")
          ? "font/woff2"
          : "text/html";
    await route.fulfill({ body: await readFile(file), contentType: type });
  });
  const models = JSON.parse(
    await readFile(path.join(process.cwd(), "src/models.fixture.json"), "utf8"),
  ).map((model: { id: string }) =>
    model.id === "base.en"
      ? { ...model, active: true, state: { kind: "installed" } }
      : model,
  );
  const granted = (await grantedCommands())[label];
  await page.addInitScript(
    ({
      failLoad,
      startupError,
      models,
      label,
      granted,
      stored,
      pendingPage,
      delayCapture,
      suspendFrames,
    }) => {
      if (suspendFrames) window.requestAnimationFrame = () => 1;
      const denied: string[] = [];
      Object.defineProperty(window, "__deniedCommands", { value: denied });
      let status = { phase: "idle", error: startupError };
      const callbacks = new Map<number, (response: unknown) => void>();
      const listeners = new Map<string, { id: number; handler: number }[]>();
      const commands: string[] = [];
      Object.defineProperty(window, "__commands", { value: commands });
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
        else if (
          data.event === "dictation-state" ||
          data.event === "dictation-error"
        )
          status = data.payload;
        emit(data.event, data.payload);
      });
      const settings = {
        version: 1,
        shortcut: "Alt+Shift+Space",
        shortcutMode: "hold",
        microphone: null,
        launchAtLogin: false,
        theme: "ember",
        overlayPosition: "bottom",
        historyRetention: "week",
        appearanceMode: "system",
        speechModel: "base.en",
        pasteMethod: "paste",
        restoreClipboard: true,
        trailingSpace: false,
        capitalizeFirst: false,
        micMode: "onDemand",
        soundFeedback: false,
        debugRecordings: false,
        annotateShortcut: "Alt+Shift+KeyA",
        annotateEnabled: true,
        annotateShowInRecordings: false,
        ...stored,
      };
      Object.defineProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__", {
        value: {
          unregisterListener: (event: string, id: number) => {
            listeners.set(
              event,
              (listeners.get(event) ?? []).filter(
                (listener) => listener.id !== id,
              ),
            );
          },
        },
      });
      Object.defineProperty(window, "__TAURI_INTERNALS__", {
        value: {
          metadata: {
            currentWindow: { label },
            currentWebview: { label },
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
            commands.push(command);
            if (!command.startsWith("plugin:") && !granted.includes(command)) {
              denied.push(command);
              throw `${command} not allowed on window \`${label}\``;
            }
            if (command === "dictation_stop" && status.phase === "recording") {
              status = { phase: "transcribing", error: null };
              emit("dictation-state", "transcribing");
            }
            if (
              command === "dictation_cancel" &&
              status.phase === "recording"
            ) {
              status = { phase: "idle", error: null };
              emit("dictation-state", "idle");
            }
            if (command === "pipeline_status") return status;
            if (
              command === "capture_shortcut" &&
              args.enabled &&
              delayCapture
            ) {
              await new Promise<void>((resolve) => {
                Object.defineProperty(window, "__finishCapture", {
                  value: resolve,
                  configurable: true,
                });
              });
            }
            if (command === "settings_ready") {
              Object.defineProperty(window, "__readyAppearance", {
                value: {
                  palette: document.documentElement.dataset.palette,
                  mode: document.documentElement.dataset.mode,
                  libraryTheme: document.documentElement.dataset.astryxTheme,
                  colorScheme: getComputedStyle(
                    document.querySelector(".settings-shell")!,
                  ).colorScheme,
                },
                configurable: true,
              });
              if (!listeners.get("settings-page")?.length)
                throw "Navigation is not listening";
              if (pendingPage) emit("settings-page", pendingPage);
            }
            if (command === "get_settings") {
              if (failLoad) throw "Settings file could not be read";
              return settings;
            }
            if (command === "save_settings") {
              if (args.settings?.shortcut === "Super+KeyQ")
                throw "Shortcut unavailable";
              Object.assign(settings, args.settings);
              emit("settings-changed", settings);
              return settings;
            }
            if (command === "input_devices")
              return ["Built-in Microphone", "USB Microphone"];
            if (command === "microphone_level") return 0.12;
            if (command === "plugin:app|version") return "0.1.0";
            if (command === "list_history")
              return { entries: [], hasMore: false };
            if (command === "list_annotations") return [];
            if (command === "list_models") return models;
            if (command === "whisper_status") return null;
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
    {
      failLoad,
      startupError,
      models,
      label,
      granted,
      stored,
      pendingPage,
      delayCapture,
      suspendFrames,
    },
  );
  await page.goto(
    label === "main"
      ? "http://dictate.test/"
      : `http://dictate.test/?window=${label}`,
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
    .getByRole("combobox", { name: "Microphone", exact: true })
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
  await expect(
    page.getByText("Used by another app — pick another", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Shortcut", exact: true }),
  ).toContainText("⌃⇧D");
  await expect(page.getByLabel("Speech model")).toHaveValue("base.en");
  await page.getByRole("button", { name: "Manage models" }).click();
  await expect(
    page.getByRole("region", { name: "Base English" }),
  ).toContainText("In use");
  await page.getByRole("button", { name: "Output", exact: true }).click();
  await page
    .getByLabel("After dictation", { exact: true })
    .selectOption("copyOnly");
  await expect(page.getByLabel("After dictation", { exact: true })).toHaveValue(
    "copyOnly",
  );
  await page
    .getByLabel("After dictation", { exact: true })
    .selectOption("pasteKeep");
  await expect(page.getByLabel("After dictation", { exact: true })).toHaveValue(
    "pasteKeep",
  );
  await page.getByRole("button", { name: "Advanced", exact: true }).click();
  await page.getByRole("switch", { name: "Sound feedback" }).click();
  await expect(
    page.getByRole("switch", { name: "Sound feedback" }),
  ).toBeChecked();
  await page.getByRole("button", { name: "General", exact: true }).click();
  await expect(
    page.getByRole("combobox", { name: "Microphone", exact: true }),
  ).toHaveValue("USB Microphone");
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
    await page.evaluate(() => document.fonts.ready);
    await expect(page.locator("#mode-help")).toHaveCSS("line-height", "18px");
    expect((await page.locator("#mode-help").boundingBox())?.height).toBe(18);
    const footerAlignment = await page.locator("footer").evaluate((el) => {
      const version = el.querySelector("span");
      const button = el.querySelector("button");
      return version && button
        ? version.getBoundingClientRect().x === button.getBoundingClientRect().x
        : false;
    });
    expect(footerAlignment).toBe(true);
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
    await expect(
      page.getByRole("heading", { name: "Nothing here yet" }),
    ).toBeVisible();
  });
}

test("startup recovery is visible after mount and dismissible with the keyboard", async ({
  page,
}) => {
  await openSettings(page, {
    startupError: {
      title: "Settings couldn’t be read",
      fix: "Using defaults. Your old file is saved beside them.",
      detail: "invalid JSON; original saved to settings.json.bad-123",
    },
  });
  await expect(page.getByRole("alert")).toContainText(
    "Settings couldn’t be read. Using defaults. Your old file is saved beside them.",
  );
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
    startupError: {
      title: "Shortcut unavailable",
      fix: "Choose another shortcut in Settings.",
      detail: "HotKey already registered",
    },
  });
  await expect(page.getByRole("alert")).toHaveText("Shortcut unavailable");
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
          theme: "ember",
          overlayPosition: "bottom",
          historyRetention: "week",
          appearanceMode: "system",
          speechModel: "base.en",
          pasteMethod: "paste",
          restoreClipboard: true,
          trailingSpace: false,
          capitalizeFirst: false,
          micMode: "onDemand",
          soundFeedback: false,
          debugRecordings: false,
          annotateShortcut: "Alt+Shift+KeyA",
          annotateEnabled: true,
          annotateShowInRecordings: false,
        },
      },
      "*",
    ),
  );
  await expect(page.getByRole("alert")).toContainText("Shortcut unavailable");
  await expect(page.locator("kbd")).toHaveCount(0);
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
        event: "dictation-state",
        payload: { phase: "idle", error: null },
      },
      "*",
    ),
  );
  await expect(page.locator("kbd")).toHaveText(["⌃", "D"]);
  await page.evaluate(() =>
    window.postMessage(
      {
        type: "dictate-test",
        event: "dictation-error",
        payload: {
          phase: "idle",
          error: {
            title: "Accessibility access needed",
            fix: "Allow dictate in System Settings → Privacy & Security → Accessibility.",
            detail: null,
          },
        },
      },
      "*",
    ),
  );
  await expect(page.getByRole("alert")).toHaveText(
    "Accessibility access needed",
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

for (const theme of ["ember", "paper"] as const) {
  test(`${theme} appearance saves live and System follows macOS`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await openSettings(page);
    await page.getByLabel("Theme", { exact: true }).selectOption(theme);
    await expect(page.locator("html")).toHaveAttribute("data-palette", theme);
    await page.getByLabel("Mode", { exact: true }).selectOption("dark");
    await expect(page.locator("html")).toHaveAttribute("data-mode", "dark");
    await page.emulateMedia({ colorScheme: "light" });
    await expect(page.locator("html")).toHaveAttribute("data-mode", "dark");
    await page.getByLabel("Mode", { exact: true }).selectOption("system");
    await expect(page.locator("html")).toHaveAttribute("data-mode", "light");
    await page.emulateMedia({ colorScheme: "dark" });
    await expect(page.locator("html")).toHaveAttribute("data-mode", "dark");
    await page.getByRole("button", { name: "About", exact: true }).click();
    await page.getByRole("button", { name: "General", exact: true }).click();
    await expect(page.getByLabel("Theme", { exact: true })).toHaveValue(theme);
    await expect(page.getByLabel("Mode", { exact: true })).toHaveValue(
      "system",
    );
  });
}

test("overlay applies appearance events live, including System changes", async ({
  page,
}) => {
  await openSettings(page, { overlay: true });
  await expect(page.locator("kbd")).toHaveCount(3);
  for (const theme of ["ember", "paper"])
    for (const appearanceMode of ["light", "dark", "system"]) {
      await page.emulateMedia({ colorScheme: "light" });
      await page.evaluate(
        ({ theme, appearanceMode }) =>
          window.postMessage(
            {
              type: "dictate-test",
              event: "settings-changed",
              payload: {
                version: 1,
                shortcut: "Alt+Shift+Space",
                shortcutMode: "hold",
                microphone: null,
                launchAtLogin: false,
                theme,
                overlayPosition: "bottom",
                historyRetention: "week",
                appearanceMode,
                speechModel: "base.en",
                pasteMethod: "paste",
                restoreClipboard: true,
                trailingSpace: false,
                capitalizeFirst: false,
                micMode: "onDemand",
                soundFeedback: false,
                debugRecordings: false,
                annotateShortcut: "Alt+Shift+KeyA",
                annotateEnabled: true,
                annotateShowInRecordings: false,
              },
            },
            "*",
          ),
        { theme, appearanceMode },
      );
      await expect(page.locator("html")).toHaveAttribute("data-palette", theme);
      await expect(page.locator("html")).toHaveAttribute(
        "data-mode",
        appearanceMode === "dark" ? "dark" : "light",
      );
      await page.emulateMedia({ colorScheme: "dark" });
      await expect(page.locator("html")).toHaveAttribute(
        "data-mode",
        appearanceMode === "light" ? "light" : "dark",
      );
    }
});

test("overlay renders live level, resets its timer and keeps terminal states distinct", async ({
  page,
}) => {
  await openSettings(page, { overlay: true });
  const phase = async (phase: string) => {
    await page.evaluate(
      (phase) =>
        window.postMessage(
          {
            type: "dictate-test",
            event: "dictation-state",
            payload: { phase, error: null },
          },
          "*",
        ),
      phase,
    );
  };
  await phase("recording");
  await expect(page.locator(".ui-timer")).toHaveText("00:01");
  await page.evaluate(() =>
    window.postMessage(
      { type: "dictate-test", event: "recording-level", payload: 0.04 },
      "*",
    ),
  );
  await expect(page.getByRole("meter")).toHaveAttribute("value", "0.4");
  await phase("transcribing");
  await expect(page.getByRole("meter")).toHaveCount(0);
  await phase("done");
  await expect(page.getByRole("status")).toHaveText("Text inserted");
  await phase("recording");
  await expect(page.locator(".ui-timer")).toHaveText("00:00");
  await phase("skipped");
  await expect(page.getByRole("status")).toContainText("No speech captured");
  await expect(page.getByRole("status")).toContainText("Hold the shortcut");
});

test("overlay keeps the finished pill through its exit and starts the next take clean", async ({
  page,
}) => {
  await openSettings(page, { overlay: true });
  const send = async (event: string, payload: unknown) => {
    await page.evaluate(
      ({ event, payload }) =>
        window.postMessage({ type: "dictate-test", event, payload }, "*"),
      { event, payload },
    );
  };
  const resizes = () =>
    page.evaluate(
      () =>
        (window as unknown as { __commands: string[] }).__commands.filter(
          (command) => command === "resize_overlay",
        ).length,
    );
  const status = page.getByRole("status");
  await send("dictation-state", { phase: "recording", error: null });
  await send("overlay-showing", null);
  await expect(page.locator(".ui-timer")).toBeVisible();
  await send("dictation-state", { phase: "copied", error: null });
  await expect(status).toHaveText("Copied — paste it anywhere");
  await page.waitForTimeout(300);
  const settled = await resizes();
  await send("dictation-state", { phase: "idle", error: null });
  await page.waitForTimeout(300);
  await expect(status).toHaveText("Copied — paste it anywhere");
  await expect(page.locator(".overlay kbd")).toHaveCount(0);
  await send("overlay-hiding", null);
  await expect(page.locator(".overlay-capsule")).toHaveClass(/overlay-hiding/);
  await page.waitForTimeout(300);
  await expect(status).toHaveText("Copied — paste it anywhere");
  expect(await resizes()).toBe(settled);
  await send("dictation-state", { phase: "recording", error: null });
  await send("overlay-showing", null);
  await expect(status).toHaveText("Recording");
  await expect(page.locator(".ui-timer")).toHaveText("00:00");
  await expect(page.locator(".overlay-capsule")).not.toHaveClass(
    /overlay-hiding/,
  );
  await send("dictation-state", { phase: "done", error: null });
  await send("dictation-state", { phase: "idle", error: null });
  await send("overlay-showing", null);
  await expect(status).toContainText("Listening");
  await expect(page.locator(".overlay kbd").first()).toBeVisible();
});

test("overlay shows an Annotate session, its blocked-dictation notice and packaging", async ({
  page,
}) => {
  await openSettings(page, { overlay: true });
  const state = async (phase: string, notice: string | null = null) => {
    await page.evaluate(
      ({ phase, notice }) =>
        window.postMessage(
          {
            type: "dictate-test",
            event: "dictation-state",
            payload: { phase, error: null, notice },
          },
          "*",
        ),
      { phase, notice },
    );
  };
  await state("annotating");
  await expect(page.getByRole("status")).toHaveText("Annotating");
  await expect(page.locator(".ui-timer")).toHaveText("00:01");
  await expect(page.getByRole("meter")).toHaveCount(0);
  await state("annotating", "Dictation is off while Annotate records");
  await expect(page.getByRole("status")).toHaveText(
    "AnnotatingDictation is off while Annotate records",
  );
  await page.screenshot({ path: "target/overlay-annotate-blocked.png" });
  await state("packaging");
  await expect(page.getByRole("status")).toHaveText("Packaging…");
  await expect(page.locator(".ui-timer")).toHaveCount(0);
});

test("invalid appearance events show a recoverable overlay error", async ({
  page,
}) => {
  await openSettings(page, { overlay: true });
  await page.evaluate(() =>
    window.postMessage(
      {
        type: "dictate-test",
        event: "settings-changed",
        payload: { theme: "invalid" },
      },
      "*",
    ),
  );
  await expect(page.getByRole("alert")).toHaveText("Settings couldn’t refresh");
});

for (const label of ["ink", "annotate-bar"] as const) {
  test(`the ${label} panel keeps the stored theme and follows changes`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await openSettings(page, {
      window: label,
      stored: { theme: "paper", appearanceMode: "dark" },
    });
    await expect(page.locator("html")).toHaveAttribute("data-palette", "paper");
    await expect(page.locator("html")).toHaveAttribute("data-mode", "dark");
    const send = (payload: unknown) =>
      page.evaluate(
        (payload) =>
          window.postMessage(
            { type: "dictate-test", event: "settings-changed", payload },
            "*",
          ),
        payload,
      );
    const current = await page.evaluate(() =>
      (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (command: string) => unknown };
        }
      ).__TAURI_INTERNALS__.invoke("get_settings"),
    );
    await send({
      ...(current as object),
      theme: "ember",
      appearanceMode: "light",
    });
    await expect(page.locator("html")).toHaveAttribute("data-palette", "ember");
    await expect(page.locator("html")).toHaveAttribute("data-mode", "light");
    // An invalid event must not call a command the panel isn't granted; the
    // valid one after it proves the invalid one has been handled.
    await send({ theme: "invalid" });
    await send({ ...(current as object), theme: "paper" });
    await expect(page.locator("html")).toHaveAttribute("data-palette", "paper");
    expect(
      await page.evaluate(
        () =>
          (window as unknown as { __deniedCommands: string[] })
            .__deniedCommands,
      ),
    ).toEqual([]);
  });
}

test("capsule state screenshots cover every theme and fit a narrow overlay", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.setViewportSize({ width: 436, height: 200 });
  await openSettings(page, { overlay: true });
  for (const theme of ["ember", "paper"])
    for (const appearanceMode of ["light", "dark"]) {
      await page.evaluate(
        ({ theme, appearanceMode }) =>
          window.postMessage(
            {
              type: "dictate-test",
              event: "settings-changed",
              payload: {
                version: 1,
                shortcut: "Alt+Shift+Space",
                shortcutMode: "hold",
                microphone: null,
                launchAtLogin: false,
                theme,
                appearanceMode,
                overlayPosition: "bottom",
                historyRetention: "week",
                speechModel: "base.en",
                pasteMethod: "paste",
                restoreClipboard: true,
                trailingSpace: false,
                capitalizeFirst: false,
                micMode: "onDemand",
                soundFeedback: false,
                debugRecordings: false,
                annotateShortcut: "Alt+Shift+KeyA",
                annotateEnabled: true,
                annotateShowInRecordings: false,
              },
            },
            "*",
          ),
        { theme, appearanceMode },
      );
      await expect(page.locator("html")).toHaveAttribute("data-palette", theme);
      await expect(page.locator("html")).toHaveAttribute(
        "data-mode",
        appearanceMode,
      );
      for (const phase of [
        "recording",
        "transcribing",
        "done",
        "skipped",
        "annotating",
        "packaging",
        "error",
      ]) {
        await page.evaluate(
          (phase) =>
            window.postMessage(
              {
                type: "dictate-test",
                event: "dictation-state",
                payload: {
                  phase: phase === "error" ? "idle" : phase,
                  error:
                    phase === "error"
                      ? {
                          title: "No sound from the microphone",
                          fix: "Check it in Settings → General → Test microphone.",
                          detail: null,
                        }
                      : null,
                },
              },
              "*",
            ),
          phase,
        );
        await expect(page.locator(`.ui-capsule-${phase}`)).toBeVisible();
        if (phase === "error") await expect(page.locator("kbd")).toHaveCount(0);
        if (phase === "recording")
          await expect(page.locator(".ui-level-bars span").first()).toHaveCSS(
            "transition-duration",
            "0s",
          );
        if (phase === "recording") {
          await page.evaluate(() =>
            window.postMessage(
              { type: "dictate-test", event: "recording-level", payload: 0.04 },
              "*",
            ),
          );
          await expect(page.getByRole("meter")).toHaveAttribute("value", "0.4");
        }
        await page.evaluate(() => document.fonts.ready);
        await page.locator(".ui-capsule").screenshot({
          path: `target/overlay/${theme}-${appearanceMode}-${phase}.png`,
          omitBackground: true,
        });
        await page.setViewportSize({ width: 320, height: 240 });
        expect(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
        ).toBe(true);
        await page.setViewportSize({ width: 436, height: 200 });
      }
    }
});

for (const exit of ["tab", "window-blur", "navigation"]) {
  test(`pending shortcut capture stays canceled after ${exit}`, async ({
    page,
  }) => {
    await openSettings(page, { delayCapture: true });
    const recorder = page.getByRole("button", {
      name: "Shortcut",
      exact: true,
    });
    await recorder.click();
    await expect(recorder).toContainText("Press shortcut");
    if (exit === "tab") await page.keyboard.press("Tab");
    else if (exit === "navigation")
      await page.getByRole("button", { name: "Output", exact: true }).click();
    else
      await page.evaluate(() =>
        window.postMessage(
          { type: "dictate-test", event: "tauri://blur", payload: false },
          "*",
        ),
      );
    await expect(page.getByText("Press shortcut")).toHaveCount(0);
    await page.evaluate(() =>
      (window as unknown as { __finishCapture: () => void }).__finishCapture(),
    );
    if (exit === "navigation")
      await page.getByRole("button", { name: "General", exact: true }).click();
    await expect(recorder).not.toContainText("Press shortcut");
  });
}

for (const mode of ["light", "dark"] as const) {
  test(`normal-width History shows the default Annotate shortcut in ${mode}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 720, height: 580 });
    await page.emulateMedia({ colorScheme: mode, reducedMotion: "reduce" });
    await openSettings(page);
    await page.getByRole("button", { name: "History", exact: true }).click();
    const annotations = page.getByRole("radio", {
      name: "Annotations",
      exact: true,
    });
    await annotations.check();
    await expect(
      page.getByText("No annotations yet", { exact: true }),
    ).toBeVisible();
    await expect(page.locator("main .ui-keycaps kbd")).toHaveText([
      "⌥",
      "⇧",
      "A",
    ]);
    expect(
      await annotations.evaluate((input) => input.parentElement!.offsetTop),
    ).toBe(
      await page
        .getByRole("radio", { name: "Dictations", exact: true })
        .evaluate((input) => input.parentElement!.offsetTop),
    );
    await page.evaluate(() => document.fonts.ready);
    expect(
      await page
        .locator(".ui-segments")
        .evaluate(
          (el) =>
            getComputedStyle(el, "::before").backgroundColor !==
            getComputedStyle(el).backgroundColor,
        ),
    ).toBe(true);
    await page.screenshot({
      path: `target/visibility-fix/history-default-${mode}.png`,
    });
  });
}

for (const palette of ["ember", "paper"])
  for (const mode of ["light", "dark"] as const) {
    test(`Astryx settings colors stay readable in ${palette} ${mode}`, async ({
      page,
    }) => {
      await openSettings(page, {
        stored: { theme: palette, appearanceMode: mode },
      });
      await expect(page.locator("html")).toHaveAttribute(
        "data-astryx-theme",
        palette === "paper" ? "dictate-paper" : "dictate",
      );
      const pairs = await page.locator(".settings-shell").evaluate((el) => {
        const probe = document.createElement("canvas");
        el.append(probe);
        const context = probe.getContext("2d", { willReadFrequently: true })!;
        const luminance = (color: string) => {
          probe.style.color = color;
          context.fillStyle = getComputedStyle(probe).color;
          context.fillRect(0, 0, 1, 1);
          return Array.from(context.getImageData(0, 0, 1, 1).data)
            .slice(0, 3)
            .map((value) => {
              const channel = value / 255;
              return channel <= 0.04045
                ? channel / 12.92
                : ((channel + 0.055) / 1.055) ** 2.4;
            })
            .reduce(
              (sum, channel, i) => sum + channel * [0.2126, 0.7152, 0.0722][i],
              0,
            );
        };
        const pairs = [];
        for (const foreground of ["text", "muted", "on-primary"])
          for (const background of foreground === "on-primary"
            ? ["primary", "primary-hover", "primary-pressed"]
            : [
                "canvas",
                "surface",
                "control",
                "control-hover",
                "control-pressed",
                "selected",
                "segment-thumb",
              ]) {
            const a = luminance(`var(--${foreground})`),
              b = luminance(`var(--${background})`);
            pairs.push({
              foreground,
              background,
              ratio: (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05),
            });
          }
        const track = el.querySelector(".astryx-switch")!;
        const thumb = el.querySelector(".astryx-switch-thumb")!;
        const trackLight = luminance(getComputedStyle(track).backgroundColor);
        for (const [name, color] of [
          ["switch surface", "var(--surface)"],
          ["switch thumb", getComputedStyle(thumb).backgroundColor],
        ]) {
          const otherLight = luminance(color);
          pairs.push({
            foreground: name,
            background: "off track",
            ratio:
              (Math.max(trackLight, otherLight) + 0.05) /
              (Math.min(trackLight, otherLight) + 0.05),
          });
        }
        probe.remove();
        return pairs;
      });
      for (const pair of pairs)
        expect(
          pair.ratio,
          `${pair.foreground} on ${pair.background}`,
        ).toBeGreaterThanOrEqual(pair.background === "off track" ? 3 : 4.5);
    });
  }

test("History preserves an explicitly saved Annotate shortcut", async ({
  page,
}) => {
  await openSettings(page, { stored: { annotateShortcut: "Alt+Shift+KeyS" } });
  await page.getByRole("button", { name: "History", exact: true }).click();
  await page.getByRole("radio", { name: "Annotations", exact: true }).check();
  await expect(page.locator("main .ui-keycaps kbd")).toHaveText([
    "⌥",
    "⇧",
    "S",
  ]);
});

test("forced 140px stress case keeps the wrapped History thumb on the selected row", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "History", exact: true }).click();
  await page.addStyleTag({ content: ".ui-segments { max-width: 140px; }" });
  const option = page.getByRole("radio", { name: "Annotations", exact: true });
  await option.check();
  await expect
    .poll(() =>
      option.evaluate((input) => {
        const label = input.parentElement!;
        const thumb = new DOMMatrixReadOnly(
          getComputedStyle(label.parentElement!, "::before").transform,
        );
        return (
          label.offsetTop > 3 &&
          Math.abs(thumb.f - label.offsetTop) < 0.1 &&
          Math.abs(thumb.e - label.offsetLeft) < 0.1 &&
          label.parentElement!.style.getPropertyValue("--thumb-y") ===
            `${label.offsetTop}px`
        );
      }),
    )
    .toBe(true);
  await expect(
    page.getByText("No annotations yet", { exact: true }),
  ).toBeVisible();
  await page.screenshot({ path: "target/independent-qa/wrapped-history.png" });
});

for (const failLoad of [false, true]) {
  test(`Settings readiness does not wait for hidden animation frames, load failure ${failLoad}`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await openSettings(page, {
      suspendFrames: true,
      failLoad,
      stored: { theme: "paper", appearanceMode: "dark" },
      pendingPage: "Annotate",
    });
    await expect
      .poll(() =>
        page.evaluate(
          () =>
            (window as unknown as { __readyAppearance?: unknown })
              .__readyAppearance,
        ),
      )
      .toEqual({
        palette: failLoad ? "ember" : "paper",
        mode: failLoad ? "light" : "dark",
        libraryTheme: failLoad ? "dictate" : "dictate-paper",
        colorScheme: failLoad ? "light dark" : "dark",
      });
    await expect(
      page.getByRole("heading", { name: "Annotate", level: 1, exact: true }),
    ).toBeVisible();
  });
}

test("Settings readiness receives queued notice navigation and retains the recovery problem", async ({
  page,
}) => {
  await openSettings(page, {
    pendingPage: "Annotate",
    stored: { appearanceMode: "dark", theme: "paper" },
    startupError: {
      title: "Annotate needs Screen Recording",
      fix: "Open System Settings to allow access.",
      detail: null,
    },
  });
  await expect(
    page.getByRole("heading", { name: "Annotate", exact: true, level: 1 }),
  ).toBeVisible();
  await expect(page.locator("html")).toHaveAttribute("data-mode", "dark");
  await expect(page.getByRole("alert")).toContainText(
    "Open System Settings to allow access.",
  );
  expect(
    await page.evaluate(
      () => (window as unknown as { __commands: string[] }).__commands,
    ),
  ).not.toContain("dismiss_error");
});

for (const mode of ["light", "dark"]) {
  test(`QA round 3 states in ${mode}`, async ({ page }) => {
    await page.emulateMedia({
      colorScheme: mode === "dark" ? "dark" : "light",
      reducedMotion: "reduce",
    });
    await page.setViewportSize({
      width: process.env.QA_BASELINE ? 820 : 720,
      height: process.env.QA_BASELINE ? 680 : 580,
    });
    const output = process.env.QA_BASELINE ? "before" : "after";
    await openSettings(page);
    await expect(
      page.getByRole("heading", { name: "General", exact: true }),
    ).toBeVisible();
    await page.screenshot({
      path: `target/qa-round-3/${output}-settings-${mode}.png`,
    });
    await page.getByRole("button", { name: "Shortcut", exact: true }).click();
    await expect(
      page
        .getByRole("button", { name: "Shortcut", exact: true })
        .filter({ hasText: "Press shortcut" }),
    ).toBeVisible();
    await page.screenshot({
      path: `target/qa-round-3/${output}-recorder-${mode}.png`,
    });
    await page.keyboard.press("Escape");
    const settingsPage = page;
    page = await page.context().newPage();
    await settingsPage.close();
    await page.emulateMedia({
      colorScheme: mode === "dark" ? "dark" : "light",
      reducedMotion: "reduce",
    });
    await page.setViewportSize({ width: 436, height: 120 });
    await openSettings(page, {
      overlay: true,
      startupError: {
        title: "Annotate needs Screen Recording",
        fix: "Open System Settings to allow access.",
        detail: null,
      },
    });
    await expect(page.getByRole("alert")).toBeVisible();
    await expect(page.locator("html")).toHaveAttribute("data-mode", mode);
    await page.screenshot({
      path: `target/qa-round-3/${output}-notice-${mode}.png`,
    });
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
    await page.screenshot({
      path: `target/qa-round-3/${output}-recording-${mode}.png`,
    });
    await page.getByRole("button", { name: "Finish dictation" }).click();
    await expect(page.getByRole("status")).toHaveText("Transcribing…");
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
    await page.getByRole("button", { name: "Discard dictation" }).click();
    await expect(page.getByRole("status")).toContainText("Ready");
  });
}

for (const width of [720, 560, 390])
  test(`shortcut keycaps are never clipped at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 580 });
    await page.emulateMedia({ colorScheme: "dark" });
    await openSettings(page);
    const recorder = page.getByRole("button", {
      name: "Shortcut",
      exact: true,
    });
    await expect(recorder.locator("kbd")).toHaveText(["⌥", "⇧", "Space"]);
    const clipped = await recorder.evaluate((button) =>
      [...button.querySelectorAll("kbd")].flatMap((kbd) => {
        const key = kbd.getBoundingClientRect();
        const hidden: string[] = [];
        for (let el = kbd.parentElement; el; el = el.parentElement) {
          if (getComputedStyle(el).overflowX === "visible") continue;
          const box = el.getBoundingClientRect();
          if (key.left < box.left - 0.5 || key.right > box.right + 0.5)
            hidden.push(
              `${kbd.textContent} clipped by ${el.tagName}.${el.className}`,
            );
        }
        const control = button.getBoundingClientRect();
        if (key.left < control.left - 0.5 || key.right > control.right + 0.5)
          hidden.push(`${kbd.textContent} outside the recorder`);
        return hidden;
      }),
    );
    expect(clipped).toEqual([]);
  });

test("settings controls share one pill shape", async ({ page }) => {
  await openSettings(page);
  await expect(page.getByRole("switch")).toBeAttached();
  const radii = await page.locator("main").evaluate((main) =>
    [
      ...main.querySelectorAll(
        ".ui-group button:not(.row-link), .ui-group select, .ui-shortcut",
      ),
    ]
      .filter((el) => el.getBoundingClientRect().width > 0)
      .map((el) => {
        const { height } = el.getBoundingClientRect();
        return (
          parseFloat(getComputedStyle(el).borderTopLeftRadius) >=
          height / 2 - 0.5
        );
      }),
  );
  expect(radii.length).toBeGreaterThan(3);
  expect(radii.every(Boolean)).toBe(true);
});
