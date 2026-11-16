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
    const side = (name: "Light theme" | "Dark theme") =>
      within(canvas.getByRole("radiogroup", { name: "Theme for" })).getByRole(
        "radio",
        { name },
      );
    // The list edits the appearance on screen at first.
    await expect(
      canvas.getByRole("radiogroup", { name: "Theme for light" }),
    ).toBeInTheDocument();
    // The side on screen is the one being edited, and it says so.
    await expect(canvas.getAllByText("In use now")).toHaveLength(1);
    const light = within(
      canvas.getByRole("radiogroup", { name: "Theme for light" }),
    );
    await userEvent.click(light.getByRole("radio", { name: /^Paper/ }));
    await expect(light.getByRole("radio", { name: /^Paper/ })).toBeChecked();
    // Switching the side shows the other choice, untouched.
    await userEvent.click(side("Dark theme"));
    const dark = within(
      canvas.getByRole("radiogroup", { name: "Theme for dark" }),
    );
    await expect(dark.getByRole("radio", { name: /^Ember/ })).toBeChecked();
    await userEvent.click(dark.getByRole("radio", { name: /^Capy/ }));
    await expect(dark.getByRole("radio", { name: /^Capy/ })).toBeChecked();
    // Both choices stay on screen while one list edits them.
    await expect(
      canvas.getByText("Light uses Paper · Dark uses Capy"),
    ).toBeInTheDocument();
    // Editing the side that is not on screen drops the marker, and going back
    // brings it with the choice you made.
    await expect(canvas.queryAllByText("In use now")).toHaveLength(0);
    await userEvent.click(side("Light theme"));
    await expect(canvas.getAllByText("In use now")).toHaveLength(1);
    await expect(
      within(
        canvas.getByRole("radiogroup", { name: "Theme for light" }),
      ).getByRole("radio", { name: /^Paper/ }),
    ).toBeChecked();
  },
};
export const Disabled: Story = {
  name: "AppearancePicker / while saving",
  render: () => <Picker disabled />,
};
