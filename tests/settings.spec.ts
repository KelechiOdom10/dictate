import { expect, test, type Page } from "@playwright/test";
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
      // Lets a test move one model between states, the way a download does.
      Object.defineProperty(window, "__setModelState", {
        value: (id, state, active = false) => {
          const model = models.find((candidate) => candidate.id === id);
          if (!model) throw new Error("no model " + id);
          model.state = state;
          model.active = active;
        },
      });
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
        themeLight: "ember",
        themeDark: "ember",
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
        glass: false,
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
                    document.querySelector("[role=main]")!,
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

async function choose(page: Page, label: string, option: string) {
  await page.getByRole("combobox", { name: label, exact: true }).click();
  await page.getByRole("option", { name: option, exact: true }).click();
  await expect(
    page.getByRole("combobox", { name: label, exact: true }),
  ).toContainText(option);
}

test("preferences save, shortcut failures preserve the old value, and placeholders navigate", async ({
  page,
}) => {
  await openSettings(page);
  await expect(
    page.getByRole("heading", { name: "General", exact: true }),
  ).toBeVisible();
  await choose(page, "Shortcut mode", "Toggle");
  await expect(
    page.getByText("Tap to record. Tap again to insert your words."),
  ).toBeVisible();
  await choose(page, "Microphone", "USB Microphone");
  await page.getByRole("button", { name: "Test microphone" }).click();
  await expect(page.getByRole("meter")).toHaveAttribute("value", "0.6");
  await page.getByRole("button", { name: "Stop test" }).click();
  await page.getByRole("switch", { name: "Launch at login" }).click();
  await expect(page.getByRole("switch")).toBeChecked();
  await page.getByRole("button", { name: /^Shortcut,/ }).click();
  await page.keyboard.press("Control+Shift+D");
  await expect(page.getByRole("button", { name: /^Shortcut,/ })).toContainText(
    "⌃⇧D",
  );
  await page.getByRole("button", { name: /^Shortcut,/ }).click();
  await page.keyboard.press("Meta+Q");
  await expect(
    page.getByText("Used by another app — pick another", { exact: true }),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: /^Shortcut,/ })).toContainText(
    "⌃⇧D",
  );
  await expect(
    page.getByRole("combobox", { name: "Speech model", exact: true }),
  ).toContainText("Base");
  await page.getByRole("button", { name: "Manage models" }).click();
  await expect(
    page.getByRole("region", { name: "Base English" }),
  ).toContainText("In use");
  await page.getByRole("button", { name: "Output", exact: true }).click();
  await choose(page, "After dictation", "Copy only");
  await choose(page, "After dictation", "Paste and keep on clipboard");
  await page.getByRole("button", { name: "Advanced", exact: true }).click();
  await page.getByRole("switch", { name: "Sound feedback" }).click();
  await expect(
    page.getByRole("switch", { name: "Sound feedback" }),
  ).toBeChecked();
  await page.getByRole("button", { name: "General", exact: true }).click();
  await expect(
    page.getByRole("combobox", { name: "Microphone", exact: true }),
  ).toContainText("USB Microphone");
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
    await expect(page.getByText(/^Version /)).toBeVisible();
    await page.screenshot({
      path: `target/settings-${colorScheme}-browser.png`,
    });
    await page.setViewportSize({ width: 390, height: 844 });
    // Below the window's own 640 floor the shell hands navigation to a drawer,
    // so the narrowest layout is read as a drawer rather than the sidebar.
    const openNav = page.getByRole("button", { name: "Open navigation" });
    await expect(openNav).toBeVisible();
    await openNav.click();
    await expect(
      page.getByRole("button", { name: "General", exact: true }),
    ).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(
      page.getByRole("button", { name: "General", exact: true }),
    ).toBeHidden();
    expect(
      await page.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
    ).toBe(true);
    await openNav.click();
    await page.getByRole("button", { name: "History", exact: true }).click();
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
    "Using defaults. Your old file is saved beside them.",
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
          themeLight: "ember",
          themeDark: "ember",
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
          glass: false,
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

test("each appearance uses the theme stored for it", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await openSettings(page, {
    stored: {
      themeLight: "paper",
      themeDark: "capy",
      appearanceMode: "system",
    },
  });
  await expect(page.locator("html")).toHaveAttribute("data-palette", "capy");
  await expect(page.locator("html")).toHaveAttribute("data-mode", "dark");
  await page.emulateMedia({ colorScheme: "light" });
  await expect(page.locator("html")).toHaveAttribute("data-palette", "paper");
  await expect(page.locator("html")).toHaveAttribute("data-mode", "light");
});

test("the light and dark columns choose their themes independently", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light" });
  await openSettings(page);
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const light = page.getByRole("radiogroup", { name: "Theme in light" });
  const dark = page.getByRole("radiogroup", { name: "Theme in dark" });
  await light.getByRole("radio", { name: /^Paper/ }).click();
  await dark.getByRole("radio", { name: /^Capy/ }).click();
  await expect(light.getByRole("radio", { name: /^Paper/ })).toBeChecked();
  await expect(dark.getByRole("radio", { name: /^Capy/ })).toBeChecked();
  // System shows the theme that belongs to the Mac's side.
  await expect(page.locator("html")).toHaveAttribute("data-palette", "paper");
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(page.locator("html")).toHaveAttribute("data-palette", "capy");
  // Pinning a side overrides the Mac without touching either choice.
  await page.getByRole("radio", { name: "Light", exact: true }).click();
  await expect(page.locator("html")).toHaveAttribute("data-palette", "paper");
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(page.locator("html")).toHaveAttribute("data-palette", "paper");
  await expect(light.getByRole("radio", { name: /^Paper/ })).toBeChecked();
});

test("each column previews its own appearance and marks the one in use", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await openSettings(page);
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const light = page.getByRole("radiogroup", { name: "Theme in light" });
  const dark = page.getByRole("radiogroup", { name: "Theme in dark" });
  const previewed = (column: typeof light) =>
    column.evaluate((group) => [
      ...new Set(
        [...group.querySelectorAll("[data-theme]")].map((el) =>
          el.getAttribute("data-theme"),
        ),
      ),
    ]);
  expect(await previewed(light)).toEqual(["light"]);
  expect(await previewed(dark)).toEqual(["dark"]);
  // Exactly one column says it is on screen. The marker sits above its own
  // list, so counting the theme groups after it names the column: two means
  // the light one, one the dark one.
  const markerColumn = () =>
    page.evaluate(() => {
      const marker = [...document.querySelectorAll("*")].find(
        (el) =>
          el.childElementCount === 0 && el.textContent?.trim() === "In use now",
      )!;
      const groups = [...document.querySelectorAll("[role=radiogroup]")].filter(
        (group) => {
          const id = group.getAttribute("aria-labelledby");
          return (
            id &&
            document.getElementById(id)?.textContent?.startsWith("Theme in ")
          );
        },
      );
      return groups.filter(
        (group) =>
          marker.compareDocumentPosition(group) &
          Node.DOCUMENT_POSITION_FOLLOWING,
      ).length;
    });
  await expect(page.getByText("In use now")).toHaveCount(1);
  expect(await markerColumn()).toBe(1);
  const modeLight = page.getByRole("radio", { name: "Light", exact: true });
  await expect(modeLight).toBeEnabled();
  await modeLight.click();
  await expect(modeLight).toBeChecked();
  await expect(page.locator("html")).toHaveAttribute("data-mode", "light");
  await expect(page.getByText("In use now")).toHaveCount(1);
  expect(await markerColumn()).toBe(2);
});

const themeCases = [
  { id: "ember", name: "Ember", slug: "ember" },
  { id: "paper", name: "Paper", slug: "paper" },
  { id: "capy", name: "Capy", slug: "capy" },
  { id: "rosePine", name: "Rosé Pine", slug: "rose-pine" },
  { id: "neutral", name: "Neutral", slug: "zinc" },
] as const;
for (const theme of themeCases) {
  test(`${theme.name} appearance saves live and System follows macOS`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await openSettings(page);
    await page.getByRole("button", { name: "Appearance", exact: true }).click();
    await expect(
      page.getByRole("heading", { name: "Appearance", level: 1 }),
    ).toBeVisible();
    const radio = page
      .getByRole("radiogroup", { name: "Theme in light" })
      .getByRole("radio", { name: new RegExp(`^${theme.name}`) });
    await radio.click();
    await expect(radio).toBeChecked();
    await expect(page.locator("html")).toHaveAttribute(
      "data-palette",
      theme.id,
    );
    await expect(
      page.locator(`[data-astryx-theme="${theme.slug}"]`).first(),
    ).toBeAttached();
    await page.getByRole("radio", { name: "Dark", exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute("data-mode", "dark");
    await page.emulateMedia({ colorScheme: "light" });
    await expect(page.locator("html")).toHaveAttribute("data-mode", "dark");
    await page.getByRole("radio", { name: "System", exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute("data-mode", "light");
    await page.emulateMedia({ colorScheme: "dark" });
    await expect(page.locator("html")).toHaveAttribute("data-mode", "dark");
    await page.getByRole("button", { name: "About", exact: true }).click();
    await page.getByRole("button", { name: "Appearance", exact: true }).click();
    await expect(radio).toBeChecked();
    await expect(
      page.getByRole("radio", { name: "System", exact: true }),
    ).toBeChecked();
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
                themeLight: theme,
                themeDark: theme,
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
                glass: false,
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
        payload: { themeLight: "invalid" },
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
      stored: {
        themeLight: "paper",
        themeDark: "paper",
        appearanceMode: "dark",
      },
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
      themeLight: "ember",
      appearanceMode: "light",
    });
    await expect(page.locator("html")).toHaveAttribute("data-palette", "ember");
    await expect(page.locator("html")).toHaveAttribute("data-mode", "light");
    // An invalid event must not call a command the panel isn't granted; the
    // valid one after it proves the invalid one has been handled.
    await send({ themeLight: "invalid" });
    await send({ ...(current as object), themeLight: "paper" });
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
  const backgrounds: Record<string, string> = {};
  for (const theme of themeCases.map(({ id }) => id))
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
                themeLight: theme,
                themeDark: theme,
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
                glass: false,
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
          backgrounds[`${theme}-${appearanceMode}`] = await page
            .locator(".ui-capsule")
            .evaluate((capsule) => getComputedStyle(capsule).backgroundColor);
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
  for (const theme of ["capy", "rosePine", "neutral"])
    for (const mode of ["light", "dark"])
      expect(backgrounds[`${theme}-${mode}`]).not.toBe(
        backgrounds[`ember-${mode}`],
      );
});

for (const exit of ["tab", "window-blur", "navigation"]) {
  test(`pending shortcut capture stays canceled after ${exit}`, async ({
    page,
  }) => {
    await openSettings(page, { delayCapture: true });
    const recorder = page.getByRole("button", {
      name: /^Shortcut,/,
    });
    await recorder.click();
    await expect(recorder).toHaveAccessibleName(/Press shortcut Esc to cancel/);
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
    // Navigating away unmounts General, so the recorder is only there to read
    // once the page is back. Reading it mid-transition raced the unmount.
    if (exit === "navigation")
      await page.getByRole("button", { name: "General", exact: true }).click();
    await expect(recorder).toBeVisible();
    await expect(recorder).not.toHaveAccessibleName(/Press shortcut/);
    // The capture that was in flight finishes now, and a canceled capture must
    // not commit a chord nobody chose.
    await page.evaluate(() =>
      (window as unknown as { __finishCapture: () => void }).__finishCapture(),
    );
    await expect(recorder).toHaveAccessibleName(
      "Shortcut, Alt + Shift + Space",
    );
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
    await expect(
      page.getByRole("img", { name: "Alt + Shift + A" }).locator("kbd"),
    ).toHaveText(["⌥", "⇧", "A"]);
    await expect(annotations).toBeChecked();
    await page.screenshot({
      path: `target/visibility-fix/history-default-${mode}.png`,
    });
  });
}

for (const theme of themeCases)
  for (const mode of ["light", "dark"] as const) {
    test(`${theme.name} ${mode} keeps every text and control pair readable`, async ({
      page,
    }) => {
      await openSettings(page, {
        stored: {
          themeLight: theme.id,
          themeDark: theme.id,
          appearanceMode: mode,
        },
      });
      await expect(page.getByRole("switch")).toBeVisible();
      await expect(page.getByRole("main")).toHaveCSS("color-scheme", mode);
      const pairs = await page.getByRole("main").evaluate((el) => {
        const probe = document.createElement("canvas");
        el.append(probe);
        const context = probe.getContext("2d", { willReadFrequently: true })!;
        const luminance = (color: string) => {
          probe.style.color = color;
          context.clearRect(0, 0, 1, 1);
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
        const ratio = (a: string, b: string) => {
          const x = luminance(a),
            y = luminance(b);
          return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
        };
        const token = (name: string) => `var(--${name})`;
        const pairs: { pair: string; ratio: number; min: number }[] = [];
        for (const foreground of ["color-text-primary", "color-text-secondary"])
          for (const background of [
            "color-background-body",
            "color-background-muted",
            "color-background-surface",
            "color-background-card",
            "color-background-popover",
            "color-neutral",
          ])
            pairs.push({
              pair: `${foreground} on ${background}`,
              ratio: ratio(token(foreground), token(background)),
              min: 4.5,
            });
        for (const background of [
          "color-background-body",
          "color-background-surface",
        ])
          pairs.push(
            {
              pair: `color-text-accent on ${background}`,
              ratio: ratio(token("color-text-accent"), token(background)),
              min: 4.5,
            },
            {
              pair: `focus-outline-color on ${background}`,
              ratio: ratio(token("focus-outline-color"), token(background)),
              min: 3,
            },
            {
              pair: `color-border-emphasized on ${background}`,
              ratio: ratio(token("color-border-emphasized"), token(background)),
              min: 3,
            },
          );
        pairs.push({
          pair: "color-on-accent on color-accent",
          ratio: ratio(token("color-on-accent"), token("color-accent")),
          min: 4.5,
        });
        probe.remove();
        return pairs;
      });
      for (const { pair, ratio, min } of pairs)
        expect(ratio, pair).toBeGreaterThanOrEqual(min);
    });
  }

test("History preserves an explicitly saved Annotate shortcut", async ({
  page,
}) => {
  await openSettings(page, { stored: { annotateShortcut: "Alt+Shift+KeyS" } });
  await page.getByRole("button", { name: "History", exact: true }).click();
  await page.getByRole("radio", { name: "Annotations", exact: true }).check();
  await expect(
    page.getByRole("img", { name: "Alt + Shift + S" }).locator("kbd"),
  ).toHaveText(["⌥", "⇧", "S"]);
});

for (const failLoad of [false, true]) {
  test(`Settings readiness does not wait for hidden animation frames, load failure ${failLoad}`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: "light" });
    await openSettings(page, {
      suspendFrames: true,
      failLoad,
      stored: {
        themeLight: "paper",
        themeDark: "paper",
        appearanceMode: "dark",
      },
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
        libraryTheme: failLoad ? "ember" : "paper",
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
    stored: { appearanceMode: "dark", themeLight: "paper", themeDark: "paper" },
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
    await page.getByRole("button", { name: /^Shortcut,/ }).click();
    await expect(
      page.getByRole("button", { name: /^Shortcut,/ }),
    ).toHaveAccessibleName(/Press shortcut Esc to cancel/);
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
      name: /^Shortcut,/,
    });
    // The capture state keeps an invisible copy of its own content so the
    // button cannot resize, so only the keys that paint are read.
    await expect(recorder.locator("kbd:visible")).toHaveText([
      "⌥",
      "⇧",
      "SPACE",
    ]);
    const clipped = await recorder.evaluate((button) =>
      [...button.querySelectorAll("kbd")]
        .filter((kbd) => kbd.checkVisibility())
        .flatMap((kbd) => {
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

test("settings controls share one compact corner, not a pill", async ({
  page,
}) => {
  await openSettings(page);
  await expect(page.getByRole("switch")).toBeAttached();
  const shapes = await page.getByRole("main").evaluate((main) =>
    [
      ...main.querySelectorAll(
        "li button:not([role=switch]), li [role=combobox]",
      ),
    ]
      .filter((el) => el.getBoundingClientRect().width > 0)
      .map((el) => {
        const painted =
          el.getAttribute("role") === "combobox" ? el.parentElement! : el;
        const { height } = painted.getBoundingClientRect();
        return {
          name: el.getAttribute("aria-label") ?? el.textContent ?? "",
          radius: parseFloat(getComputedStyle(painted).borderTopLeftRadius),
          half: height / 2,
        };
      }),
  );
  expect(shapes.length).toBeGreaterThan(3);
  expect([...new Set(shapes.map(({ radius }) => radius))]).toHaveLength(1);
  for (const { radius, half } of shapes) expect(radius).toBeLessThan(half);
});

test("settings page transitions target the main pane", async ({ page }) => {
  await openSettings(page);
  await expect(
    page
      .getByRole("main")
      .evaluate((main) => getComputedStyle(main).viewTransitionName),
  ).resolves.toBe("settings-page");
});

test("settings polish: sidebar order, Appearance page, resting buttons, two weights, releases link", async ({
  page,
}) => {
  await openSettings(page);
  const nav = page
    .getByRole("navigation")
    .getByRole("button")
    .filter({ hasNotText: "Check for updates" });
  await expect(nav).toHaveText([
    "General",
    "Appearance",
    "Models",
    "Annotate",
    "History",
    "Output",
    "Advanced",
    "About",
  ]);
  await expect(
    page.getByRole("radiogroup", { name: /^Theme in / }),
  ).toHaveCount(0);
  const tops = await nav.evaluateAll((items) =>
    items.map((item) => item.getBoundingClientRect()),
  );
  for (let index = 1; index < tops.length; index++)
    expect(tops[index].top).toBeGreaterThan(tops[index - 1].bottom);
  const manage = page.getByRole("button", {
    name: "Manage models",
    exact: true,
  });
  await expect(manage).toHaveCSS("box-shadow", "none");
  await manage.hover();
  await expect(manage).toHaveCSS("box-shadow", /inset/);
  const weights = await page
    .getByRole("main")
    .evaluate((main) => [
      ...new Set(
        [...main.querySelectorAll("*")]
          .filter((element) =>
            [...element.childNodes].some(
              (node) => node.nodeType === 3 && node.textContent?.trim(),
            ),
          )
          .map((element) => getComputedStyle(element).fontWeight),
      ),
    ]);
  expect(weights.sort()).toEqual(["400", "600"]);
  await page.getByRole("button", { name: /Check for updates/ }).click();
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as unknown as { __commands: string[] }).__commands.includes(
          "open_location",
        ),
      ),
    )
    .toBe(true);
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  await expect(
    page.getByRole("radiogroup", { name: "Theme in light" }),
  ).toBeVisible();
  await expect(
    page.getByRole("radiogroup", { name: "Theme in dark" }),
  ).toBeVisible();
});

test("a model card keeps its height as its state changes, so the rows below never move", async ({
  page,
}) => {
  await openSettings(page, { pendingPage: "Models" });
  const card = page.locator('section[aria-label="Small English"]');
  await expect(card).toBeVisible();
  const below = page.locator('section[aria-label="Medium English"]');
  const moved = async () => ({
    height: Math.round((await card.boundingBox())!.height),
    top: (await card.boundingBox())!.top,
    belowTop: (await below.boundingBox())!.top,
  });
  const setState = async (state: unknown, active = false) => {
    await page.evaluate(
      ({ state, active }) => {
        (
          window as unknown as {
            __setModelState: (id: string, s: unknown, a?: boolean) => void;
          }
        ).__setModelState("small.en", state, active);
        window.postMessage(
          { type: "dictate-test", event: "models-changed", payload: null },
          "*",
        );
      },
      { state, active },
    );
  };
  const states: [unknown, RegExp, boolean?][] = [
    [{ kind: "available" }, /Download Small English/],
    [
      {
        kind: "downloading",
        received: 40_000_000,
        total: 466_000_000,
        cancelling: false,
      },
      /Cancel Small English download/,
    ],
    [{ kind: "installed" }, /Use Small English/],
    [{ kind: "installed" }, /Used for every dictation/, true],
    [
      { kind: "failed", message: "Download failed. Check your connection." },
      /Retry Small English download/,
    ],
  ];
  const seen = [];
  for (const [state, label, active] of states) {
    await setState(state, active ?? false);
    await expect(
      card
        .getByRole("button", { name: label })
        .or(card.getByText(label))
        .first(),
    ).toBeVisible();
    seen.push(await moved());
  }
  expect([...new Set(seen.map(({ height }) => height))]).toHaveLength(1);
  expect([...new Set(seen.map(({ belowTop }) => belowTop))]).toHaveLength(1);
  expect([...new Set(seen.map(({ top }) => top))]).toHaveLength(1);
});

test("cards rest on a hairline and controls stay borderless until hover", async ({
  page,
}) => {
  await openSettings(page);
  const card = page.getByRole("main").locator(".astryx-card").first();
  await expect(card).toBeVisible();
  const border = await card.evaluate((el) => {
    const style = getComputedStyle(el);
    return { width: style.borderTopWidth, color: style.borderTopColor };
  });
  expect(parseFloat(border.width)).toBeLessThanOrEqual(1);
  const alpha = /\/\s*([\d.]+)\s*\)/.exec(border.color);
  expect(alpha ? Number(alpha[1]) : 1).toBeLessThan(0.6);
  const field = page.getByRole("searchbox").first();
  if (await field.count()) {
    const painted = field.locator("xpath=..");
    await expect(painted).toHaveCSS(
      "border-top-color",
      /rgba\(0, 0, 0, 0\)|transparent/,
    );
    await painted.hover();
    await expect(painted).not.toHaveCSS(
      "border-top-color",
      /rgba\(0, 0, 0, 0\)|transparent/,
    );
  }
});

test("theme previews show the theme's own colours and no status words", async ({
  page,
}) => {
  await openSettings(page, { pendingPage: "Appearance" });
  const themes = page.getByRole("radiogroup", { name: /^Theme in / });
  await expect(themes).toHaveCount(2);
  await expect(themes.locator(".astryx-badge")).toHaveCount(0);
  await expect(themes.getByText("Ready")).toHaveCount(0);
  await expect(themes.getByText("Inserted")).toHaveCount(0);
  const dots = await themes
    .locator(".astryx-status-dot")
    .evaluateAll((els) => [
      ...new Set(els.map((el) => getComputedStyle(el).backgroundColor)),
    ]);
  expect(dots.length).toBeGreaterThan(1);
});

test("the sidebar sits a step behind the page in every theme and mode", async ({
  page,
}) => {
  for (const theme of ["ember", "paper", "capy", "rosePine", "neutral"]) {
    for (const mode of ["light", "dark"]) {
      // A tab each: the settings mock seeds settings at load, so a second
      // mount in one tab keeps the first theme it saw.
      const tab = await page.context().newPage();
      await openSettings(tab, {
        stored: {
          themeLight: theme,
          themeDark: theme,
          appearanceMode: mode,
        },
      });
      await expect(tab.getByRole("navigation")).toBeVisible();
      await expect(tab.locator("html")).toHaveAttribute("data-mode", mode);
      const surfaces = await tab.evaluate(() => {
        const nav = document.querySelector(".astryx-side-nav")!;
        const main =
          document.querySelector("[role=main]") ??
          document.querySelector("main") ??
          document.body;
        const pick = (el: Element) => getComputedStyle(el).backgroundColor;
        return { nav: pick(nav), main: pick(main) };
      });
      expect(
        surfaces.nav,
        `${theme} ${mode} leaves the sidebar unpainted`,
      ).not.toMatch(/rgba\(0, 0, 0, 0\)|transparent/);
      expect(surfaces.nav, `${theme} ${mode} paints one surface`).not.toBe(
        surfaces.main,
      );
      await tab.close();
    }
  }
});

test("the translucent window steps the surfaces aside only while it is on", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const surfaces = () =>
    page.evaluate(() => {
      const pick = (selector: string) => {
        const el = document.querySelector(selector);
        return el ? getComputedStyle(el).backgroundColor : "absent";
      };
      return {
        glass: document.documentElement.dataset.glass ?? null,
        shell: pick(".astryx-app-shell"),
        main: pick(".astryx-app-shell [role=main]"),
        nav: pick(".astryx-side-nav"),
        card: pick(".astryx-card"),
      };
    });
  const off = await surfaces();
  expect(off.glass).toBeNull();
  expect(off.shell).not.toBe("rgba(0, 0, 0, 0)");
  const toggle = page.getByRole("switch", { name: "Translucent window" });
  await expect(toggle).toBeEnabled();
  await toggle.click();
  await expect(toggle).toBeChecked();
  const on = await surfaces();
  expect(on.glass).toBe("true");
  // The window's own backdrop steps aside for the material macOS draws there,
  // and the page and sidebar keep most of their surface.
  expect(on.shell).toBe("rgba(0, 0, 0, 0)");
  expect(on.main).toMatch(/\/ 0\.88\)$/);
  expect(on.nav).toMatch(/\/ 0\.92\)$/);
  // Cards stay opaque, so text keeps the contrast it was measured at.
  expect(on.card).toBe(off.card);
  await page.getByRole("button", { name: "About", exact: true }).click();
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  await expect(
    page.getByRole("switch", { name: "Translucent window" }),
  ).toBeChecked();
  expect((await surfaces()).glass).toBe("true");
  await page.getByRole("switch", { name: "Translucent window" }).click();
  await expect.poll(async () => (await surfaces()).glass).toBeNull();
  expect((await surfaces()).shell).toBe(off.shell);
});

test("the model metrics stack one metric per line", async ({ page }) => {
  await openSettings(page, { pendingPage: "Models" });
  const card = page.locator('section[aria-label="Small English"]');
  await expect(card).toBeVisible();
  const rows = await card.evaluate((el) => {
    const stack = el.querySelector(".ui-model-metrics")!;
    return [...stack.children].map((row) => ({
      height: Math.round(row.getBoundingClientRect().height),
      parts: [...row.children].map((part) => {
        const box = part.getBoundingClientRect();
        return {
          height: Math.round(box.height),
          top: Math.round(box.top),
          left: Math.round(box.left),
          text: (part.textContent ?? "").trim(),
        };
      }),
    }));
  });
  expect(rows).toHaveLength(3);
  for (const row of rows) {
    expect(row.height).toBeLessThanOrEqual(24);
    for (const part of row.parts) expect(part.height).toBeLessThanOrEqual(24);
  }
  expect(rows.map((row) => row.parts[0].text)).toEqual([
    "Accuracy",
    "Speed",
    "Size",
  ]);
  expect(new Set(rows.map((row) => row.parts[0].left)).size).toBe(1);
  for (let index = 1; index < rows.length; index++)
    expect(rows[index].parts[0].top).toBeGreaterThan(
      rows[index - 1].parts[0].top,
    );
});

test("a label and its description never run together", async ({ page }) => {
  const names = [
    "General",
    "Appearance",
    "Models",
    "Annotate",
    "History",
    "Output",
    "Advanced",
    "About",
  ];
  await openSettings(page);
  for (const name of names) {
    await page
      .getByRole("navigation")
      .getByRole("button", { name, exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name, exact: true, level: 1 }),
    ).toBeVisible();
    const touching = await page.getByRole("main").evaluate((main) =>
      [
        ...main.querySelectorAll(
          ".astryx-text, .astryx-heading, .astryx-field-label",
        ),
      ]
        .map((el) => {
          const next = el.nextElementSibling;
          if (!next || !(next.textContent ?? "").trim()) return null;
          const above = el.getBoundingClientRect();
          const below = next.getBoundingClientRect();
          if (!above.height || !below.height) return null;
          if (!el.checkVisibility() || !next.checkVisibility()) return null;
          const stacked = Math.abs(above.left - below.left) < 2;
          const gap = Math.round(below.top - above.bottom);
          if (!stacked || gap > 1 || gap < 0) return null;
          return {
            label: (el.textContent ?? "").slice(0, 40),
            description: (next.textContent ?? "").slice(0, 40),
            gap,
          };
        })
        .filter(Boolean),
    );
    expect(touching, `${name} stacks a label on its description`).toEqual([]);
  }
});

test("cancelling the shortcut recorder leaves nothing painted", async ({
  page,
}) => {
  await openSettings(page);
  const recorder = page.getByRole("button", { name: /^Shortcut,/ });
  await expect(recorder).toBeVisible();
  const paint = () =>
    recorder.evaluate((el) => {
      const style = getComputedStyle(el);
      return { boxShadow: style.boxShadow, outline: style.outlineStyle };
    });
  await page.mouse.move(0, 0);
  const resting = await paint();
  await recorder.click();
  await expect(recorder).toHaveAccessibleName(/Press shortcut Esc to cancel/);
  await page.keyboard.press("Escape");
  await expect(recorder).toHaveAccessibleName(/^Shortcut, /);
  await page.mouse.move(0, 0);
  await expect.poll(async () => (await paint()).outline).toBe(resting.outline);
  expect((await paint()).boxShadow).toBe(resting.boxShadow);
});

const settingsRows = (page: Page) =>
  page.getByRole("main").evaluate(() =>
    [...document.querySelectorAll("li.astryx-item")].map((row) => {
      const box = row.getBoundingClientRect();
      return {
        height: Math.round(box.height),
        top: Math.round(box.top),
        label: (row.textContent ?? "").trim().slice(0, 24),
      };
    }),
  );

test("every stateful row keeps its height and the rows below it never move", async ({
  page,
}) => {
  // A row allowed to change height is named here, on purpose. It is empty:
  // no settings row may grow or shrink when its control changes state.
  const allowed: string[] = [];
  await openSettings(page);
  const compare = async (
    what: string,
    before: Awaited<ReturnType<typeof settingsRows>>,
  ) => {
    const after = await settingsRows(page);
    expect(
      after.map((row) => row.height),
      what + " heights",
    ).toEqual(before.map((row) => row.height));
    expect(
      after.map((row) => row.top),
      what + " tops",
    ).toEqual(before.map((row) => row.top));
    expect(allowed, what).toEqual([]);
  };
  const before = await settingsRows(page);

  await page.getByRole("button", { name: "Test microphone" }).click();
  await expect(page.getByRole("button", { name: "Stop test" })).toBeVisible();
  await compare("Test microphone listening", before);
  await page.getByRole("button", { name: "Stop test" }).click();
  await expect(
    page.getByRole("button", { name: "Test microphone" }),
  ).toBeVisible();
  await compare("Test microphone stopped", before);

  const launch = page.getByRole("switch", { name: "Launch at login" });
  await launch.click();
  await compare("Launch at login on", before);
  await launch.click();
  await compare("Launch at login off", before);

  const recorder = page.getByRole("button", { name: /^Shortcut,/ });
  await recorder.click();
  await page.keyboard.press("Escape");
  await expect(recorder).toHaveAccessibleName(/^Shortcut, /);
  await compare("shortcut capture", before);

  await page
    .getByRole("navigation")
    .getByRole("button", { name: "Annotate", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Annotate", exact: true, level: 1 }),
  ).toBeVisible();
  const annotateBefore = await settingsRows(page);
  // Only the plain switches: turning Annotate itself on asks macOS for Screen
  // Recording, and that answer is a banner, not a row.
  for (const name of ["Show in screen recordings"]) {
    const toggle = page.getByRole("switch", { name });
    await toggle.click();
    await compare(name + " on", annotateBefore);
    await toggle.click();
    await compare(name + " off", annotateBefore);
  }
});

test("a keycap is a different surface from the button it sits in", async ({
  page,
}) => {
  for (const theme of ["ember", "paper", "capy", "rosePine", "neutral"]) {
    for (const mode of ["light", "dark"]) {
      const tab = await page.context().newPage();
      await openSettings(tab, {
        stored: {
          themeLight: theme,
          themeDark: theme,
          appearanceMode: mode,
        },
      });
      const slug = {
        ember: "ember",
        paper: "paper",
        capy: "capy",
        rosePine: "rose-pine",
        neutral: "zinc",
      }[theme];
      await expect(tab.locator("html")).toHaveAttribute(
        "data-astryx-theme",
        slug!,
      );
      await expect(tab.locator("html")).toHaveAttribute("data-mode", mode);
      const first = tab.getByRole("button", { name: /^Shortcut,/ }).first();
      await expect(first).toBeVisible();
      // Composited luminance, not a string compare: a 92% fill and an opaque
      // one of the same colour are different strings and the same surface.
      const surfaces = await first.evaluate((el) => {
        const keycap = el.querySelector("kbd");
        if (!keycap) return null;
        const card = el.closest(".astryx-card");
        const canvas = document.createElement("canvas");
        canvas.width = canvas.height = 1;
        const context = canvas.getContext("2d")!;
        const paint = (layers: string[]) => {
          context.clearRect(0, 0, 1, 1);
          for (const layer of layers) {
            context.fillStyle = layer;
            context.fillRect(0, 0, 1, 1);
          }
          const [r, g, b] = context.getImageData(0, 0, 1, 1).data;
          const channel = (value: number) => {
            const srgb = value / 255;
            return srgb <= 0.03928
              ? srgb / 12.92
              : ((srgb + 0.055) / 1.055) ** 2.4;
          };
          return (
            0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
          );
        };
        const under = [getComputedStyle(card ?? document.body).backgroundColor];
        const button = paint([...under, getComputedStyle(el).backgroundColor]);
        const withKeycap = paint([
          ...under,
          getComputedStyle(el).backgroundColor,
          getComputedStyle(keycap).backgroundColor,
        ]);
        const lighter = Math.max(button, withKeycap);
        const darker = Math.min(button, withKeycap);
        return {
          ratio: (lighter + 0.05) / (darker + 0.05),
          buttonCss: getComputedStyle(el).backgroundColor,
          keycapCss: getComputedStyle(keycap).backgroundColor,
        };
      });
      expect(surfaces, theme + " " + mode).not.toBeNull();
      expect(surfaces!.ratio, theme + " " + mode).toBeGreaterThan(1.1);
      await tab.close();
    }
  }
});

test("a shortcut capture adds nothing to the row and keeps the button its size", async ({
  page,
}) => {
  await openSettings(page);
  const recorder = page.getByRole("button", { name: /^Shortcut,/ });
  await expect(recorder).toBeVisible();
  const box = async () => {
    const rect = await recorder.boundingBox();
    return { width: Math.round(rect!.width), height: Math.round(rect!.height) };
  };
  const rowOf = (locator: typeof recorder) =>
    locator.evaluate((el) => {
      const row = el.closest("li")!;
      const rect = row.getBoundingClientRect();
      return { top: Math.round(rect.top), height: Math.round(rect.height) };
    });
  const restBox = await box();
  const restRow = await rowOf(recorder);
  await recorder.click();
  await expect(recorder).toHaveAccessibleName(/Press shortcut Esc to cancel/);
  await expect(recorder).toHaveText(/Press shortcut/);
  expect(await box()).toEqual(restBox);
  expect(await rowOf(recorder)).toEqual(restRow);
  await expect(
    recorder.locator(
      ".ui-capture-live:not(.ui-capture-ghost) .astryx-status-dot",
    ),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(recorder).toHaveAccessibleName(/^Shortcut, /);
  expect(await box()).toEqual(restBox);
  expect(await rowOf(recorder)).toEqual(restRow);
});

test("cancelling leaves the row's own hairline and no focus line", async ({
  page,
}) => {
  await openSettings(page);
  const rows = page.getByRole("main").locator("li.astryx-item");
  // The list draws its own hairline under every row but the last, and that
  // stays: what has to go is the thicker line a cancelled capture leaves.
  const divider = await rows.first().evaluate((row) => {
    const style = getComputedStyle(row);
    return style.borderBottomWidth + " " + style.borderBottomStyle;
  });
  expect(divider).toBe("1px solid");
  const recorder = page.getByRole("button", { name: /^Shortcut,/ });
  await recorder.click();
  await page.keyboard.press("Escape");
  // Measured cause: the button's own outline is none after Escape, and the
  // 2px ring is painted on the ROW by the library's :has(:focus-visible) rule.
  // So the row is what this reads, and the hairline under it must survive.
  const ring = () =>
    recorder.evaluate((el) => {
      const row = el.closest("li")!;
      const style = getComputedStyle(row);
      return {
        width: parseFloat(style.outlineWidth),
        divider: style.borderBottomWidth,
      };
    });
  await expect.poll(async () => (await ring()).width).toBe(0);
  expect((await ring()).divider).toBe("1px");
  await expect
    .poll(() => recorder.evaluate((el) => getComputedStyle(el).outlineStyle))
    .toBe("none");
  // Quieting lasts only until the next key, so a keyboard user still gets a
  // ring the moment they move.
  await page.keyboard.press("Shift");
  await expect
    .poll(() =>
      recorder.evaluate((el) =>
        getComputedStyle(el.closest("li")!)
          .getPropertyValue("--focus-outline-width")
          .trim(),
      ),
    )
    .not.toBe("0px");
  await page.keyboard.press("Tab");
  await expect
    .poll(() =>
      recorder.evaluate(
        (el) => getComputedStyle(el.closest("li")!).outlineStyle,
      ),
    )
    .toBe("none");
});

test("the microphone button keeps one width across its two labels", async ({
  page,
}) => {
  await openSettings(page);
  const button = page.getByRole("button", { name: "Test microphone" });
  const resting = await button.boundingBox();
  await button.click();
  const stopped = page.getByRole("button", { name: "Stop test" });
  await expect(stopped).toBeVisible();
  const listening = await stopped.boundingBox();
  expect(Math.round(listening!.width)).toBe(Math.round(resting!.width));
  await expect(stopped).toHaveAccessibleName("Stop test");
});

test("the active model card carries its own hairline, not only a badge", async ({
  page,
}) => {
  await openSettings(page, { pendingPage: "Models" });
  const active = page.locator(
    'section[aria-label="Base English"] .astryx-card',
  );
  await expect(active).toBeVisible();
  const shadow = await active.evaluate((el) => getComputedStyle(el).boxShadow);
  expect(shadow).toMatch(/inset/);
  expect(shadow).not.toBe("none");
  const idle = page.locator('section[aria-label="Tiny English"] .astryx-card');
  const idleShadow = await idle.evaluate(
    (el) => getComputedStyle(el).boxShadow,
  );
  expect(idleShadow).not.toMatch(/inset/);
});
