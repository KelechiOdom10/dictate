import "./App.css";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { useState } from "react";
import { expect, fireEvent, userEvent, waitFor, within } from "storybook/test";
import type { AnnotationEntry } from "./annotations";
import type { HistoryEntry, TakeStatus } from "./history";
import modelFixture from "./models.fixture.json";
import {
  modelEntrySchema,
  type ModelEntry,
  type ModelState,
  type SpeechModel,
} from "./models";
import problems from "./problems.fixture.json";
import { settingsSchema, type Settings } from "./settings";
import SettingsWindow from "./SettingsWindow";
import { RecordingCapsule } from "./ui";

const meta = {
  title: "Pages",
  parameters: { layout: "fullscreen" },
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
async function choose(
  canvasElement: HTMLElement,
  label: string,
  option: string | RegExp,
) {
  const select = within(canvasElement).getByRole("combobox", { name: label });
  await waitFor(() => expect(select).toBeEnabled());
  await userEvent.click(select);
  fireEvent.click(
    await within(document.body).findByRole("option", { name: option }),
  );
  await waitFor(() =>
    expect(
      within(canvasElement).getByRole("combobox", { name: label }),
    ).toHaveTextContent(option),
  );
}
const minute = 60_000;
const sampleHistory = (now: number): HistoryEntry[] =>
  [
    {
      text: "Stop by the bakery before six and pick up the rye loaf for Sunday.",
      ago: 12 * minute,
      app: "Notes",
      inferenceMs: 412,
      totalMs: 655,
    },
    {
      text: "Ship the history page once the screenshots look right in all four themes.\nThen hand it to review.",
      ago: 3 * 60 * minute,
      app: "Slack",
      inferenceMs: 538,
      totalMs: 802,
    },
    {
      text: "",
      ago: 26 * 60 * minute,
      app: "Notes",
      inferenceMs: null,
      totalMs: 4,
      status: "skipped" as TakeStatus,
      detail: "0.18 s, below minimum 0.30 s",
    },
    {
      text: "Remember that the overlay should never take focus from the app you are typing in.",
      ago: 2 * 24 * 60 * minute,
      app: "Visual Studio Code",
      inferenceMs: 691,
      totalMs: 1_010,
      status: "failed" as TakeStatus,
      detail:
        "Allow dictate in System Settings → Privacy & Security → Accessibility, then retry",
    },
    {
      text: "Let’s move the Thursday sync to Friday morning, same time.",
      ago: 5 * 24 * 60 * minute,
      app: "Mail",
      inferenceMs: 377,
      totalMs: 590,
    },
  ].map(
    (
      { ago, status = "pasted" as TakeStatus, detail = null, ...entry },
      index,
    ): HistoryEntry => ({
      id: index + 1,
      createdAtMs: now - ago,
      model: "ggml-base.en",
      audioMs: entry.text.length * 60,
      status,
      detail,
      ...entry,
    }),
  );

type ModelsMock = {
  active?: SpeechModel;
  states?: Partial<Record<SpeechModel, ModelState>>;
  whisperMissing?: boolean;
};
const annotationsFolder =
  "/Users/you/Library/Application Support/com.kelechiodom.dictate/annotations";
const sampleAnnotations = (now: number): AnnotationEntry[] => [
  {
    id: "2026-11-15_10-42-08",
    startedAt: new Date(now - 6 * minute).toISOString(),
    durationMs: 49_430,
    app: "System Settings",
    frames: 3,
    marks: 3,
    text: "This toggle should be on the right. And this list is too dense. The error at the bottom is cut off.",
    folder: `${annotationsFolder}/2026-11-15_10-42-08`,
    hasContactSheet: true,
    packagingMs: 3_180,
    partial: [],
  },
  {
    id: "2026-11-14_16-05-51",
    startedAt: new Date(now - 19 * 60 * minute).toISOString(),
    durationMs: 12_100,
    app: "Linear",
    frames: 0,
    marks: 1,
    text: "",
    folder: `${annotationsFolder}/2026-11-14_16-05-51`,
    hasContactSheet: false,
    packagingMs: 2_040,
    partial: [
      "Video cut short: the screen recording did not finish, so there are no frames. What it caught is in recording-partial.mp4.",
    ],
  },
];
/** Annotate commands the pages sent, newest last, as `command id`. */
const annotateCalls: string[] = [];
type AnnotateMock = { entries?: AnnotationEntry[]; permission?: boolean };

const MB = 1_000_000;
/** Locations the About page asked the app to open, newest last. */
const opened: string[] = [];
/** What "Check for updates" resolves to in the About stories. */
let updateResult: Record<string, string> = { kind: "upToDate" };

async function mockSettings({ globals }: { globals: Record<string, unknown> }) {
  return mockApp(globals, [], {
    states: {
      "base.en": { kind: "installed" },
      "small.en": { kind: "installed" },
    },
  });
}
function mockApp(
  globals: Record<string, unknown>,
  initialHistory: HistoryEntry[],
  models: ModelsMock = { states: { "base.en": { kind: "installed" } } },
  overrides: Partial<Settings> = {},
  annotate: AnnotateMock = {},
) {
  annotateCalls.length = 0;
  let history = initialHistory;
  const states = { ...models.states };
  const catalog = modelFixture.map((entry) => modelEntrySchema.parse(entry));
  const listModels = (): ModelEntry[] =>
    catalog.map((entry) => ({
      ...entry,
      active: entry.id === settings.speechModel,
      state: states[entry.id] ?? { kind: "available" },
    }));
  const modelArg = (args: unknown) =>
    catalog.find(
      (entry) =>
        typeof args === "object" &&
        args !== null &&
        "model" in args &&
        entry.id === args.model,
    )!;
  let settings: Settings = {
    version: 1,
    shortcut: "Alt+Shift+Space",
    shortcutMode: "hold",
    microphone: null,
    launchAtLogin: false,
    themeLight: settingsSchema.shape.themeLight.parse(globals.theme),
    themeDark: settingsSchema.shape.themeLight.parse(globals.theme),
    overlayPosition: "bottom",
    historyRetention: "week",
    appearanceMode: globals.mode === "dark" ? "dark" : "light",
    speechModel: models.active ?? "base.en",
    pasteMethod: "paste",
    restoreClipboard: true,
    trailingSpace: false,
    capitalizeFirst: false,
    micMode: "onDemand",
    soundFeedback: false,
    debugRecordings: false,
    annotateShortcut: "Alt+Shift+KeyS",
    annotateEnabled: true,
    annotateShowInRecordings: false,
    glass: false,
    ...overrides,
  };
  mockWindows("settings");
  opened.length = 0;
  mockIPC(
    (command, args) => {
      if (command === "pipeline_status") return { phase: "idle", error: null };
      if (command === "get_settings") return settings;
      if (command === "save_settings") {
        settings = settingsSchema.parse(
          args && "settings" in args ? args.settings : undefined,
        );
        return settings;
      }
      if (command === "input_devices")
        return ["Built-in Microphone", "USB Microphone"];
      if (command === "microphone_level") return 0.12;
      if (command === "plugin:app|version") return "0.1.0";
      if (command === "app_folders")
        return {
          data: "/Users/you/Library/Application Support/com.kelechiodom.dictate",
          logs: "/Users/you/Library/Logs/com.kelechiodom.dictate",
        };
      if (command === "update_status") return { kind: "idle" };
      if (command === "check_for_updates") return updateResult;
      if (command === "open_location") {
        opened.push(String(args && "location" in args ? args.location : ""));
        return null;
      }
      if (command === "list_history") {
        const search = String(
          args && "search" in args ? args.search : "",
        ).toLowerCase();
        const limit = Number(args && "limit" in args ? args.limit : 500);
        const matches = history.filter((entry) =>
          `${entry.text} ${entry.app ?? ""}`.toLowerCase().includes(search),
        );
        return {
          entries: matches.slice(0, limit),
          hasMore: matches.length > limit,
        };
      }
      if (command === "list_models") return listModels();
      if (command === "list_annotations") return annotate.entries ?? [];
      if (command === "development_build") return false;
      if (command === "screen_permission") return annotate.permission ?? true;
      if (command === "delete_annotation") {
        const { id } = args as { id: string };
        annotateCalls.push(`delete_annotation ${id}`);
        annotate.entries = (annotate.entries ?? []).filter(
          (entry) => entry.id !== id,
        );
        return null;
      }
      if (
        command === "copy_annotation" ||
        command === "reveal_annotation" ||
        command === "open_screen_recording_settings"
      ) {
        annotateCalls.push(
          `${command} ${args && "id" in args ? String(args.id) : ""}`.trim(),
        );
        return null;
      }
      if (command === "whisper_status")
        return models.whisperMissing ? problems.whisperMissing : null;
      if (command === "download_model") {
        const model = modelArg(args);
        states[model.id] = {
          kind: "downloading",
          received: 0,
          total: model.bytes,
          cancelling: false,
        };
      }
      if (command === "cancel_download") delete states[modelArg(args).id];
      if (command === "delete_model") delete states[modelArg(args).id];
      if (command === "delete_history") {
        const id = args && "id" in args ? args.id : undefined;
        history = history.filter((entry) => entry.id !== id);
      }
      return null;
    },
    { shouldMockEvents: true },
  );
  return () => clearMocks();
}
function GeneralPage({
  page = "General",
  width,
}: {
  page?: string;
  width?: number;
}) {
  const [client] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: { retry: false },
          mutations: { retry: false },
        },
      }),
  );
  return (
    <div style={{ height: "100vh", width }}>
      <QueryClientProvider client={client}>
        <SettingsWindow initialPage={page} />
      </QueryClientProvider>
    </div>
  );
}
export const General: Story = {
  beforeEach: mockSettings,
  render: () => <GeneralPage />,
  play: async ({ canvasElement }) => {
    const control = await within(canvasElement).findByRole("switch", {
      name: "Launch at login",
    });
    await expect(control.closest(".astryx-switch-field")).toBeVisible();
    await expect(control).toBeEnabled();
    await expect(control).not.toHaveAttribute("aria-disabled", "true");
    await expect(control).not.toBeChecked();
  },
};

export const GeneralNoModel: Story = {
  beforeEach: ({ globals }) => mockApp(globals, [], { states: {} }),
  render: () => <GeneralPage />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(
      await c.findByText(
        "No speech model yet. Download one to start dictating.",
      ),
    ).toBeVisible();
    await expect(
      c.getByRole("button", { name: "Download a model" }),
    ).toBeVisible();
  },
};
export const GeneralActiveModelMissing: Story = {
  beforeEach: ({ globals }) =>
    mockApp(globals, [], { states: { "small.en": { kind: "installed" } } }),
  render: () => <GeneralPage />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(
      await c.findByText("Base English isn’t installed. Choose another model."),
    ).toBeVisible();
    await expect(
      c.queryByRole("button", { name: "Download a model" }),
    ).not.toBeInTheDocument();
    await choose(canvasElement, "Speech model", /^Small English/);
    await expect(c.queryByText(/isn’t installed/)).not.toBeInTheDocument();
  },
};

export const GeneralFreshProfile: Story = {
  beforeEach: ({ globals }) =>
    mockApp(globals, [], {
      active: "parakeet-unified-en",
      states: { "parakeet-unified-en": { kind: "installed" } },
    }),
  render: () => <GeneralPage />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const select = await c.findByRole("combobox", { name: "Speech model" });
    await waitFor(() =>
      expect(select).toHaveTextContent("Parakeet English (Recommended)"),
    );
  },
};
export const GeneralLoading: Story = {
  beforeEach: () => {
    mockWindows("settings");
    mockIPC(
      (command) => {
        if (command === "pipeline_status")
          return { phase: "idle", error: null };
        return command === "get_settings" ? new Promise(() => {}) : null;
      },
      { shouldMockEvents: true },
    );
    return clearMocks;
  },
  render: () => <GeneralPage />,
};
export const GeneralError: Story = {
  beforeEach: () => {
    mockWindows("settings");
    mockIPC(
      (command) => {
        if (command === "pipeline_status")
          return { phase: "idle", error: null };
        if (command === "get_settings")
          throw new Error("Settings file could not be read.");
        return null;
      },
      { shouldMockEvents: true },
    );
    return clearMocks;
  },
  render: () => <GeneralPage />,
  play: async ({ canvasElement }) => {
    await expect(
      await within(canvasElement).findByRole("alert"),
    ).toHaveTextContent("Settings file could not be read.");
  },
};
export const History: Story = {
  beforeEach: ({ globals }) => mockApp(globals, sampleHistory(Date.now())),
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const slack = await c.findByRole("button", { name: /Ship the history/ });
    await userEvent.click(slack);
    await expect(slack.closest("[aria-current]")).toHaveAttribute(
      "aria-current",
      "true",
    );
    await expect(
      c.getByRole("region", { name: "Dictation" }),
    ).toHaveTextContent("Then hand it to review.");
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() =>
      expect(
        c.getByRole("button", { name: /No speech detected/ }),
      ).toHaveFocus(),
    );
    await expect(c.getByRole("button", { name: "Copy text" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
    await userEvent.click(c.getByRole("button", { name: /Bakery/i }));
  },
};
const layoutOf = (...elements: HTMLElement[]) =>
  elements.map((element) => {
    const { top, left, height, width } = element.getBoundingClientRect();
    return { top, left, height, width };
  });
export const HistoryCopy: Story = {
  beforeEach: ({ globals }) => mockApp(globals, sampleHistory(Date.now())),
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(await c.findByRole("button", { name: /Bakery/i }));
    const copy = c.getByRole("button", { name: "Copy text" });
    const detail = c.getByRole("region", { name: "Dictation" });
    // The actions sit in the detail's own header, so the pane must not need a
    // horizontal scroll to reach them.
    await expect(detail.scrollWidth).toBeLessThanOrEqual(detail.clientWidth);
    const parts = () => [
      detail,
      c.getByRole("button", { name: "Delete dictation" }),
      c.getByRole("button", { name: /Bakery/i }),
    ];
    const before = layoutOf(...parts());
    await userEvent.click(copy);
    await expect(await c.findByText("Copied to clipboard")).toBeInTheDocument();
    await expect(layoutOf(...parts())).toEqual(before);
  },
};
export const HistoryNarrow: Story = {
  beforeEach: ({ globals }) => mockApp(globals, sampleHistory(Date.now())),
  render: () => <GeneralPage page="History" width={640} />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const list = await c.findByRole("group", { name: "Dictations" });
    const rows = within(list).getAllByRole("button", { expanded: false });
    await expect(rows).toHaveLength(5);
    await expect(c.queryByRole("region", { name: "Dictation" })).toBeNull();
  },
};
export const HistoryNarrowOpen: Story = {
  beforeEach: ({ globals }) => mockApp(globals, sampleHistory(Date.now())),
  render: () => <GeneralPage page="History" width={640} />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const bakery = await c.findByRole("button", { name: /bakery/i });
    const slack = c.getByRole("button", { name: /Ship the history/ });
    await userEvent.click(bakery);
    await userEvent.click(slack);
    await expect(bakery).toHaveAttribute("aria-expanded", "false");
    await expect(slack).toHaveAttribute("aria-expanded", "true");
    const detail = c.getByRole("region", { name: "Dictation" });
    await expect(detail).toHaveTextContent("Then hand it to review.");
    await expect(detail.scrollWidth).toBeLessThanOrEqual(detail.clientWidth);
    await userEvent.click(
      within(detail).getByRole("button", { name: "Copy text" }),
    );
    await expect(await c.findByText("Copied to clipboard")).toBeInTheDocument();
    await expect(
      within(detail).getByRole("button", { name: "Delete dictation" }),
    ).toBeVisible();
  },
};
const annotationsHistory = ({
  globals,
}: {
  globals: Record<string, unknown>;
}) =>
  mockApp(
    globals,
    [],
    undefined,
    {},
    {
      entries: sampleAnnotations(Date.now()),
    },
  );
export const HistoryAnnotations: Story = {
  beforeEach: annotationsHistory,
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(await c.findByRole("radio", { name: "Annotations" }));
    const detail = await c.findByRole("region", { name: "Annotation" });
    await expect(detail).toHaveTextContent(
      "This toggle should be on the right.",
    );
    await expect(detail).toHaveTextContent("3 with a contact sheet");
    await expect(detail).toHaveTextContent("3.2 s");
    await expect(detail.scrollWidth).toBeLessThanOrEqual(detail.clientWidth);
  },
};
export const HistoryAnnotationsNarrowOpen: Story = {
  beforeEach: annotationsHistory,
  render: () => <GeneralPage page="History" width={640} />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(await c.findByRole("radio", { name: "Annotations" }));
    const list = await c.findByRole("group", { name: "Annotations" });
    const [first, second] = within(list).getAllByRole("button", {
      expanded: false,
    });
    await userEvent.click(first!);
    await userEvent.click(second!);
    await expect(first).toHaveAttribute("aria-expanded", "false");
    await expect(second).toHaveAttribute("aria-expanded", "true");
    const detail = c.getByRole("region", { name: "Annotation" });
    await expect(detail).toHaveTextContent("Nothing was said in this session.");
    await userEvent.click(
      within(detail).getByRole("button", { name: "Copy again" }),
    );
    await userEvent.click(
      within(detail).getByRole("button", { name: "Reveal in Finder" }),
    );
    await expect(annotateCalls).toEqual([
      "copy_annotation 2026-11-14_16-05-51",
      "reveal_annotation 2026-11-14_16-05-51",
    ]);
  },
};
export const HistoryAnnotationActions: Story = {
  beforeEach: annotationsHistory,
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(await c.findByRole("radio", { name: "Annotations" }));
    const copyAgain = await c.findByRole("button", { name: "Copy again" });
    const before = layoutOf(
      c.getByRole("region", { name: "Annotation" }),
      copyAgain,
      c.getByRole("button", { name: "Reveal in Finder" }),
    );
    await userEvent.click(copyAgain);
    await expect(
      await c.findByText("Summary copied to clipboard"),
    ).toBeInTheDocument();
    await expect(
      layoutOf(
        c.getByRole("region", { name: "Annotation" }),
        copyAgain,
        c.getByRole("button", { name: "Reveal in Finder" }),
      ),
    ).toEqual(before);
    await userEvent.click(c.getByRole("button", { name: "Reveal in Finder" }));
    await expect(annotateCalls).toEqual([
      "copy_annotation 2026-11-15_10-42-08",
      "reveal_annotation 2026-11-15_10-42-08",
    ]);
    await userEvent.click(
      c.getByRole("button", { name: /1 mark, nothing said/ }),
    );
    await expect(
      c.getByRole("region", { name: "Annotation" }),
    ).toHaveTextContent("Nothing was said in this session.");
    await expect(
      c.getByRole("region", { name: "Annotation" }),
    ).toHaveTextContent(/Partial session\.\s*Video cut short/);
  },
};
export const HistoryAnnotationDelete: Story = {
  beforeEach: annotationsHistory,
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(await c.findByRole("radio", { name: "Annotations" }));
    const detail = await c.findByRole("region", { name: "Annotation" });
    await userEvent.click(
      within(detail).getByRole("button", { name: "Delete annotation" }),
    );
    const dialog = await within(document.body).findByRole("alertdialog");
    await userEvent.click(
      await within(dialog).findByRole("button", { name: "Delete annotation" }),
    );
    await waitFor(() =>
      expect(annotateCalls).toContain("delete_annotation 2026-11-15_10-42-08"),
    );
    await waitFor(() =>
      expect(
        c.queryByRole("button", { name: /This toggle should be on the right/ }),
      ).not.toBeInTheDocument(),
    );
  },
};
export const HistoryAnnotationsEmpty: Story = {
  beforeEach: ({ globals }) => mockApp(globals, []),
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(await c.findByRole("radio", { name: "Annotations" }));
    await expect(
      await c.findByRole("heading", { name: "No annotations yet" }),
    ).toBeVisible();
  },
};
export const HistoryDelete: Story = {
  beforeEach: ({ globals }) => mockApp(globals, sampleHistory(Date.now())),
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await c.findByRole("button", { name: /bakery/i });
    await userEvent.click(c.getByRole("button", { name: "Delete dictation" }));
    await expect(c.getByRole("button", { name: "Cancel" })).toHaveFocus();
    await userEvent.keyboard("{Escape}");
    await expect(c.getByRole("button", { name: /bakery/i })).toBeVisible();
    await userEvent.click(c.getByRole("button", { name: "Delete dictation" }));
    await userEvent.click(
      within(
        c.getByRole("alertdialog", { name: "Delete this dictation?" }),
      ).getByRole("button", { name: "Delete dictation" }),
    );
    await waitFor(() =>
      expect(c.queryByRole("button", { name: /bakery/i })).toBeNull(),
    );
    await expect(
      c
        .getByRole("button", { name: /Ship the history/ })
        .closest("[aria-current]"),
    ).toHaveAttribute("aria-current", "true");
  },
};
export const HistoryRetention: Story = {
  beforeEach: ({ globals }) => mockApp(globals, sampleHistory(Date.now())),
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await c.findByRole("combobox", { name: "Keep history for" });
    await choose(canvasElement, "Keep history for", "Forever");
    await expect(c.queryByRole("alertdialog")).toBeNull();
    const keep = c.getByRole("combobox", { name: "Keep history for" });
    await userEvent.click(keep);
    fireEvent.click(
      await within(document.body).findByRole("option", { name: "1 day" }),
    );
    const confirm = await c.findByRole("alertdialog", {
      name: "Delete dictations older than 1 day?",
    });
    await waitFor(() => expect(confirm).toBeVisible());
    await userEvent.click(c.getByRole("button", { name: "Cancel" }));
    await expect(keep).toHaveTextContent("Forever");
  },
};
export const HistoryLoadMore: Story = {
  beforeEach: ({ globals }) => {
    const now = Date.now();
    return mockApp(
      globals,
      Array.from({ length: 503 }, (_, index): HistoryEntry => {
        const [sample] = sampleHistory(now);
        return {
          ...sample,
          id: index + 1,
          text: `Dictation number ${index + 1}`,
          createdAtMs: now - index * minute,
        };
      }),
    );
  },
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(await c.findByText("Showing the latest 500.")).toBeVisible();
    await expect(c.queryByRole("button", { name: /number 501/ })).toBeNull();
    await userEvent.click(
      c.getByRole("button", { name: "Load older dictations" }),
    );
    await expect(
      await c.findByRole("button", { name: /number 503/ }),
    ).toBeVisible();
    await expect(c.queryByText(/Showing the latest/)).toBeNull();
  },
};
export const HistoryEmpty: Story = {
  beforeEach: ({ globals }) => mockApp(globals, []),
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    await expect(
      await within(canvasElement).findByRole("heading", {
        name: "Nothing here yet",
      }),
    ).toBeVisible();
  },
};
export const HistorySearch: Story = {
  beforeEach: ({ globals }) => mockApp(globals, sampleHistory(Date.now())),
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const search = await c.findByRole("searchbox", { name: "Search history" });
    await userEvent.type(search, "slack");
    await waitFor(() =>
      expect(c.getAllByRole("button", { name: /Slack/ })).toHaveLength(1),
    );
    await expect(c.queryByRole("button", { name: /bakery/i })).toBeNull();
  },
};
export const HistoryNoMatches: Story = {
  beforeEach: ({ globals }) => mockApp(globals, sampleHistory(Date.now())),
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const search = await c.findByRole("searchbox", { name: "Search history" });
    await userEvent.type(search, "zebra");
    await userEvent.click(
      await c.findByRole("button", { name: "Clear search" }),
    );
    await expect(search).toHaveValue("");
    await expect(
      await c.findByRole("button", { name: /bakery/i }),
    ).toBeVisible();
    await userEvent.type(search, "zebra");
    await c.findByRole("heading", { name: "No matches for “zebra”" });
  },
};
export const ModelsIdle: Story = {
  beforeEach: ({ globals }) => mockApp(globals, [], { states: {} }),
  render: () => <GeneralPage page="Models" />,
  play: async ({ canvasElement }) => {
    await expect(
      await within(canvasElement).findByText(
        /Download a model to start dictating/,
      ),
    ).toBeVisible();
  },
};
export const ModelsFreshProfile: Story = {
  beforeEach: ({ globals }) =>
    mockApp(globals, [], { active: "parakeet-unified-en", states: {} }),
  render: () => <GeneralPage page="Models" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(
      await c.findByText(
        "Download a model to start dictating. Parakeet English is a good first choice.",
      ),
    ).toBeVisible();
    await expect(
      c.getByRole("region", { name: "Parakeet English" }),
    ).toHaveTextContent("Recommended");
  },
};
export const ModelsDownloadAndCancel: Story = {
  beforeEach: ({ globals }) => mockApp(globals, [], { states: {} }),
  render: () => <GeneralPage page="Models" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(
      await c.findByRole("button", { name: "Download Tiny English" }),
    );
    await userEvent.click(
      await c.findByRole("button", { name: "Cancel Tiny English download" }),
    );
    await expect(
      await c.findByRole("button", { name: "Download Tiny English" }),
    ).toBeVisible();
  },
};
export const ModelsDownloading: Story = {
  beforeEach: ({ globals }) =>
    mockApp(globals, [], {
      states: {
        "base.en": { kind: "installed" },
        "small.en": {
          kind: "downloading",
          received: 205 * MB,
          total: 487_614_201,
          cancelling: false,
        },
      },
    }),
  render: () => <GeneralPage page="Models" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(
      await c.findByRole("progressbar", { name: "Downloading" }),
    ).toBeVisible();
    await expect(c.getByText("205 MB of 488 MB")).toBeVisible();
  },
};
export const ModelsPreparing: Story = {
  beforeEach: ({ globals }) =>
    mockApp(globals, [], { states: { "base.en": { kind: "preparing" } } }),
  render: () => <GeneralPage page="Models" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(await c.findByText("Preparing your model…")).toBeVisible();
    expect(c.queryByText(/Download a model to start/)).toBeNull();
  },
};
export const ModelsInstalled: Story = {
  beforeEach: ({ globals }) =>
    mockApp(globals, [], {
      states: {
        "tiny.en": { kind: "installed" },
        "base.en": { kind: "installed" },
        "small.en": { kind: "installed" },
      },
    }),
  render: () => <GeneralPage page="Models" />,
  play: async ({ canvasElement }) => {
    await expect(
      await within(canvasElement).findByRole("region", {
        name: "Base English",
      }),
    ).toHaveTextContent("In use");
  },
};
export const ModelsUseAndDelete: Story = {
  beforeEach: ({ globals }) =>
    mockApp(globals, [], {
      states: {
        "tiny.en": { kind: "installed" },
        "base.en": { kind: "installed" },
        "small.en": { kind: "installed" },
      },
    }),
  render: () => <GeneralPage page="Models" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(
      await c.findByRole("button", { name: "Use Small English" }),
    );
    await waitFor(() =>
      expect(
        c.getByRole("region", { name: "Small English" }),
      ).toHaveTextContent("In use"),
    );
    await waitFor(() =>
      expect(c.getByRole("region", { name: "Small English" })).toContainElement(
        document.activeElement as HTMLElement,
      ),
    );
    await userEvent.click(
      c.getByRole("button", { name: "Delete Tiny English" }),
    );
    await userEvent.click(c.getByRole("button", { name: "Delete model" }));
    await waitFor(() =>
      expect(
        c.getByRole("button", { name: "Download Tiny English" }),
      ).toHaveFocus(),
    );
  },
};
export const ModelsError: Story = {
  beforeEach: ({ globals }) =>
    mockApp(globals, [], {
      whisperMissing: true,
      states: {
        "base.en": { kind: "installed" },
        "large-v3-turbo": {
          kind: "failed",
          message: "Download failed. Check your connection and try again.",
        },
      },
    }),
  render: () => <GeneralPage page="Models" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(await c.findByText("brew install whisper.cpp")).toBeVisible();
    await expect(
      c.getByRole("button", { name: "Retry Large v3 Turbo download" }),
    ).toBeVisible();
  },
};
export const Annotate: Story = {
  beforeEach: ({ globals }) => mockApp(globals, []),
  render: () => <GeneralPage page="Annotate" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(
      await c.findByRole("switch", { name: "Annotate" }),
    ).toBeChecked();
    await expect(await c.findByText("Allowed")).toBeVisible();
    await expect(
      c.getByRole("switch", { name: "Show in screen recordings" }),
    ).not.toBeChecked();
  },
};
export const AnnotateToggles: Story = {
  beforeEach: ({ globals }) => mockApp(globals, []),
  render: () => <GeneralPage page="Annotate" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const enabled = await c.findByRole("switch", { name: "Annotate" });
    const show = c.getByRole("switch", { name: "Show in screen recordings" });
    await userEvent.click(show);
    await waitFor(() => expect(show).toBeChecked());
    await userEvent.click(enabled);
    await waitFor(() => expect(enabled).not.toBeChecked());
    await expect(c.getByRole("button", { name: /^Shortcut,/ })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  },
};
export const AnnotateNotAllowed: Story = {
  beforeEach: ({ globals }) =>
    mockApp(globals, [], undefined, {}, { permission: false }),
  render: () => <GeneralPage page="Annotate" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(await c.findByText("Not allowed")).toBeVisible();
    await userEvent.click(
      c.getByRole("button", { name: "Open System Settings" }),
    );
    await expect(annotateCalls).toEqual(["open_screen_recording_settings"]);
  },
};
export const Output: Story = {
  beforeEach: ({ globals }) => mockApp(globals, []),
  render: () => <GeneralPage page="Output" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const method = await c.findByRole("combobox", {
      name: "After dictation",
    });
    await expect(method).toHaveTextContent(/^Paste$/);
    await choose(canvasElement, "After dictation", "Copy only");
    await expect(c.getByText(/Paste with ⌘V when you’re ready/)).toBeVisible();
    await choose(canvasElement, "After dictation", /^Paste$/);
    await choose(
      canvasElement,
      "After dictation",
      "Paste and keep on clipboard",
    );
    const capitalise = c.getByRole("switch", {
      name: "Capitalise the first letter",
    });
    await userEvent.click(capitalise);
    await waitFor(() => expect(capitalise).toBeChecked());
  },
};
export const OutputCopyOnly: Story = {
  beforeEach: ({ globals }) =>
    mockApp(globals, [], undefined, {
      pasteMethod: "copyOnly",
      trailingSpace: true,
      capitalizeFirst: true,
    }),
  render: () => <GeneralPage page="Output" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(
      await c.findByRole("combobox", { name: "After dictation" }),
    ).toHaveTextContent("Copy only");
    await expect(
      c.getByRole("switch", { name: "Add a trailing space" }),
    ).toBeChecked();
  },
};
export const Advanced: Story = {
  beforeEach: ({ globals }) => mockApp(globals, []),
  render: () => <GeneralPage page="Advanced" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const warm = await c.findByRole("switch", {
      name: "Keep the microphone ready",
    });
    await expect(warm).not.toBeChecked();
    await userEvent.click(warm);
    await waitFor(() => expect(warm).toBeChecked());
    await expect(c.getByText(/first word isn’t clipped/)).toBeVisible();
    await choose(canvasElement, "Overlay position", "Top");
    await choose(canvasElement, "Overlay position", "Bottom");
  },
};
export const About: Story = {
  beforeEach: ({ globals }) => mockApp(globals, []),
  render: () => <GeneralPage page="About" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await expect(await c.findByText(/Logs\/com\.kelechiodom/)).toBeVisible();
    await expect(c.getByRole("status", { name: "Version" })).toHaveTextContent(
      "0.1.0",
    );
    await userEvent.click(
      c.getByRole("button", { name: "Show in Finder, app data" }),
    );
    await userEvent.click(
      c.getByRole("button", { name: "Show in Finder, logs" }),
    );
    await userEvent.click(
      c.getByRole("button", { name: "View on GitHub, source code" }),
    );
    await expect(opened).toEqual(["data", "logs", "source"]);
  },
};
const checkForUpdates = async (c: ReturnType<typeof within>) => {
  await userEvent.click(c.getByRole("button", { name: "Check now" }));
};
export const AboutUpToDate: Story = {
  beforeEach: ({ globals }) => {
    updateResult = { kind: "upToDate" };
    return mockApp(globals, []);
  },
  render: () => <GeneralPage page="About" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await checkForUpdates(c);
    await expect(await c.findByText("dictate is up to date.")).toBeVisible();
  },
};
export const AboutUpdateWaitsForIdle: Story = {
  beforeEach: ({ globals }) => {
    updateResult = { kind: "waiting", version: "0.2.0" };
    return mockApp(globals, []);
  },
  render: () => <GeneralPage page="About" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await checkForUpdates(c);
    await expect(
      await c.findByText(/0\.2\.0 is downloaded\. It installs and restarts/),
    ).toBeVisible();
    await expect(c.getByRole("button", { name: "Check now" })).toHaveAttribute(
      "aria-disabled",
      "true",
    );
  },
};
export const AboutUpdateFailed: Story = {
  beforeEach: ({ globals }) => {
    updateResult = {
      kind: "failed",
      message: "Couldn't check for updates: could not reach the server",
    };
    return mockApp(globals, []);
  },
  render: () => <GeneralPage page="About" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await checkForUpdates(c);
    await expect(
      await c.findByText(/could not reach the server/),
    ).toBeVisible();
    await expect(c.getByRole("button", { name: "Check now" })).toBeEnabled();
  },
};
export const Appearance: Story = {
  beforeEach: mockSettings,
  render: () => <GeneralPage page="Appearance" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    // Two groups: the mode, then the theme for the side that mode shows.
    const mode = await c.findByRole("radiogroup", { name: "Mode" });
    for (const name of ["System", "Light", "Dark"])
      expect(within(mode).getByRole("radio", { name })).toBeInTheDocument();
    const themes = c.getByRole("radiogroup", { name: /^(Light|Dark) theme$/ });
    for (const name of ["Ember", "Paper", "Capy", "Rosé Pine", "Neutral"])
      expect(within(themes).getByRole("radio", { name })).toBeInTheDocument();
    expect(
      within(mode)
        .getAllByRole("radio")
        .filter((radio) => (radio as HTMLInputElement).checked),
    ).toHaveLength(1);
    // Every theme is a picture of its light and dark side, nothing else.
    expect(themes.querySelectorAll("svg")).toHaveLength(5);
    expect(themes.querySelectorAll(".astryx-status-dot")).toHaveLength(0);
    // Glass is opt-in and starts off.
    expect(
      c.getByRole("switch", { name: "Translucent window" }),
    ).not.toBeChecked();
  },
};

export const AppearanceGlass: Story = {
  name: "Appearance / translucent window on",
  beforeEach: ({ globals }) => mockApp(globals, [], undefined, { glass: true }),
  render: () => <GeneralPage page="Appearance" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const toggle = await c.findByRole("switch", {
      name: "Translucent window",
    });
    expect(toggle).toBeChecked();
    expect(
      c.getByText(/Lets what is behind the window show through/),
    ).toBeInTheDocument();
    await userEvent.click(toggle);
    await expect(
      c.getByRole("switch", { name: "Translucent window" }),
    ).not.toBeChecked();
  },
};

export const Overlay: Story = {
  render: () => (
    <main
      className="story-page"
      style={{ display: "grid", placeItems: "end center", padding: "32px" }}
    >
      <RecordingCapsule
        state={{ kind: "recording", seconds: 33, level: 0.65 }}
      />
    </main>
  ),
};
