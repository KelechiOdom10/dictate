import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { HStack } from "@astryxdesign/core/HStack";
import { Text } from "@astryxdesign/core/Text";
import { KeyboardShortcut, ShortcutRecorder } from "./ShortcutRecorder";
const meta = { title: "Components/ShortcutRecorder" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
function RecorderDemo({ disabled = false }: { disabled?: boolean }) {
  const [value, setValue] = useState("Alt+Shift+Space");
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState("");
  return (
    // An inline row, so the button sizes to its content the way it does in a
    // settings row instead of stretching to the frame.
    <HStack gap={2} vAlign="center">
      <ShortcutRecorder
        label="Shortcut"
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
      {error && <Text role="alert">{error}</Text>}
    </HStack>
  );
}
export const CaptureFlow: Story = {
  name: "Capture flow",
  render: () => <RecorderDemo />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const button = c.getByRole("button");
    await userEvent.click(button);
    await userEvent.keyboard("a");
    await expect(c.getByRole("alert")).toHaveTextContent("Include Command");
    await userEvent.keyboard("{Escape}");
    await expect(button).toHaveTextContent(/space/i);
    await userEvent.click(button);
    await userEvent.keyboard("{Control>}{Shift>}D{/Shift}{/Control}");
    await expect(button).toHaveTextContent(/⌃.*⇧.*D/);
    await userEvent.click(button);
    await userEvent.keyboard("{Tab}");
    await expect(button).toHaveTextContent(/⌃.*⇧.*D/);
  },
};
export const Disabled: Story = { render: () => <RecorderDemo disabled /> };
// The resting state and the waiting state, each held from the first frame, so
// a screenshot of either shows what its name claims. Both are 161x32: the
// invisible copy of the wider state reserves the size. CaptureFlow below runs
// the whole click, error, Escape and commit sequence instead.
export const Resting: Story = {
  render: () => <RecorderDemo />,
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const button = c.getByRole("button");
    await expect(button).toHaveTextContent(/⌥.*⇧.*SPACE/);
    expect(
      button.querySelector(".ui-capture-live:not(.ui-capture-ghost)"),
    ).toBeNull();
  },
};
export const CaptureWaiting: Story = {
  name: "Capture waiting",
  render: () => (
    <ShortcutRecorder
      label="Shortcut"
      value="Alt+Space"
      recording
      onStart={() => {}}
      onCancel={() => {}}
      onCommit={() => {}}
      onError={() => {}}
    />
  ),
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const button = c.getByRole("button");
    await expect(button).toHaveTextContent(/Press shortcut/);
    await expect(button).toHaveAccessibleName(/Esc to cancel/);
    // Two dots live in the button: the invisible copy that reserves the width
    // and the one that paints. This story is the painted one.
    expect(
      button.querySelector(
        ".ui-capture-live:not(.ui-capture-ghost) .astryx-status-dot",
      ),
    ).not.toBeNull();
    await expect(
      button.querySelector(".ui-capture-live:not(.ui-capture-ghost) kbd"),
    ).toHaveTextContent("Esc");
  },
};
export const Keycaps: Story = {
  name: "KeyboardShortcut",
  render: () => (
    <HStack gap={4}>
      <KeyboardShortcut value="Super+Shift+KeyD" />
      <KeyboardShortcut value="Alt+Space" />
      <KeyboardShortcut value="Control+Alt+Shift+Super+KeyK" />
    </HStack>
  ),
};
