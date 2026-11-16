import type { Meta, StoryObj } from "@storybook/react-vite";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { AudioLines, Copy } from "lucide-react";
import { useState } from "react";
import { expect, within } from "storybook/test";
import { settingsSchema, type Settings } from "./settings";
import SettingsWindow from "./SettingsWindow";
import {
  Button,
  DetailPanel,
  FilterChips,
  ListItem,
  ModelCard,
  RecordingCapsule,
  SearchField,
  SidebarNav,
} from "./ui";

const meta = {
  title: "Pages",
  parameters: { layout: "fullscreen" },
} satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
async function mockSettings() {
  let settings: Settings = {
    version: 1,
    shortcut: "Alt+Shift+Space",
    shortcutMode: "hold",
    microphone: null,
    launchAtLogin: false,
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
      return null;
    },
    { shouldMockEvents: true },
  );
  return () => clearMocks();
}
function GeneralPage() {
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
        <SettingsWindow />
      </QueryClientProvider>
    </div>
  );
}
export const General: Story = {
  globals: { theme: "light" },
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
export const GeneralDark: Story = { ...General, globals: { theme: "dark" } };
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
  render: () => (
    <MockShell page="History">
      <div className="story-stack">
        <SearchField label="Search history" value="" onChange={() => {}} />
        <FilterChips
          label="History filter"
          options={[
            { value: "all", label: "All" },
            { value: "starred", label: "Starred" },
          ]}
          value="all"
          onChange={() => {}}
        />
        <ListItem
          date="Today · 9:41 AM"
          title="A thought for later"
          preview="Make space for the things that matter. Start with one small thing."
          actions={
            <Button variant="ghost">
              <Copy size={14} aria-hidden="true" />
              Copy text
            </Button>
          }
        />
        <ListItem
          date="Yesterday · 4:30 PM"
          title="The long way home"
          preview="Remember to pick up coffee on the way back."
        />
        <DetailPanel
          title="A thought for later"
          metadata={[
            { label: "Recorded", value: "Today, 9:41 AM" },
            { label: "Duration", value: "33 seconds" },
          ]}
        >
          <p>
            Make space for the things that matter. Start with one small thing.
          </p>
        </DetailPanel>
      </div>
    </MockShell>
  ),
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
