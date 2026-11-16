import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { HStack } from "@astryxdesign/core/HStack";
import { Text } from "@astryxdesign/core/Text";
import { VStack } from "@astryxdesign/core/VStack";
import { KeyboardShortcut, ShortcutRecorder } from "./ShortcutRecorder";
const meta = { title: "Components/ShortcutRecorder" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
function RecorderDemo({ disabled = false }: { disabled?: boolean }) {
  const [value, setValue] = useState("Alt+Shift+Space");
  const [recording, setRecording] = useState(false);
  const [error, setError] = useState("");
  return (
    <VStack gap={2}>
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
      <Text>Press to change your shortcut. Escape or Tab cancels.</Text>
      {error && <Text role="alert">{error}</Text>}
    </VStack>
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
export const Recording: Story = {
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
