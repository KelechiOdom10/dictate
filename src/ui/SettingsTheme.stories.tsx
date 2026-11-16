import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { Switch } from "@astryxdesign/core/Switch";
import { VStack } from "@astryxdesign/core/VStack";
import { SettingsTheme } from "./SettingsTheme";

const meta = {
  title: "Components/SettingsTheme",
  component: SettingsTheme,
  render: (_, { globals }) => (
    <SettingsTheme palette={globals.theme} mode={globals.mode}>
      <Startup />
    </SettingsTheme>
  ),
} satisfies Meta<typeof SettingsTheme>;
export default meta;
type Story = StoryObj<typeof meta>;

function Startup() {
  const [value, setValue] = useState(false);
  return (
    <VStack gap={4}>
      <Switch
        label="Launch at login"
        description="Keep dictate ready in your menu bar."
        value={value}
        onChange={setValue}
      />
      <Switch label="Unavailable off" value={false} isDisabled />
      <Switch label="Unavailable on" value isDisabled />
    </VStack>
  );
}

export const StartupStates: Story = {
  args: { children: null },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const toggle = canvas.getByRole("switch", { name: "Launch at login" });
    await userEvent.click(toggle);
    await expect(toggle).toBeChecked();
    toggle.focus();
    await userEvent.keyboard(" ");
    await expect(toggle).not.toBeChecked();
    const disabled = canvas.getByRole("switch", { name: "Unavailable on" });
    await userEvent.click(disabled);
    await expect(disabled).toBeChecked();
    await expect(disabled).toHaveAttribute("aria-disabled", "true");
  },
};
