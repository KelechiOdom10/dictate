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
    Pick<Settings, "theme" | "appearanceMode">
  >({ theme: "ember", appearanceMode: "system" });
  return (
    <AppearancePicker
      theme={value.theme}
      mode={value.appearanceMode}
      disabled={disabled}
      onChange={(patch) => setValue((current) => ({ ...current, ...patch }))}
    />
  );
}

export const EveryTheme: Story = {
  name: "AppearancePicker / every theme, mode set separately",
  render: () => <Picker />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("radio", { name: /Ember/ })).toBeChecked();
    for (const name of ["Paper", "Capy", "Rosé Pine", "Neutral"]) {
      const radio = canvas.getByRole("radio", { name: new RegExp(name) });
      await userEvent.click(radio);
      await expect(radio).toBeChecked();
    }
    const dark = canvas.getByRole("radio", { name: "Dark" });
    await userEvent.click(dark);
    await expect(dark).toBeChecked();
    await expect(canvas.getByRole("radio", { name: /Neutral/ })).toBeChecked();
  },
};
export const Disabled: Story = {
  name: "AppearancePicker / while saving",
  render: () => <Picker disabled />,
};
