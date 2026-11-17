import { expect, test, type Locator, type Page } from "@playwright/test";
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
    history = [],
    annotations = [],
    delaySave = false,
    saveError = null,
  }: {
    failLoad?: boolean;
    startupError?: { title: string; fix: string; detail: string | null } | null;
    overlay?: boolean;
    window?: WindowLabel;
    stored?: Record<string, unknown>;
    pendingPage?: string | null;
    delayCapture?: boolean;
    suspendFrames?: boolean;
    history?: { id: number }[];
    annotations?: { id: string }[];
    delaySave?: boolean;
    saveError?: string | null;
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
      history,
      annotations,
      delaySave,
      saveError,
    }) => {
      if (suspendFrames) window.requestAnimationFrame = () => 1;
      const denied: string[] = [];
      Object.defineProperty(window, "__deniedCommands", { value: denied });
      let status = { phase: "idle", error: startupError };
      const callbacks = new Map<number, (response: unknown) => void>();
      const listeners = new Map<string, { id: number; handler: number }[]>();
      const commands: string[] = [];
      Object.defineProperty(window, "__commands", { value: commands });
      Object.defineProperty(window, "__deleted", { value: [] });
      Object.defineProperty(window, "__deleteHooks", { value: {} });
      Object.defineProperty(window, "__addTake", {
        value: (take: { id: number }) => {
          history = [take, ...history];
          emit("history-changed", null);
        },
      });
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
              id?: number | string;
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
              status.phase !== "idle"
            )
              throw status.phase === "annotating"
                ? "Stop Annotate before recording a shortcut"
                : "Finish dictating before recording a shortcut";
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
              if (delaySave)
                await new Promise<void>((resolve) => {
                  Object.defineProperty(window, "__finishSave", {
                    value: resolve,
                    configurable: true,
                  });
                });
              if (saveError) throw saveError;
              if (args.settings?.shortcut === "Super+KeyQ")
                throw "Shortcut unavailable";
              if (args.settings?.annotateShortcut === settings.shortcut)
                throw "That shortcut is your dictation shortcut. Choose another.";
              Object.assign(settings, args.settings);
              emit("settings-changed", settings);
              return settings;
            }
            if (command === "input_devices")
              return ["Built-in Microphone", "USB Microphone"];
            if (command === "microphone_level") return 0.12;
            if (command === "plugin:app|version") return "0.1.0";
            if (command === "update_status") return { kind: "idle" };
            if (command === "check_for_updates") {
              const update = { kind: "upToDate" };
              emit("update-state", update);
              return update;
            }
            if (command === "storage_usage")
              return { historyBytes: 18_432_000, annotationBytes: 3_145_728 };
            if (command === "list_history")
              return {
                entries: history
                  .filter((entry) =>
                    String((entry as { text?: string }).text ?? "")
                      .toLowerCase()
                      .includes(String(args.search ?? "").toLowerCase()),
                  )
                  .slice(0, Number((args as { limit?: number }).limit ?? 1e9)),
                hasMore:
                  history.length >
                  Number((args as { limit?: number }).limit ?? 1e9),
              };
            if (command === "list_annotations") return annotations;
            if (
              command === "delete_history" ||
              command === "delete_annotation"
            ) {
              const hooks = (
                window as unknown as {
                  __deleteHooks: { failId?: unknown; delayMs?: number };
                }
              ).__deleteHooks;
              if (hooks.delayMs)
                await new Promise((resolve) =>
                  setTimeout(resolve, hooks.delayMs),
                );
              if (hooks.failId === args.id) throw "disk said no";
              (window as unknown as { __deleted: unknown[] }).__deleted.push(
                args.id,
              );
              if (command === "delete_history")
                history = history.filter((entry) => entry.id !== args.id);
              else
                annotations = annotations.filter(
                  (entry) => entry.id !== args.id,
                );
            }
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
      history,
      annotations,
      delaySave,
      saveError,
    },
  );
  await page.goto(
    label === "main"
      ? "http://dictate.test/"
      : `http://dictate.test/?window=${label}`,
  );
}

for (const surface of ["General", "Annotate"]) {
  test(`${surface} shortcut records from the keyboard, reports errors inline and lets Tab leave`, async ({
    page,
  }) => {
    await page.emulateMedia({ reducedMotion: "reduce" });
    await openSettings(page);
    await page.getByRole("button", { name: surface, exact: true }).click();
    const recorder = page.getByRole("button", { name: /^Shortcut,/ });
    await recorder.focus();
    await page.keyboard.press("Enter");
    await expect(recorder).toHaveAccessibleName(/Press shortcut Esc to cancel/);
    await page.keyboard.press("a");
    await expect(recorder.locator("xpath=ancestor::li")).toContainText(
      "Include Command",
    );
    await page.keyboard.press("Escape");
    await expect(recorder).not.toHaveAccessibleName(/Press shortcut/);
    await expect(recorder).toBeFocused();
    await recorder.click();
    await page.keyboard.press("Tab");
    await expect(recorder).not.toBeFocused();
    await expect(recorder).not.toHaveAccessibleName(/Press shortcut/);
    await recorder.click();
    await page.keyboard.press("Control+Shift+K");
    await expect(recorder).toHaveAccessibleName(
      "Shortcut, Control + Shift + K",
    );
    await page.getByRole("button", { name: "Output", exact: true }).click();
    await page.getByRole("button", { name: surface, exact: true }).click();
    await expect(recorder).toHaveAccessibleName(
      "Shortcut, Control + Shift + K",
    );
  });
}

test("Annotate shortcut conflicts retain the binding and report beside its control", async ({
  page,
}) => {
  await openSettings(page);
  await page.getByRole("button", { name: "Annotate", exact: true }).click();
  const recorder = page.getByRole("button", { name: /^Shortcut,/ });
  await recorder.click();
  await page.keyboard.press("Alt+Shift+Space");
  await expect(recorder.locator("xpath=ancestor::li")).toContainText(
    "That shortcut is your dictation shortcut",
  );
  await expect(recorder).toHaveAccessibleName("Shortcut, Alt + Shift + A");
});

test("a chord entered before native capture is ready cannot save after cancellation", async ({
  page,
}) => {
  await openSettings(page, { delayCapture: true });
  const recorder = page.getByRole("button", { name: /^Shortcut,/ });
  await recorder.click();
  await page.keyboard.press("Control+Shift+K");
  await page.keyboard.press("Escape");
  await page.evaluate(() =>
    (window as unknown as { __finishCapture: () => void }).__finishCapture(),
  );
  await expect(recorder).toHaveAccessibleName("Shortcut, Alt + Shift + Space");
  expect(
    await page.evaluate(() =>
      (window as unknown as { __commands: string[] }).__commands.filter(
        (command) => command === "save_settings",
      ),
    ),
  ).toEqual([]);
});

for (const phase of ["recording", "annotating"]) {
  test(`shortcut editing explains why it is unavailable while ${phase}`, async ({
    page,
  }) => {
    await openSettings(page);
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
    const recorder = page.getByRole("button", { name: /^Shortcut,/ });
    await recorder.click();
    await expect(recorder.locator("xpath=ancestor::li")).toContainText(
      phase === "annotating"
        ? "Stop Annotate before recording a shortcut"
        : "Finish dictating before recording a shortcut",
    );
    await expect(recorder).toHaveAccessibleName(
      "Shortcut, Alt + Shift + Space",
    );
  });
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

test("the two appearances keep their own theme", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await openSettings(page);
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  const html = page.locator("html");
  const mode = (name: string) =>
    page
      .getByRole("radiogroup", { name: "Mode" })
      .getByRole("radio", { name, exact: true });
  const theme = (side: "Light" | "Dark", name: string) =>
    page
      .getByRole("radiogroup", { name: `${side} theme` })
      .getByRole("radio", { name, exact: true });
  // System on a light Mac edits the light theme, and the window follows at once.
  await theme("Light", "Paper").click();
  await expect(html).toHaveAttribute("data-palette", "paper");
  await expect(
    page.getByText("Your Mac is light now. Dark uses Ember."),
  ).toBeVisible();
  // Picking Dark switches what the grid edits; the light choice stays put.
  await mode("Dark").click();
  await expect(theme("Dark", "Ember")).toBeChecked();
  await theme("Dark", "Capy").click();
  await expect(html).toHaveAttribute("data-palette", "capy");
  await expect(
    page.getByText("Light uses Paper.", { exact: true }),
  ).toBeVisible();
  await mode("Light").click();
  await expect(theme("Light", "Paper")).toBeChecked();
  await expect(html).toHaveAttribute("data-palette", "paper");
  await expect(
    page.getByText("Dark uses Capy.", { exact: true }),
  ).toBeVisible();
  // Back on System, the Mac decides which of the two is shown and edited.
  await mode("System").click();
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(theme("Dark", "Capy")).toBeChecked();
  await expect(html).toHaveAttribute("data-palette", "capy");
  await page.getByRole("button", { name: "About", exact: true }).click();
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  await expect(theme("Dark", "Capy")).toBeChecked();
  await page.emulateMedia({ colorScheme: "light" });
  await expect(theme("Light", "Paper")).toBeChecked();
});

test("the picker is two radio groups that the keyboard walks and that never move", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light" });
  for (const width of [720, 640]) {
    await page.setViewportSize({ width, height: 580 });
    await openSettings(page, { pendingPage: "Appearance" });
    const groups = page.getByRole("radiogroup");
    await expect(groups).toHaveCount(2);
    const mode = page.getByRole("radiogroup", { name: "Mode" });
    for (const name of ["System", "Light", "Dark"])
      await expect(mode.getByRole("radio", { name, exact: true })).toHaveCount(
        1,
      );
    const themes = page.getByRole("radiogroup", { name: "Light theme" });
    await expect(themes.getByRole("radio")).toHaveCount(5);
    const layout = () =>
      page.evaluate(() => {
        const main = document.querySelector("[role=main]")!;
        const boxes = [
          ...main.querySelectorAll(".astryx-radio-list-item, svg, label"),
        ].map((element) => {
          const box = element.getBoundingClientRect();
          return [box.x, box.y, box.width, box.height]
            .map(Math.round)
            .join(",");
        });
        const clipped = [
          ...main.querySelectorAll(".astryx-radio-list-item"),
        ].filter((item) => item.scrollWidth > item.clientWidth + 1).length;
        return {
          boxes,
          clipped,
          overflow: document.documentElement.scrollWidth > innerWidth,
        };
      });
    const before = await layout();
    expect(before.clipped).toBe(0);
    expect(before.overflow).toBe(false);
    await themes.getByRole("radio", { name: "Ember", exact: true }).focus();
    await page.keyboard.press("ArrowRight");
    await expect(
      themes.getByRole("radio", { name: "Paper", exact: true }),
    ).toBeChecked();
    await expect(page.locator("html")).toHaveAttribute("data-palette", "paper");
    await mode.getByRole("radio", { name: "Dark", exact: true }).click();
    await expect(
      page.getByRole("radiogroup", { name: "Dark theme" }),
    ).toBeVisible();
    expect(await layout()).toEqual(before);
    await mode.getByRole("radio", { name: "System", exact: true }).click();
    expect(await layout()).toEqual(before);
  }
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
      .getByRole("radiogroup", { name: "Light theme" })
      .getByRole("radio", { name: theme.name, exact: true });
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
    await page.emulateMedia({ colorScheme: "light" });
    await page.getByRole("button", { name: "About", exact: true }).click();
    await page.getByRole("button", { name: "Appearance", exact: true }).click();
    // Coming back, the choice is still the one in the card.
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
    await expect(
      recorder,
      "the settings panel never rendered the Shortcut row",
    ).toBeVisible();
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
    // Navigating away unmounts General. Reading the recorder before coming back
    // only passed while the view transition still held the old page.
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

test("settings polish: sidebar order, Appearance page, resting buttons, two weights, update check", async ({
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
  await expect(page.getByRole("radiogroup", { name: "Mode" })).toHaveCount(0);
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
          "check_for_updates",
        ),
      ),
    )
    .toBe(true);
  await expect(
    page.getByRole("heading", { name: "About", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Check now", exact: true }),
  ).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as unknown as { __commands: string[] }).__commands.includes(
        "open_location",
      ),
    ),
  ).toBe(false);
  await page.getByRole("button", { name: "Appearance", exact: true }).click();
  await expect(
    page
      .getByRole("radiogroup", { name: "Light theme" })
      .getByRole("radio", { name: "Ember", exact: true }),
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

test("theme previews are pictures of each theme's own two sides", async ({
  page,
}) => {
  await openSettings(page, { pendingPage: "Appearance" });
  const themes = page.getByRole("radiogroup", { name: "Light theme" });
  await expect(themes.getByRole("radio")).toHaveCount(5);
  await expect(themes.locator(".astryx-status-dot, .astryx-badge")).toHaveCount(
    0,
  );
  await expect(themes.getByText("Aa")).toHaveCount(0);
  const sides = await themes
    .locator("svg")
    .evaluateAll((previews) =>
      previews.map((svg) => [
        svg.querySelector(":scope > rect")!.getAttribute("fill"),
        svg.querySelector(":scope > g > rect")!.getAttribute("fill"),
      ]),
    );
  expect(sides).toHaveLength(5);
  for (const [light, dark] of sides) expect(light).not.toBe(dark);
  expect(new Set(sides.map((pair) => pair.join())).size).toBe(5);
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

const minuteMs = 60_000;
const takes = Array.from({ length: 24 }, (_, index) => ({
  id: index + 1,
  text: `Take ${index + 1}: water the plants, then call the bank about the card.`,
  createdAtMs: Date.now() - (index + 1) * 37 * minuteMs,
  app: index % 2 ? "Slack" : "Notes",
  model: "ggml-base.en",
  audioMs: 2_400,
  inferenceMs: 410,
  totalMs: 650,
  status: "pasted",
  detail: null,
}));
const sessions = Array.from({ length: 14 }, (_, index) => ({
  id: `2026-11-${String(15 - (index % 10)).padStart(2, "0")}_10-${String(index).padStart(2, "0")}-00`,
  startedAt: new Date(Date.now() - (index + 1) * 53 * minuteMs).toISOString(),
  durationMs: 21_000,
  app: "Linear",
  frames: 2,
  marks: 2,
  text: `Session ${index + 1}: this toggle belongs on the right.`,
  folder: `/Users/you/Library/Application Support/com.kelechiodom.dictate/annotations/${index}`,
  hasContactSheet: true,
  packagingMs: 2_100,
  partial: [],
}));

async function openHistory(page: Page, width: number, view = "Dictations") {
  await page.setViewportSize({ width, height: 580 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openSettings(page, { history: takes, annotations: sessions });
  await page.getByRole("button", { name: "History", exact: true }).click();
  if (view === "Annotations")
    await page.getByRole("radio", { name: "Annotations", exact: true }).check();
  return page.getByRole("group", { name: view, exact: true });
}

async function expectNothingClipped(list: Locator) {
  await expect
    .poll(() =>
      list.evaluate((group) => group.getAnimations({ subtree: true }).length),
    )
    .toBe(0);
  const outside = await list.evaluate((group) => {
    const box = group.getBoundingClientRect();
    return [...group.querySelectorAll("*")]
      .filter((el) => el.getBoundingClientRect().width > 1)
      .filter((el) => {
        const { left, right } = el.getBoundingClientRect();
        return left < box.left - 1 || right > box.right + 1;
      })
      .map((el) => `${el.tagName} ${el.textContent?.slice(0, 40)}`);
  });
  expect(outside).toEqual([]);
  expect(
    await list.page().evaluate(() => {
      const root = document.documentElement;
      return root.scrollWidth - root.clientWidth;
    }),
  ).toBeLessThanOrEqual(0);
}

const sentCommands = (page: Page) =>
  page.evaluate(
    () => (window as unknown as { __commands: string[] }).__commands,
  );

for (const width of [640, 720]) {
  test(`at ${width}px a History row opens in place with copy and delete inline`, async ({
    page,
  }) => {
    const list = await openHistory(page, width);
    const row = list.getByRole("button", { name: /^Take 18:/ });
    await expect(row).toHaveAttribute("aria-expanded", "false");
    await expect(page.getByRole("region", { name: "Dictation" })).toHaveCount(
      0,
    );
    await row.click();
    await expect(row).toHaveAttribute("aria-expanded", "true");
    const detail = page.getByRole("region", { name: "Dictation" });
    await expect(detail).toContainText("Take 18: water the plants");
    const copy = detail.getByRole("button", { name: "Copy text" });
    const header = (await row.boundingBox())!;
    const copyBox = (await copy.boundingBox())!;
    expect(copyBox.y).toBeGreaterThan(header.y);
    expect(copyBox.y - (header.y + header.height)).toBeLessThan(80);
    expect(copyBox.y + copyBox.height).toBeLessThanOrEqual(580);
    await expectNothingClipped(list);
    await copy.click();
    await expect(detail.getByText("Copied to clipboard")).toBeAttached();
    expect(await sentCommands(page)).toContain(
      "plugin:clipboard-manager|write_text",
    );
    await detail.getByRole("button", { name: "Delete dictation" }).click();
    await page
      .getByRole("alertdialog", { name: "Delete this dictation?" })
      .getByRole("button", { name: "Delete dictation" })
      .click();
    await expect(list.getByRole("button", { name: /^Take 18:/ })).toHaveCount(
      0,
    );
    await expect(
      list.getByRole("button", { name: /^Take 19:/ }),
    ).toHaveAttribute("aria-expanded", "true");
  });
}

test("the narrow History list keeps one row open and announces it", async ({
  page,
}) => {
  const list = await openHistory(page, 640);
  const third = list.getByRole("button", { name: /^Take 3:/ });
  const sixth = list.getByRole("button", { name: /^Take 6:/ });
  await third.click();
  await expect(third).toHaveAttribute("aria-expanded", "true");
  const contentId = await third.getAttribute("aria-controls");
  await expect(
    page
      .locator(`[id="${contentId}"]`)
      .getByRole("region", { name: "Dictation" }),
  ).toContainText("Take 3:");
  expect(
    await third.evaluate(
      (trigger, id) =>
        trigger.compareDocumentPosition(document.getElementById(id!)!) &
        Node.DOCUMENT_POSITION_FOLLOWING,
      contentId,
    ),
  ).toBeTruthy();
  await sixth.evaluate((el) => el.scrollIntoView({ block: "center" }));
  const before = (await sixth.boundingBox())!.y;
  await sixth.click();
  await expect(sixth).toHaveAttribute("aria-expanded", "true");
  await expect(third).toHaveAttribute("aria-expanded", "false");
  await expect(list.locator('button[aria-expanded="true"]')).toHaveCount(1);
  await expect(page.getByRole("region", { name: "Dictation" })).toHaveCount(1);
  expect(Math.abs((await sixth.boundingBox())!.y - before)).toBeLessThanOrEqual(
    1,
  );
  await sixth.focus();
  await page.keyboard.press("Enter");
  await expect(sixth).toHaveAttribute("aria-expanded", "false");
  await expect(list.locator('button[aria-expanded="true"]')).toHaveCount(0);
  await page.keyboard.press("Space");
  await expect(sixth).toHaveAttribute("aria-expanded", "true");
  await page.keyboard.press("Tab");
  await expect(
    page
      .getByRole("region", { name: "Dictation" })
      .getByRole("button", { name: "Copy text" }),
  ).toBeFocused();
});

test("at 640px an Annotate row opens in place with its actions, one at a time", async ({
  page,
}) => {
  const list = await openHistory(page, 640, "Annotations");
  const second = list.getByRole("button", { name: /^Session 2:/ });
  const twelfth = list.getByRole("button", { name: /^Session 12:/ });
  await expect(page.getByRole("region", { name: "Annotation" })).toHaveCount(0);
  await second.click();
  await twelfth.click();
  await expect(second).toHaveAttribute("aria-expanded", "false");
  await expect(twelfth).toHaveAttribute("aria-expanded", "true");
  await expect(list.locator('button[aria-expanded="true"]')).toHaveCount(1);
  const detail = page.getByRole("region", { name: "Annotation" });
  await expect(detail).toContainText("Session 12:");
  await expectNothingClipped(list);
  for (const name of ["Copy again", "Reveal in Finder"]) {
    const action = detail.getByRole("button", { name });
    const box = (await action.boundingBox())!;
    expect(box.y + box.height).toBeLessThanOrEqual(580);
    await action.click();
  }
  await detail.getByRole("button", { name: "Delete annotation" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete this annotation?" })
    .getByRole("button", { name: "Delete annotation" })
    .click();
  await expect(list.getByRole("button", { name: /^Session 12:/ })).toHaveCount(
    0,
  );
  expect(await sentCommands(page)).toEqual(
    expect.arrayContaining([
      "copy_annotation",
      "reveal_annotation",
      "delete_annotation",
    ]),
  );
});

for (const view of ["Dictations", "Annotations"] as const)
  test(`a wide window keeps the two-pane ${view} layout`, async ({ page }) => {
    await openHistory(page, 1100, view);
    const title = view === "Dictations" ? "Dictation" : "Annotation";
    const first = view === "Dictations" ? /Take 1:/ : /Session 1:/;
    const fourth = view === "Dictations" ? /Take 4:/ : /Session 4:/;
    await expect(page.getByRole("group", { name: view })).toHaveCount(0);
    const list = page.getByRole("list", { name: view });
    await expect(list.locator("button[aria-expanded]")).toHaveCount(0);
    const detail = page.getByRole("region", { name: title });
    await expect(detail).toContainText(first);
    const listBox = (await list.boundingBox())!;
    const detailBox = (await detail.boundingBox())!;
    expect(detailBox.x).toBeGreaterThan(listBox.x + listBox.width);
    expect(Math.abs(detailBox.y - listBox.y)).toBeLessThanOrEqual(1);
    await list.getByRole("button", { name: fourth }).click();
    await expect(
      list
        .getByRole("button", { name: fourth })
        .locator("xpath=ancestor::*[@aria-current][1]"),
    ).toHaveAttribute("aria-current", "true");
    await expect(detail).toContainText(fourth);
    await expect(detail.getByRole("heading", { name: title })).toBeVisible();
  });

/** What a History row paints before its content: the leading edge of selection. */
const edgeMarker = (row: Locator) =>
  row.evaluate((element) => {
    const bar = getComputedStyle(element, "::before");
    return { width: bar.width, colour: bar.backgroundColor };
  });

/** The History list is a List in two panes and an accordion group when narrow. */
const historyList = (page: Page, view: string, width: number) =>
  width >= 820
    ? page.getByRole("list", { name: view, exact: true })
    : page.getByRole("group", { name: view, exact: true });

const rowBoxes = (rows: Locator) =>
  rows.evaluateAll((items) =>
    items.map((item) => {
      const box = item.getBoundingClientRect();
      return [box.x, box.y, box.width, box.height].map(Math.round).join();
    }),
  );

for (const initialMode of ["system", "light", "dark"] as const) {
  test(`Appearance ${initialMode} arrows and Tab survive live palette changes`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: "dark", reducedMotion: "reduce" });
    await openSettings(page, {
      pendingPage: "Appearance",
      stored: { appearanceMode: initialMode },
    });
    const modes = page.getByRole("radiogroup", { name: "Mode", exact: true });
    await expect(modes).toBeVisible();
    await page.keyboard.press("Tab");
    await modes
      .getByRole("radio", {
        name:
          initialMode === "system"
            ? "System"
            : initialMode === "light"
              ? "Light"
              : "Dark",
        exact: true,
      })
      .focus();
    for (let step = 0; step < 3; step++) {
      await page.keyboard.press("ArrowRight");
      await expect
        .poll(() =>
          modes.evaluate(
            (group) =>
              group.contains(document.activeElement) &&
              (document.activeElement as HTMLInputElement).checked,
          ),
        )
        .toBe(true);
    }
    await page.keyboard.press("Tab");
    const themes = page.getByRole("radiogroup", {
      name: /^(Light|Dark) theme$/,
    });
    await expect
      .poll(() =>
        themes.evaluate((group) => group.contains(document.activeElement)),
      )
      .toBe(true);
    for (const [name, palette] of [
      ["Paper", "paper"],
      ["Capy", "capy"],
      ["Rosé Pine", "rosePine"],
      ["Neutral", "neutral"],
      ["Ember", "ember"],
    ]) {
      await page.keyboard.press("ArrowRight");
      await expect(page.locator("html")).toHaveAttribute(
        "data-palette",
        palette,
      );
      const radio = themes.getByRole("radio", { name, exact: true });
      await expect(radio).toBeChecked();
      await expect(radio).toBeFocused();
    }
    await page.keyboard.press("Shift+Tab");
    await expect
      .poll(() =>
        modes.evaluate((group) => group.contains(document.activeElement)),
      )
      .toBe(true);
    for (const width of [640, 720, 1100, 1512]) {
      await page.setViewportSize({ width, height: 900 });
      const previews = themes.locator("svg");
      for (const rect of await previews.evaluateAll((els) =>
        els.map((el) => el.getBoundingClientRect().toJSON()),
      )) {
        expect(rect.width).toBeLessThanOrEqual(64);
        expect(rect.height).toBeLessThanOrEqual(39);
      }
      const tiles = await page.getByRole("radio").evaluateAll((radios) =>
        radios.map((radio) =>
          [...radio.closest('[role="radiogroup"]')!.children]
            .find((tile) => tile.contains(radio))!
            .getBoundingClientRect()
            .toJSON(),
        ),
      );
      for (const tile of tiles) {
        expect(tile.width).toBeGreaterThanOrEqual(44);
        expect(tile.height).toBeGreaterThanOrEqual(44);
      }
    }
    await expect(
      page.getByRole("switch", { name: "Translucent window" }),
    ).toHaveCount(0);
  });
}

test("Appearance retains focus while saving and when the runtime rejects a save", async ({
  page,
}) => {
  await openSettings(page, {
    pendingPage: "Appearance",
    delaySave: true,
    saveError: "Stop Annotate before changing settings",
  });
  const modes = page.getByRole("radiogroup", { name: "Mode", exact: true });
  const system = modes.getByRole("radio", { name: "System", exact: true });
  await expect(system).toBeVisible();
  await page.keyboard.press("Tab");
  await system.focus();
  await page.keyboard.press("ArrowRight");
  const light = modes.getByRole("radio", { name: "Light", exact: true });
  await expect(light).toBeChecked();
  await expect(light).toBeFocused();
  await expect(light).not.toHaveAttribute("aria-disabled", "true");
  await expect(page.getByRole("status")).toHaveText("Saving…");
  await page.evaluate(() =>
    (window as unknown as { __finishSave: () => void }).__finishSave(),
  );
  await expect(page.getByRole("alert")).toContainText(
    "Stop Annotate before changing settings",
  );
  await expect(system).toBeChecked();
  await expect(light).toBeFocused();
});

for (const phase of ["recording", "transcribing", "annotating", "packaging"]) {
  test(`Appearance explains ${phase} safety without losing keyboard navigation`, async ({
    page,
  }) => {
    await openSettings(page, { pendingPage: "Appearance" });
    const mode = page.getByRole("radiogroup", { name: "Mode", exact: true });
    const themes = page.getByRole("radiogroup", {
      name: /^(Light|Dark) theme$/,
    });
    const system = mode.getByRole("radio", { name: "System", exact: true });
    await expect(system).toBeVisible();
    await system.focus();
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
    const reason =
      phase === "annotating" || phase === "packaging"
        ? "Stop Annotate before changing appearance."
        : "Finish dictating before changing appearance.";
    await expect(
      page.getByRole("status").filter({ hasText: reason }),
    ).toBeVisible();
    await expect(system).toHaveAttribute("aria-disabled", "true");
    await expect(system).toBeFocused();
    await system.press("Tab");
    await expect
      .poll(() =>
        themes.evaluate((group) => group.contains(document.activeElement)),
      )
      .toBe(true);
    const before = await sentCommands(page);
    await page.keyboard.press("ArrowRight");
    await expect(
      themes.getByRole("radio", { name: "Ember", exact: true }),
    ).toBeChecked();
    expect(
      (await sentCommands(page)).filter((c) => c === "save_settings"),
    ).toEqual(before.filter((c) => c === "save_settings"));
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
    await expect(system).not.toHaveAttribute("aria-disabled", "true");
    await expect
      .poll(() =>
        themes.evaluate((group) => group.contains(document.activeElement)),
      )
      .toBe(true);
  });
}

test("General keyboard order and control actions survive the focus polish", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openSettings(page, { stored: { speechModel: "tiny.en" } });
  const shortcut = page.getByRole("button", { name: /^Shortcut,/ });
  const mode = page.getByRole("combobox", {
    name: "Shortcut mode",
    exact: true,
  });
  const model = page.getByRole("combobox", {
    name: "Speech model",
    exact: true,
  });
  const manage = page.getByRole("button", {
    name: "Manage models",
    exact: true,
  });
  const mic = page.getByRole("combobox", { name: "Microphone", exact: true });
  const testMic = page.getByRole("button", {
    name: "Test microphone",
    exact: true,
  });
  const startup = page.getByRole("switch", { name: "Launch at login" });
  await expect(shortcut).toBeVisible();
  await page.keyboard.press("Tab");
  await shortcut.focus();
  for (const control of [
    shortcut,
    mode,
    model,
    manage,
    mic,
    testMic,
    startup,
  ]) {
    await expect(control).toBeFocused();
    await control.press("Tab");
  }
  await shortcut.focus();
  await shortcut.press("Enter");
  await expect(shortcut).toHaveAccessibleName(/Press shortcut/);
  await shortcut.press("Escape");
  await expect(shortcut).not.toHaveAccessibleName(/Press shortcut/);
  for (const selector of [mode, model, mic]) {
    await selector.focus();
    await selector.press("Space");
    await expect(page.getByRole("listbox")).toBeVisible();
    await selector.press("ArrowDown");
    await selector.press("Escape");
    await expect(page.getByRole("listbox")).not.toBeVisible();
    await expect(selector).toBeFocused();
  }
  await testMic.focus();
  await testMic.press("Enter");
  const stop = page.getByRole("button", { name: "Stop test", exact: true });
  await expect(stop).toBeFocused();
  await stop.press("Enter");
  await expect(testMic).toBeFocused();
  await startup.focus();
  await startup.press("Space");
  await expect(startup).toBeChecked();
  await manage.focus();
  await manage.press("Enter");
  await expect(
    page.getByRole("heading", { name: "Models", level: 1 }),
  ).toBeVisible();
});

for (const theme of ["ember", "paper", "capy", "rosePine", "neutral"]) {
  for (const mode of ["light", "dark"] as const) {
    test(`${theme} ${mode} Settings keyboard traversal rings the control, not its passive row`, async ({
      page,
    }) => {
      await page.setViewportSize({ width: 1100, height: 900 });
      await page.emulateMedia({ colorScheme: mode, reducedMotion: "reduce" });
      await openSettings(page, {
        history: takes,
        annotations: sessions,
        stored: { themeLight: theme, themeDark: theme, appearanceMode: mode },
      });
      for (const name of [
        "General",
        "Appearance",
        "Models",
        "Annotate",
        "History",
        "Output",
        "Advanced",
        "About",
      ]) {
        await page
          .getByRole("navigation")
          .getByRole("button", { name, exact: true })
          .click();
        const main = page.getByRole("main");
        await expect(
          main.getByRole("heading", { name, level: 1, exact: true }),
        ).toBeVisible();
        const controls = main.locator(
          'button, input, select, textarea, a[href], [tabindex="0"]',
        );
        await expect.poll(() => controls.count()).toBeGreaterThan(0);
        const geometry = () =>
          main.locator(".ui-settings-row").evaluateAll((rows) =>
            rows.map((row) => {
              const el = row as HTMLElement;
              return [
                el.offsetLeft,
                el.offsetTop,
                el.offsetWidth,
                el.offsetHeight,
              ];
            }),
          );
        const before = await geometry();
        await page.keyboard.press("Tab");
        await main.evaluate((root) => {
          const first = [
            ...root.querySelectorAll<HTMLElement>(
              'button, input, select, textarea, a[href], [tabindex="0"]',
            ),
          ].find(
            (el) =>
              el.tabIndex >= 0 &&
              !el.hasAttribute("disabled") &&
              el.checkVisibility(),
          );
          first?.focus();
        });
        const visited = new Set<string>();
        for (let step = 0; step < 60; step++) {
          const focus = await main.evaluate((root) => {
            const active = document.activeElement as HTMLElement;
            if (!root.contains(active)) return null;
            const row = active.closest(".ui-settings-row");
            const rings = row
              ? [...row.querySelectorAll("*")].filter((el) => {
                  const css = getComputedStyle(el);
                  return (
                    el.checkVisibility({ checkOpacity: true }) &&
                    el.getBoundingClientRect().width > 1 &&
                    el.getBoundingClientRect().height > 1 &&
                    ((css.outlineStyle !== "none" &&
                      parseFloat(css.outlineWidth) > 0) ||
                      (el.contains(active) &&
                        css.boxShadow.includes("inset") &&
                        css.boxShadow.includes("2px")))
                  );
                })
              : [];
            const css = row && getComputedStyle(row);
            return {
              key: [...root.querySelectorAll("*")].indexOf(active).toString(),
              name: active.getAttribute("aria-label") ?? active.textContent,
              passive: !!row,
              rowRing: css
                ? css.outlineStyle !== "none" &&
                  parseFloat(css.outlineWidth) > 0
                : false,
              rings: rings.length,
            };
          });
          if (!focus) break;
          expect(visited.has(focus.key), name + " repeats a Tab stop").toBe(
            false,
          );
          visited.add(focus.key);
          if (focus.passive) {
            expect(focus.rowRing, name + " / " + focus.name).toBe(false);
            expect(focus.rings, name + " / " + focus.name).toBe(1);
          }
          await page.keyboard.press("Tab");
        }
        expect(visited.size, name).toBeGreaterThan(0);
        expect(await geometry(), name + " row geometry").toEqual(before);
      }
    });
  }
}

for (const theme of ["ember", "paper", "capy", "rosePine", "neutral"]) {
  for (const mode of ["light", "dark"] as const) {
    test(`${theme} ${mode} History markers are inset and separate from focus`, async ({
      page,
    }) => {
      await page.emulateMedia({ colorScheme: mode, reducedMotion: "reduce" });
      await openSettings(page, {
        history: takes,
        annotations: sessions,
        stored: { themeLight: theme, themeDark: theme, appearanceMode: mode },
      });
      await page.getByRole("button", { name: "History", exact: true }).click();
      for (const view of ["Dictations", "Annotations"]) {
        await page.getByRole("radio", { name: view, exact: true }).check();
        for (const width of [1100, 640]) {
          await page.setViewportSize({ width, height: 760 });
          const titles = view === "Dictations" ? /^Take / : /^Session /;
          const controls = page.getByRole("button", { name: titles });
          if (width === 640) await controls.first().click();
          const selected =
            width === 640
              ? page.locator(".ui-history-accordion-current > button")
              : page.locator(".ui-history-row-current");
          await expect(selected).toHaveCount(1);
          const idle =
            width === 640
              ? controls.nth(1)
              : controls.nth(1).locator("xpath=ancestor::li[1]");
          await page.mouse.move(0, 0);
          expect(
            await selected.evaluate(
              (row) => getComputedStyle(row).backgroundColor,
            ),
          ).toBe(
            await idle.evaluate((row) => getComputedStyle(row).backgroundColor),
          );
          const restingHover = await selected.evaluate(
            (row) => getComputedStyle(row).backgroundImage,
          );
          if (width === 1100) {
            await selected.hover();
            await expect
              .poll(() =>
                selected.evaluate(
                  (row) => getComputedStyle(row).backgroundImage,
                ),
              )
              .not.toBe(restingHover);
          }
          await page.mouse.move(0, 0);
          const shape = await selected.evaluate((row) => {
            const marker = getComputedStyle(row, "::before");
            const label = row.querySelector(".astryx-text")!;
            return {
              top: marker.top,
              bottom: marker.bottom,
              left: marker.left,
              width: marker.width,
              rounded: parseFloat(marker.borderRadius) > 0,
              textGap:
                label.getBoundingClientRect().left -
                row.getBoundingClientRect().left -
                parseFloat(marker.left) -
                parseFloat(marker.width),
            };
          });
          expect(shape).toMatchObject({
            top: "8px",
            bottom: "8px",
            left: "4px",
            width: "2px",
            rounded: true,
          });
          expect(shape.textGap).toBeGreaterThanOrEqual(8);
          const before = await controls.evaluateAll((rows) =>
            rows.map((row) => row.getBoundingClientRect().toJSON()),
          );
          await page.keyboard.press("Tab");
          await controls.nth(1).focus();
          await expect(controls.nth(1)).toBeFocused();
          expect(
            await selected.evaluate(
              (row) => getComputedStyle(row, "::before").width,
            ),
          ).toBe("2px");
          const focused =
            width === 640
              ? controls.nth(1)
              : controls.nth(1).locator("xpath=ancestor::li[1]");
          expect((await edgeMarker(focused)).width).not.toBe("2px");
          const ring = await focused.evaluate((row) => {
            const button = row.matches("button")
              ? row
              : row.querySelector("button")!;
            return [row, button].some((node) => {
              const css = getComputedStyle(node);
              return (
                css.outlineStyle !== "none" && parseFloat(css.outlineWidth) > 0
              );
            });
          });
          expect(ring).toBe(true);
          if (width === 1100) {
            expect(
              await focused.evaluate((row) => ({
                offset: getComputedStyle(row).outlineOffset,
                rounded: parseFloat(getComputedStyle(row).borderRadius) > 0,
              })),
            ).toEqual({ offset: "-4px", rounded: true });
          }
          expect(
            await controls.evaluateAll((rows) =>
              rows.map((row) => row.getBoundingClientRect().toJSON()),
            ),
          ).toEqual(before);
        }
      }
    });
  }
}

test("the entry the detail belongs to is marked in both History layouts", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light", reducedMotion: "reduce" });
  await page.setViewportSize({ width: 1100, height: 580 });
  await openSettings(page, { history: takes, annotations: sessions });
  await page.getByRole("button", { name: "History", exact: true }).click();
  const list = historyList(page, "Dictations", 1100);
  const rows = list.locator("li");
  // The library's own selected row is reported, and only one row carries it.
  await expect(rows.first()).toHaveAttribute("aria-current", "true");
  await expect(list.locator('[aria-current="true"]')).toHaveCount(1);
  const picked = await edgeMarker(rows.first());
  expect(picked.width).toBe("2px");
  expect(picked.colour).not.toBe("rgba(0, 0, 0, 0)");
  expect((await edgeMarker(rows.nth(2))).width).not.toBe("2px");
  // Choosing another entry moves the marker and moves nothing else.
  const before = await rowBoxes(rows);
  await rows.nth(2).getByRole("button").click();
  await expect(rows.nth(2)).toHaveAttribute("aria-current", "true");
  await expect(rows.first()).not.toHaveAttribute("aria-current", "true");
  expect(await rowBoxes(rows)).toEqual(before);
  // The narrow layout marks the row it has open, and keeps the same choice.
  await page.setViewportSize({ width: 640, height: 580 });
  const third = page.getByRole("button", { name: /^Take 3:/ });
  await expect(third).toHaveAttribute("aria-expanded", "true");
  expect((await edgeMarker(third)).width).toBe("2px");
  expect(
    (await edgeMarker(page.getByRole("button", { name: /^Take 5:/ }))).width,
  ).not.toBe("2px");
  await page.getByRole("button", { name: /^Take 5:/ }).click();
  await expect(third).toHaveAttribute("aria-expanded", "false");
  expect((await edgeMarker(third)).width).not.toBe("2px");
  expect(
    (await edgeMarker(page.getByRole("button", { name: /^Take 5:/ }))).width,
  ).toBe("2px");
});

for (const width of [640, 720, 820, 1100]) {
  for (const view of ["Dictations", "Annotations"] as const) {
    test(`at ${width}px ${view} says what it keeps on disk above the list`, async ({
      page,
    }) => {
      await openHistory(page, width, view);
      const list = historyList(page, view, width);
      const line = page.getByText(
        new RegExp(
          view === "Dictations"
            ? "^17\\.6 MB on this Mac"
            : "^3\\.0 MB on this Mac · 14 sessions · kept until deleted$",
        ),
      );
      await expect(line).toHaveCount(1);
      await expect(line).toBeVisible();
      // Above the list, and on screen before anything is scrolled.
      const [lineBox, listBox] = [
        (await line.boundingBox())!,
        (await list.boundingBox())!,
      ];
      expect(lineBox.y + lineBox.height).toBeLessThanOrEqual(listBox.y + 1);
      expect(lineBox.y + lineBox.height).toBeLessThanOrEqual(580);
      expect(
        await page.evaluate(() => document.scrollingElement!.scrollTop),
      ).toBe(0);
      // One quiet line of type: no card, no rule, no background of its own.
      const painted = await line.evaluate((element) => {
        const style = getComputedStyle(element);
        return {
          background: style.backgroundColor,
          border: style.borderBottomWidth,
          height: element.getBoundingClientRect().height,
        };
      });
      expect(painted.background).toBe("rgba(0, 0, 0, 0)");
      expect(painted.border).toBe("0px");
      expect(painted.height).toBeLessThanOrEqual(24);
      // The retention control lives on that line, not below the list.
      if (view === "Dictations")
        await expect(
          page.getByRole("combobox", { name: "Keep history for" }),
        ).toBeVisible();
    });
  }
}

for (const width of [640, 720, 820, 1100]) {
  for (const view of ["Dictations", "Annotations"] as const) {
    test(`at ${width}px ${view} storage stays readable with the navigation hint`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: 760 });
      await page.emulateMedia({ reducedMotion: "reduce" });
      await openSettings(page, { history: takes, annotations: sessions });
      await page.getByRole("button", { name: "History", exact: true }).click();
      await page.keyboard.press("Tab");
      const tab = page.getByRole("radio", { name: view, exact: true });
      await tab.focus();
      await tab.press("Space");
      const hint = page.getByText("to navigate", { exact: true });
      await expect(hint).toBeVisible();
      const line = page.getByText(
        view === "Dictations"
          ? /^17\.6 MB on this Mac/
          : /^3\.0 MB on this Mac · 14 sessions · kept until deleted$/,
      );
      await expect(line).toBeVisible();
      const readable = () =>
        line.evaluate((element) => {
          const range = document.createRange();
          range.selectNodeContents(element);
          const text = range.getBoundingClientRect();
          const box = element.getBoundingClientRect();
          const overlays = [...document.querySelectorAll("[popover]")]
            .filter((node) => node.matches(":popover-open"))
            .map((node) => node.getBoundingClientRect());
          return {
            complete:
              text.left >= box.left - 1 &&
              text.right <= box.right + 1 &&
              text.top >= box.top - 1 &&
              text.bottom <= box.bottom + 1 &&
              text.right <= innerWidth,
            covered: overlays.some(
              (overlay) =>
                overlay.left < text.right &&
                overlay.right > text.left &&
                overlay.top < text.bottom &&
                overlay.bottom > text.top,
            ),
          };
        });
      await expect.poll(readable).toEqual({ complete: true, covered: false });
      const before = await line.boundingBox();
      await tab.press("Tab");
      await expect(hint).not.toBeVisible();
      expect(await readable()).toEqual({ complete: true, covered: false });
      expect(await line.boundingBox()).toEqual(before);
    });
  }
}

const deletes = async (page: Page, command: string) =>
  (await sentCommands(page)).filter((sent) => sent === command).length;

const selectionBar = (page: Page, noun: string) =>
  page.getByRole("toolbar", { name: `Selected ${noun}` });

const deletedIds = (page: Page) =>
  page.evaluate(
    () => (window as unknown as { __deleted: unknown[] }).__deleted,
  );

const setDeleteHooks = (
  page: Page,
  hooks: { failId?: unknown; delayMs?: number },
) =>
  page.evaluate(
    (hooks) =>
      Object.assign(
        (window as unknown as { __deleteHooks: object }).__deleteHooks,
        hooks,
      ),
    hooks,
  );

test("wide Dictations bulk delete keeps the viewed row apart from checked rows", async ({
  page,
}) => {
  await openHistory(page, 1100);
  const list = page.getByRole("list", { name: "Dictations" });
  const detail = page.getByRole("region", { name: "Dictation" });
  await expect(detail).toContainText("Take 1:");
  await page.getByRole("button", { name: "Select", exact: true }).click();
  const bar = selectionBar(page, "dictations");
  await expect(bar.getByText("0 selected", { exact: true })).toBeVisible();
  await expect(bar.getByRole("button", { name: "Delete" })).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Select", exact: true }),
  ).toHaveCount(0);
  await list.getByRole("checkbox", { name: /^Select Take 3:/ }).check();
  await list.getByRole("checkbox", { name: /^Select Take 5:/ }).check();
  await expect(bar.getByText("2 selected", { exact: true })).toBeVisible();
  await expect(
    bar.getByRole("checkbox", { name: "Select all 24 shown" }),
  ).toHaveJSProperty("indeterminate", true);
  // The viewed row's marker steps aside while checkboxes are showing.
  await expect(list.locator(".ui-history-row-current")).toHaveCount(0);
  await expect(detail).toContainText("Take 1:");
  await list.getByRole("button", { name: /^Take 4:/ }).click();
  await expect(detail).toContainText("Take 4:");
  await expect(
    list.getByRole("checkbox", { name: /^Select Take 4:/ }),
  ).not.toBeChecked();
  await bar.getByRole("button", { name: "Delete" }).click();
  const dialog = page.getByRole("alertdialog", {
    name: "Delete 2 dictations?",
  });
  await expect(dialog).toContainText("They can’t be recovered.");
  await dialog.getByRole("button", { name: "Delete 2 dictations" }).click();
  await expect(list.getByRole("button", { name: /^Take 3:/ })).toHaveCount(0);
  await expect(list.getByRole("button", { name: /^Take 5:/ })).toHaveCount(0);
  await expect(list.getByRole("button")).toHaveCount(22);
  expect(await deletes(page, "delete_history")).toBe(2);
  expect(await deletedIds(page)).toEqual([3, 5]);
  await expect(bar).toHaveCount(0);
  await expect(detail).toContainText("Take 4:");
  await expect(list.locator(".ui-history-row-current")).toHaveCount(1);
});

test("Dictations select-all names its scope and Escape steps back", async ({
  page,
}) => {
  await openHistory(page, 1100);
  await page.getByRole("searchbox", { name: "Search history" }).fill("Take 1");
  await page.getByRole("button", { name: "Select", exact: true }).click();
  const bar = selectionBar(page, "dictations");
  await bar
    .getByRole("checkbox", { name: "Select all 11 matching “Take 1”" })
    .check();
  await expect(bar.getByText("11 selected", { exact: true })).toBeVisible();
  await bar.getByRole("button", { name: "Delete" }).click();
  await expect(
    page.getByRole("alertdialog", { name: "Delete 11 dictations?" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("alertdialog")).toHaveCount(0);
  await expect(bar.getByText("11 selected", { exact: true })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(bar).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Select", exact: true }),
  ).toBeFocused();
  await expect(page.getByRole("checkbox")).toHaveCount(0);
  expect(await deletes(page, "delete_history")).toBe(0);
});

test("narrow Dictations select by keyboard and return to the accordion", async ({
  page,
}) => {
  await openHistory(page, 640);
  await page.getByRole("button", { name: "Select", exact: true }).click();
  const list = page.getByRole("list", { name: "Dictations" });
  await expect(list.locator("button[aria-expanded]")).toHaveCount(0);
  const second = list.getByRole("checkbox", { name: /^Select Take 2:/ });
  await second.focus();
  await page.keyboard.press("Space");
  await expect(second).toBeChecked();
  await list.getByText(/^Take 6:/).click();
  await expect(
    list.getByRole("checkbox", { name: /^Select Take 6:/ }),
  ).toBeChecked();
  const bar = selectionBar(page, "dictations");
  await expect(bar.getByText("2 selected", { exact: true })).toBeVisible();
  await bar.getByRole("button", { name: "Stop selecting" }).click();
  const group = page.getByRole("group", { name: "Dictations", exact: true });
  await expect(group.locator("button[aria-expanded]")).toHaveCount(24);
  await expect(page.getByRole("checkbox")).toHaveCount(0);
});

for (const width of [640, 1100])
  test(`Annotations bulk delete at ${width}px removes each session the single-delete way`, async ({
    page,
  }) => {
    await openHistory(page, width, "Annotations");
    await page.getByRole("button", { name: "Select", exact: true }).click();
    await page.getByRole("checkbox", { name: /^Select Session 2:/ }).check();
    await page.getByRole("checkbox", { name: /^Select Session 9:/ }).check();
    await page.getByRole("checkbox", { name: /^Select Session 11:/ }).check();
    await selectionBar(page, "annotations")
      .getByRole("button", { name: "Delete" })
      .click();
    const dialog = page.getByRole("alertdialog", {
      name: "Delete 3 annotations?",
    });
    await expect(dialog).toContainText(
      "Their recordings, frames and summaries are removed.",
    );
    await dialog.getByRole("button", { name: "Delete 3 annotations" }).click();
    await expect(page.getByText(/^Session 9:/)).toHaveCount(0);
    expect(await deletes(page, "delete_annotation")).toBe(3);
    expect(await deletedIds(page)).toEqual(
      [2, 9, 11].map((n) => sessions[n - 1].id),
    );
    await expect(page.getByText(/· 11 sessions ·/)).toBeVisible();
  });

for (const view of ["Dictations", "Annotations"] as const)
  for (const width of [640, 1100])
    test(`${view} at ${width}px pin the selection pill to the bottom of the pane`, async ({
      page,
    }) => {
      await openHistory(page, width, view);
      const noun = view === "Dictations" ? "dictations" : "annotations";
      const list = page.getByRole("list", { name: view });
      const rowTops = () =>
        list
          .getByRole("listitem")
          .evaluateAll((els) =>
            els.map((el) => Math.round(el.getBoundingClientRect().top)),
          );
      await page.getByRole("button", { name: "Select", exact: true }).click();
      const before = await rowTops();
      const bar = selectionBar(page, noun);
      const pill = page.locator(".ui-selection-pill");
      await list.getByRole("checkbox").nth(1).check();
      await expect(bar.getByText("1 selected", { exact: true })).toBeVisible();
      // Checking moves nothing above the bar.
      expect(await rowTops()).toEqual(before);
      // Pinned 16px above the bottom of the window and centred on the pane,
      // wherever the list is scrolled.
      const main = await page
        .locator("main, [role=main]")
        .first()
        .evaluate((el) => ({
          x: el.getBoundingClientRect().left + el.clientLeft,
          width: el.clientWidth,
        }));
      for (const end of [false, true]) {
        if (end)
          await list.evaluate((el) => {
            for (let node = el.parentElement; node; node = node.parentElement)
              if (/auto|scroll/.test(getComputedStyle(node).overflowY))
                node.scrollTop = node.scrollHeight;
            document.scrollingElement!.scrollTop = 1e9;
          });
        const box = (await pill.boundingBox())!;
        expect(Math.abs(580 - 16 - (box.y + box.height))).toBeLessThanOrEqual(
          2,
        );
        expect(
          Math.abs(box.x + box.width / 2 - (main.x + main.width / 2)),
        ).toBeLessThanOrEqual(2);
      }
      // At the end of the list the last row clears the bar.
      const last = (await list.getByRole("listitem").last().boundingBox())!;
      expect(last.y + last.height).toBeLessThanOrEqual(
        (await pill.boundingBox())!.y,
      );
      // One pill: fully rounded, bordered and raised.
      const look = await pill.evaluate((el) => {
        const style = getComputedStyle(el);
        return {
          radius: parseFloat(style.borderTopLeftRadius),
          height: el.getBoundingClientRect().height,
          border: style.borderTopWidth,
          shadow: style.boxShadow,
        };
      });
      expect(look.radius).toBeGreaterThanOrEqual(look.height / 2);
      expect(look.border).toBe("1px");
      expect(look.shadow).not.toBe("none");
      // Keyboard: Tab reaches the bar, arrows walk it, X stops selecting
      // and hands focus back to Select.
      const all = bar.getByRole("checkbox", { name: /^Select all \d+ shown$/ });
      for (let step = 0; step < 80; step++) {
        if (await all.evaluate((el) => el === document.activeElement)) break;
        await page.keyboard.press("Tab");
      }
      await expect(all).toBeFocused();
      await page.keyboard.press("ArrowRight");
      await expect(bar.getByRole("button", { name: "Delete" })).toBeFocused();
      await page.keyboard.press("ArrowRight");
      await expect(
        bar.getByRole("button", { name: "Stop selecting" }),
      ).toBeFocused();
      await page.keyboard.press("Enter");
      await expect(bar).toHaveCount(0);
      await expect(
        page.getByRole("button", { name: "Select", exact: true }),
      ).toBeFocused();
    });

for (const width of [640, 1100])
  test(`Annotations at ${width}px search narrows the list and select-all follows it`, async ({
    page,
  }) => {
    await openHistory(page, width, "Annotations");
    const select = page.getByRole("button", { name: "Select", exact: true });
    await expect(select).toBeVisible();
    await page
      .getByRole("searchbox", { name: "Search annotations" })
      .fill("Session 1");
    await select.click();
    await expect(
      page.getByRole("checkbox", { name: /^Select Session \d+:/ }),
    ).toHaveCount(6);
    const bar = selectionBar(page, "annotations");
    await bar
      .getByRole("checkbox", { name: "Select all 6 matching “Session 1”" })
      .check();
    await expect(bar.getByText("6 selected", { exact: true })).toBeVisible();
    await bar.getByRole("button", { name: "Delete" }).click();
    await page
      .getByRole("alertdialog")
      .getByRole("button", { name: "Delete 6 annotations" })
      .click();
    await expect(bar).toHaveCount(0);
    expect(await deletedIds(page)).toEqual(
      [1, 10, 11, 12, 13, 14].map((n) => sessions[n - 1].id),
    );
  });

test("both History tabs show the Select button with real data", async ({
  page,
}) => {
  await openHistory(page, 1100);
  await expect(
    page.getByRole("button", { name: "Select", exact: true }),
  ).toBeVisible();
  await page.getByRole("radio", { name: "Annotations", exact: true }).check();
  await expect(
    page.getByRole("searchbox", { name: "Search annotations" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Select", exact: true }),
  ).toBeVisible();
});

test("a checked row hidden by search stays checked but is not counted or deleted", async ({
  page,
}) => {
  await openHistory(page, 1100);
  await page.getByRole("button", { name: "Select", exact: true }).click();
  const list = page.getByRole("list", { name: "Dictations" });
  await list.getByRole("checkbox", { name: /^Select Take 3:/ }).check();
  const bar = selectionBar(page, "dictations");
  const search = page.getByRole("searchbox", { name: "Search history" });
  await search.fill("Take 1");
  await expect(bar.getByText("0 selected · 1 hidden")).toBeVisible();
  await expect(bar.getByRole("button", { name: "Delete" })).toBeDisabled();
  await list.getByRole("checkbox", { name: /^Select Take 12:/ }).check();
  await search.fill("");
  await expect(bar.getByText("2 selected", { exact: true })).toBeVisible();
  await expect(
    list.getByRole("checkbox", { name: /^Select Take 3:/ }),
  ).toBeChecked();
  await search.fill("Take 1");
  await bar.getByRole("button", { name: "Delete" }).click();
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "Delete 1 dictation" })
    .click();
  await expect(bar).toHaveCount(0);
  expect(await deletedIds(page)).toEqual([12]);
});

test("a failed bulk delete says how far it got and keeps the survivors checked", async ({
  page,
}) => {
  await openHistory(page, 1100, "Annotations");
  await setDeleteHooks(page, { failId: sessions[8].id });
  await page.getByRole("button", { name: "Select", exact: true }).click();
  for (const n of [2, 9, 11])
    await page
      .getByRole("checkbox", { name: new RegExp(`^Select Session ${n}:`) })
      .check();
  const bar = selectionBar(page, "annotations");
  await bar.getByRole("button", { name: "Delete" }).click();
  await page
    .getByRole("alertdialog")
    .getByRole("button", { name: "Delete 3 annotations" })
    .click();
  await expect(
    page.getByText("Deleted 1 of 3 annotations. disk said no"),
  ).toBeVisible();
  await expect(bar.getByText("2 selected", { exact: true })).toBeVisible();
  expect(await deletedIds(page)).toEqual([sessions[1].id]);
});

test("while a bulk delete runs, the confirm is disabled and selecting cannot end", async ({
  page,
}) => {
  await openHistory(page, 1100);
  await setDeleteHooks(page, { delayMs: 600 });
  await page.getByRole("button", { name: "Select", exact: true }).click();
  const list = page.getByRole("list", { name: "Dictations" });
  await list.getByRole("checkbox", { name: /^Select Take 3:/ }).check();
  await list.getByRole("checkbox", { name: /^Select Take 5:/ }).check();
  const bar = selectionBar(page, "dictations");
  await bar.getByRole("button", { name: "Delete" }).click();
  const action = page
    .getByRole("alertdialog")
    .getByRole("button", { name: "Delete 2 dictations" });
  await action.click();
  await expect(action).toHaveAttribute("aria-disabled", "true");
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(bar).toBeVisible();
  await expect(
    bar.getByRole("button", { name: "Stop selecting" }),
  ).toBeDisabled();
  await expect(bar).toHaveCount(0);
  expect(await deletedIds(page)).toEqual([3, 5]);
});

test("a take arriving while all are checked stays unchecked", async ({
  page,
}) => {
  await openHistory(page, 1100);
  await page.getByRole("button", { name: "Select", exact: true }).click();
  const bar = selectionBar(page, "dictations");
  await bar.getByRole("checkbox", { name: "Select all 24 shown" }).check();
  await page.evaluate(() =>
    (window as unknown as { __addTake: (take: object) => void }).__addTake({
      id: 99,
      text: "Take 99: arrived while selecting.",
      createdAtMs: Date.now(),
      app: "Notes",
      model: "ggml-base.en",
      audioMs: 2_400,
      inferenceMs: 410,
      totalMs: 650,
      status: "pasted",
      detail: null,
    }),
  );
  const all = bar.getByRole("checkbox", { name: "Select all 25 shown" });
  await expect(all).toHaveJSProperty("indeterminate", true);
  await expect(bar.getByText("24 selected", { exact: true })).toBeVisible();
  await expect(
    page.getByRole("checkbox", { name: /^Select Take 99:/ }),
  ).not.toBeChecked();
});

test("a take finishing while 500 are selected keeps every checked row in the list", async ({
  page,
}) => {
  const many = Array.from({ length: 500 }, (_, index) => ({
    ...takes[0],
    id: index + 1,
    text: `Take ${index + 1}`,
    createdAtMs: Date.now() - (index + 1) * minuteMs,
  }));
  await page.setViewportSize({ width: 1100, height: 700 });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await openSettings(page, { history: many });
  await page.getByRole("button", { name: "History", exact: true }).click();
  await page.getByRole("button", { name: "Select", exact: true }).click();
  await page.getByRole("checkbox", { name: "Select all 500 shown" }).check();
  const status = page.locator(".ui-selection-count");
  await expect(status).toHaveText("500 selected");
  await page.evaluate(() =>
    (window as unknown as { __addTake: (take: unknown) => void }).__addTake({
      id: 9999,
      text: "New take",
      createdAtMs: Date.now(),
      app: "Slack",
      model: "ggml-base.en",
      audioMs: 1000,
      inferenceMs: 10,
      totalMs: 20,
      status: "pasted",
      detail: null,
    }),
  );
  await expect(
    page.getByRole("checkbox", { name: /^Select New take/ }),
  ).not.toBeChecked();
  await expect(status).toHaveText("500 selected");
  await page.getByRole("button", { name: "Delete", exact: true }).click();
  await expect(page.getByRole("alertdialog")).toContainText(
    "Delete 500 dictations?",
  );
});

const longTitle =
  "Send the deploy link https://dictate.kellycooks.dev/releases/download/v0.1.3/dictate_0.1.3_aarch64.dmg?utm_source=history&utm_medium=desktop to Aleksandra";
const longApp =
  "Microsoft Visual Studio Code – Insiders (Rosetta) – Untitled-1";

for (const width of [640, 720, 1100]) {
  for (const view of ["Dictations", "Annotations"]) {
    test(`at ${width}px a ${view} row with an unbroken title keeps its date and chevron in place`, async ({
      page,
    }) => {
      await page.setViewportSize({ width, height: 600 });
      await page.emulateMedia({ reducedMotion: "reduce" });
      await openSettings(page, {
        history: [
          { ...takes[0], text: longTitle, app: longApp },
          ...takes.slice(1, 4),
        ],
        annotations: [
          { ...sessions[0], text: longTitle, app: longApp },
          ...sessions.slice(1, 4),
        ],
      });
      await page.getByRole("button", { name: "History", exact: true }).click();
      if (view === "Annotations")
        await page
          .getByRole("radio", { name: "Annotations", exact: true })
          .check();
      const pane = page.locator("main, [role=main]").first();
      const paneRight = await pane.evaluate(
        (el) => el.getBoundingClientRect().right,
      );
      const date = page.getByText(/ago$/).first();
      await expect(date).toBeVisible();
      const dateBox = (await date.boundingBox())!;
      expect(dateBox.x + dateBox.width).toBeLessThanOrEqual(paneRight);
      for (const chevron of await page
        .locator(
          ".astryx-collapsible-trigger > svg, .astryx-collapsible-trigger [aria-hidden=true]",
        )
        .all()) {
        const box = await chevron.boundingBox();
        if (box) expect(box.x + box.width).toBeLessThanOrEqual(paneRight);
      }
      const title = page.getByText(longTitle.slice(0, 20)).first();
      const clamped = await title.evaluate(
        (el) =>
          el.scrollHeight > el.clientHeight + 1 ||
          el.scrollWidth > el.clientWidth + 1,
      );
      const titleBox = (await title.boundingBox())!;
      expect(titleBox.x + titleBox.width).toBeLessThanOrEqual(dateBox.x);
      expect(clamped).toBe(true);
      expect(
        await page.evaluate(
          () =>
            document.documentElement.scrollWidth -
            document.documentElement.clientWidth,
        ),
      ).toBeLessThanOrEqual(0);
    });
  }
}
