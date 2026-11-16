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
    const light = (theme: string) =>
      canvas.getByRole("radio", { name: `Set light theme to ${theme}` });
    const dark = (theme: string) =>
      canvas.getByRole("radio", { name: `Set dark theme to ${theme}` });
    await expect(light("Ember")).toBeChecked();
    await expect(dark("Ember")).toBeChecked();
    // The two halves of a card are two different choices.
    await userEvent.click(light("Paper"));
    await expect(light("Paper")).toBeChecked();
    await expect(dark("Paper")).not.toBeChecked();
    await userEvent.click(dark("Capy"));
    await expect(dark("Capy")).toBeChecked();
    await expect(light("Capy")).not.toBeChecked();
    await expect(
      canvas.getByText("Light uses Paper · Dark uses Capy", { exact: true }),
    ).toBeInTheDocument();
    // Mode is three tiles of its own, and one of them is picked.
    const tiles = ["System", "Light", "Dark"].map((name) =>
      canvas.getByRole("radio", { name }),
    );
    await expect(tiles[0]).toBeChecked();
    await userEvent.click(tiles[2]);
    await expect(tiles[2]).toBeChecked();
    await expect(tiles[0]).not.toBeChecked();
    // Exactly one half carries the in-use marker.
    await expect(canvas.getAllByLabelText("Current appearance")).toHaveLength(
      1,
    );
  },
};
export const Disabled: Story = {
  name: "AppearancePicker / while saving",
  render: () => <Picker disabled />,
};
