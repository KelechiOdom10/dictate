import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { SettingsGroup, SettingsRow, SidebarNav } from "./Settings";
import { Toggle } from "./Controls";
const meta = { title: "Components/Settings" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
export const Row: Story = {
  name: "SettingsRow",
  render: () => (
    <SettingsRow
      id="row-login"
      label="Launch at login"
      help="Keep dictate ready in your menu bar."
    >
      <Toggle id="row-login" aria-describedby="row-login-help" />
    </SettingsRow>
  ),
};
export const Group: Story = {
  name: "SettingsGroup",
  render: () => (
    <SettingsGroup title="Startup">
      <SettingsRow
        id="group-login"
        label="Launch at login"
        help="Keep dictate ready in your menu bar."
      >
        <Toggle id="group-login" aria-describedby="group-login-help" />
      </SettingsRow>
    </SettingsGroup>
  ),
};
export const Navigation: Story = {
  name: "SidebarNav / selected and unselected",
  render: () => {
    const [value, setValue] = useState("General");
    return (
      <SidebarNav
        label="Settings"
        items={["General", "Models", "History"].map((label) => ({
          label,
          value: label,
        }))}
        value={value}
        onChange={setValue}
      />
    );
  },
};
