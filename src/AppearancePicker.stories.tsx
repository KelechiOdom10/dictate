import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { AppearancePicker } from "./AppearancePicker";
import type { Settings } from "./settings";

const meta = { title: "Components/AppearancePicker" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

function Picker({ disabled = false }: { disabled?: boolean }) {
  const [value, setValue] = useState<
    Pick<Settings, "themeLight" | "themeDark" | "appearanceMode">
  >({ themeLight: "ember", themeDark: "ember", appearanceMode: "system" });
  return (
    <AppearancePicker
      themeLight={value.themeLight}
      themeDark={value.themeDark}
      mode={value.appearanceMode}
      disabled={disabled}
      onChange={(patch) => setValue((current) => ({ ...current, ...patch }))}
    />
  );
}

export const EveryTheme: Story = {
  name: "AppearancePicker / a theme for each appearance",
  render: () => <Picker />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const light = within(
      canvas.getByRole("radiogroup", { name: "Theme in light" }),
    );
    const dark = within(
      canvas.getByRole("radiogroup", { name: "Theme in dark" }),
    );
    await expect(light.getByRole("radio", { name: /Ember/ })).toBeChecked();
    await expect(dark.getByRole("radio", { name: /Ember/ })).toBeChecked();
    await userEvent.click(light.getByRole("radio", { name: /Paper/ }));
    await expect(light.getByRole("radio", { name: /Paper/ })).toBeChecked();
    // Picking in one column leaves the other alone.
    await expect(dark.getByRole("radio", { name: /Ember/ })).toBeChecked();
    await userEvent.click(dark.getByRole("radio", { name: /Capy/ }));
    await expect(dark.getByRole("radio", { name: /Capy/ })).toBeChecked();
    await expect(light.getByRole("radio", { name: /Paper/ })).toBeChecked();
    // Exactly one column says it is the one on screen, whatever the mode.
    await expect(canvas.getAllByText("In use now")).toHaveLength(1);
    const modeDark = canvas.getByRole("radio", { name: "Dark" });
    await userEvent.click(modeDark);
    await expect(modeDark).toBeChecked();
    await expect(dark.getByRole("radio", { name: /Capy/ })).toBeChecked();
    await expect(canvas.getAllByText("In use now")).toHaveLength(1);
  },
};
export const Disabled: Story = {
  name: "AppearancePicker / while saving",
  render: () => <Picker disabled />,
};
