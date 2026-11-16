import type { Meta, StoryObj } from "@storybook/react-vite";
import { RecordingCapsule, LevelMeter } from "./RecordingCapsule";
import { KeyboardShortcut } from "./ShortcutRecorder";
import { expect, within } from "storybook/test";
import problems from "../problems.fixture.json";
const meta = {
  title: "Components/RecordingCapsule",
  component: RecordingCapsule,
} satisfies Meta<typeof RecordingCapsule>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Recording: Story = {
  args: { state: { kind: "recording", seconds: 33, level: 0.65 } },
};
export const Transcribing: Story = {
  args: { state: { kind: "transcribing" } },
};
export const Skipped: Story = { args: { state: { kind: "skipped" } } };
export const Done: Story = { args: { state: { kind: "done" } } };
export const Copied: Story = { args: { state: { kind: "copied" } } };
export const Error: Story = {
  args: {
    state: {
      kind: "error",
      title: problems.accessibility.title,
      fix: problems.accessibility.fix,
    },
  },
};
export const RealErrors: Story = {
  name: "Error / real pipeline messages",
  args: { state: { kind: "done" } },
  render: () => (
    <div className="story-stack" style={{ maxWidth: 420 }}>
      {Object.values(problems).map(({ title, fix }) => (
        <RecordingCapsule key={title} state={{ kind: "error", title, fix }} />
      ))}
    </div>
  ),
  play: async ({ canvasElement }) => {
    const alerts = within(canvasElement).getAllByRole("alert");
    await expect(alerts).toHaveLength(Object.keys(problems).length);
    for (const [index, { title, fix }] of Object.values(problems).entries()) {
      await expect(alerts[index]).toHaveTextContent(`${title}${fix}`, {
        normalizeWhitespace: false,
      });
      await expect(alerts[index]).not.toHaveTextContent(
        /Open Settings to retry/,
      );
      for (const line of alerts[index].querySelectorAll("span"))
        await expect(line.scrollWidth).toBeLessThanOrEqual(line.clientWidth);
    }
  },
};
export const Ready: Story = {
  args: {
    state: { kind: "ready", message: "Ready" },
    shortcut: <KeyboardShortcut value="Alt+Shift+Space" />,
  },
};
export const AllStates: Story = {
  args: { state: { kind: "done" } },
  render: () => (
    <div className="story-stack">
      <RecordingCapsule
        state={{ kind: "recording", seconds: 33, level: 0.65 }}
      />
      <RecordingCapsule state={{ kind: "transcribing" }} />
      <RecordingCapsule state={{ kind: "done" }} />
      <RecordingCapsule state={{ kind: "copied" }} />
      <RecordingCapsule state={{ kind: "skipped" }} />
      <RecordingCapsule
        state={{
          kind: "error",
          title: problems.noSamples.title,
          fix: problems.noSamples.fix,
        }}
      />
    </div>
  ),
};
export const Levels: Story = {
  name: "LevelMeter / silent, speech, peak",
  args: { state: { kind: "done" } },
  render: () => (
    <div className="story-stack">
      <LevelMeter level={0} label="Silent" />
      <LevelMeter level={0.6} label="Speech" />
      <LevelMeter level={1} label="Peak" />
    </div>
  ),
};

export const StartupRecovery: Story = {
  args: {
    state: {
      kind: "error",
      title: "Settings couldn’t be read",
      fix: "Using defaults. Your old file is saved beside them.",
    },
  },
};
export const MissingMicrophone: Story = {
  args: {
    state: { kind: "recording", seconds: 12 },
    notice: "Using the default microphone",
    shortcut: <KeyboardShortcut value="Alt+Shift+Space" />,
  },
};
