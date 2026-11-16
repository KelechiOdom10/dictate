import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { KeyboardShortcut, ShortcutRecorder } from "./ShortcutRecorder";
const meta = { title: "Components/ShortcutRecorder" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
function RecorderDemo({ disabled = false }: { disabled?: boolean }) {
  const [value, setValue] = useState("Alt+Shift+Space");
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState("");
  return (
    <div className="story-stack">
      <ShortcutRecorder
        aria-describedby="shortcut-instructions"
        value={value}
        recording={recording}
        disabled={disabled}
        onStart={() => {
          setError("");
          setRecording(true);
        }}
        onCancel={() => setRecording(false)}
        onCommit={(value) => {
          setValue(value);
          setRecording(false);
          setError("");
        }}
        onError={setError}
      />
      <p id="shortcut-instructions">
        Press to change your shortcut. Escape or Tab cancels.
      </p>
      {error && <p role="alert">{error}</p>}
    </div>
  );
}
export const Capture: Story = {
  render: () => <RecorderDemo />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const button = c.getByRole("button");
    await userEvent.click(button);
    await userEvent.keyboard("a");
    await expect(c.getByRole("alert")).toHaveTextContent("Include Command");
    await userEvent.keyboard("{Escape}");
    await expect(button).toHaveTextContent("Space");
    await userEvent.click(button);
    await userEvent.keyboard("{Control>}{Shift>}D{/Shift}{/Control}");
    await expect(button).toHaveTextContent("⌃⇧D");
    await userEvent.click(button);
    await userEvent.keyboard("{Tab}");
    await expect(button).toHaveTextContent("⌃⇧D");
  },
};
export const Disabled: Story = { render: () => <RecorderDemo disabled /> };
export const Recording: Story = {
  render: () => (
    <ShortcutRecorder
      value="Alt+Space"
      recording
      onStart={() => {}}
      onCancel={() => {}}
      onCommit={() => {}}
      onError={() => {}}
    />
  ),
};
export const Keycaps: Story = {
  name: "KeyboardShortcut",
  render: () => (
    <div className="story-row">
      <KeyboardShortcut value="Super+Shift+KeyD" />
      <KeyboardShortcut value="Alt+Space" />
    </div>
  ),
};
