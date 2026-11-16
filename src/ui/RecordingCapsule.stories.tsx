import type { Meta, StoryObj } from "@storybook/react-vite";
import { RecordingCapsule, LevelMeter } from "./RecordingCapsule";
import { KeyboardShortcut } from "./ShortcutRecorder";
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
export const Done: Story = { args: { state: { kind: "done" } } };
export const Error: Story = {
  args: { state: { kind: "error", message: "Microphone access is required." } },
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
      <RecordingCapsule
        state={{ kind: "error", message: "Microphone access is required." }}
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
