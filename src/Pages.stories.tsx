import "./App.css";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { AudioLines } from "lucide-react";
import { useState } from "react";
import { expect, userEvent, waitFor, within } from "storybook/test";
import type { HistoryEntry, TakeStatus } from "./history";
import { settingsSchema, type Settings } from "./settings";
import SettingsWindow from "./SettingsWindow";
import { ModelCard, RecordingCapsule, SidebarNav } from "./ui";

const meta = {
  title: "Pages",
  parameters: { layout: "fullscreen" },
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
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

async function mockSettings({ globals }: { globals: Record<string, unknown> }) {
  return mockApp(globals, []);
}
function mockApp(
  globals: Record<string, unknown>,
  initialHistory: HistoryEntry[],
) {
  let history = initialHistory;
  let settings: Settings = {
    version: 1,
    shortcut: "Alt+Shift+Space",
    shortcutMode: "hold",
    microphone: null,
    launchAtLogin: false,
    theme: globals.theme === "paper" ? "paper" : "ember",
    overlayPosition: "bottom",
    historyRetention: "week",
    appearanceMode: globals.mode === "dark" ? "dark" : "light",
  };
  mockWindows("settings");
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
      if (command === "list_history") {
        const search = String(
          args && "search" in args ? args.search : "",
        ).toLowerCase();
        return history.filter((entry) =>
          `${entry.text} ${entry.app ?? ""}`.toLowerCase().includes(search),
        );
      }
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
function GeneralPage({ page = "General" }: { page?: string }) {
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
    <div style={{ height: "100vh" }}>
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
    await expect(
      await within(canvasElement).findByRole("switch", {
        name: "Launch at login",
      }),
    ).toBeVisible();
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
function MockShell({
  page,
  children,
}: {
  page: string;
  children: React.ReactNode;
}) {
  return (
    <div className="settings-shell story-page">
      <aside>
        <div className="brand">
          <AudioLines size={22} aria-hidden="true" />
          <span>dictate</span>
        </div>
        <SidebarNav
          label="Settings preview"
          items={["General", "Models", "History"].map((label) => ({
            label,
            value: label,
          }))}
          value={page}
          onChange={() => {}}
        />
        <footer>Preview · sample data</footer>
      </aside>
      <main className="settings-content">
        <header>
          <h1>{page}</h1>
          <p>
            {page === "History"
              ? "A little space for everything you’ve said."
              : "Find the right balance for your words."}
          </p>
        </header>
        {children}
      </main>
    </div>
  );
}
export const History: Story = {
  beforeEach: ({ globals }) => mockApp(globals, sampleHistory(Date.now())),
  render: () => <GeneralPage page="History" />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const slack = await c.findByRole("button", { name: /Ship the history/ });
    await userEvent.click(slack);
    await expect(slack).toHaveAttribute("aria-current", "true");
    await expect(c.getByRole("region", { name: "3h ago" })).toHaveTextContent(
      "Then hand it to review.",
    );
    await userEvent.keyboard("{ArrowDown}");
    await expect(
      c.getByRole("button", { name: /No speech detected/ }),
    ).toHaveFocus();
    await expect(c.getByRole("button", { name: "Copy text" })).toBeDisabled();
    await userEvent.click(c.getByRole("button", { name: /Bakery/i }));
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
export const Models: Story = {
  render: () => (
    <MockShell page="Models">
      <div className="story-stack">
        <ModelCard
          name="Base English"
          description="A balanced everyday companion."
          accuracy={75}
          speed={90}
          size="142 MB"
          badge="Recommended"
          status={{ kind: "installed" }}
        />
        <ModelCard
          name="Small English"
          description="More detail, a little more time."
          accuracy={90}
          speed={65}
          size="466 MB"
          status={{ kind: "downloading", progress: 42 }}
        />
        <p className="ui-date">
          Illustrative ratings for this preview, not measured benchmarks.
        </p>
      </div>
    </MockShell>
  ),
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
