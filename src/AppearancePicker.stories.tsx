import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { AppearancePicker } from "./AppearancePicker";
import type { Settings } from "./settings";

const meta = { title: "Components/AppearancePicker" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;

function Picker({ disabledReason }: { disabledReason?: string }) {
  const [value, setValue] = useState<
    Pick<Settings, "themeLight" | "themeDark" | "appearanceMode">
  >({ themeLight: "ember", themeDark: "ember", appearanceMode: "light" });
  return (
    <AppearancePicker
      themeLight={value.themeLight}
      themeDark={value.themeDark}
      mode={value.appearanceMode}
      disabledReason={disabledReason}
      onChange={(patch) => setValue((current) => ({ ...current, ...patch }))}
    />
  );
}

export const EveryTheme: Story = {
  name: "AppearancePicker / a theme for each appearance",
  render: () => <Picker />,
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    const mode = (name: string) =>
      within(canvas.getByRole("radiogroup", { name: "Mode" })).getByRole(
        "radio",
        { name },
      );
    const theme = (side: "Light" | "Dark", name: string) =>
      within(
        canvas.getByRole("radiogroup", { name: `${side} theme` }),
      ).getByRole("radio", { name });
    await expect(mode("Light")).toBeChecked();
    await userEvent.click(theme("Light", "Paper"));
    await expect(theme("Light", "Paper")).toBeChecked();
    await expect(canvas.getByText("Dark uses Ember.")).toBeInTheDocument();
    // Picking the Dark tile turns the same grid to the dark theme.
    await userEvent.click(mode("Dark"));
    await expect(theme("Dark", "Ember")).toBeChecked();
    await userEvent.click(theme("Dark", "Capy"));
    await expect(canvas.getByText("Light uses Paper.")).toBeInTheDocument();
    await userEvent.click(mode("Light"));
    await expect(theme("Light", "Paper")).toBeChecked();
    await expect(canvas.getByText("Dark uses Capy.")).toBeInTheDocument();
  },
};
export const Disabled: Story = {
  name: "AppearancePicker / while Annotate runs",
  render: () => (
    <Picker disabledReason="Stop Annotate before changing appearance." />
  ),
  play: async ({ canvasElement }) => {
    const radios = within(canvasElement).getAllByRole("radio");
    await expect(radios).toHaveLength(8);
    for (const radio of radios)
      await expect(radio).toHaveAttribute("aria-disabled", "true");
    await userEvent.tab();
    const light = within(canvasElement).getByRole("radio", { name: "Light" });
    light.focus();
    await expect(light).toHaveFocus();
    await userEvent.keyboard("{ArrowRight}");
    await expect(light).toBeChecked();
  },
};
