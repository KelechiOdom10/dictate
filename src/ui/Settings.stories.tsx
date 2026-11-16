import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import { Selector } from "@astryxdesign/core/Selector";
import { Switch } from "@astryxdesign/core/Switch";
import { PathText, SettingsGroup, SettingsRow } from "./Settings";
const meta = { title: "Components/Settings" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
function Login() {
  const [value, setValue] = useState(false);
  return (
    <SettingsRow
      label="Launch at login"
      help="Keep dictate ready in your menu bar."
    >
      <Switch
        label="Launch at login"
        isLabelHidden
        value={value}
        onChange={setValue}
      />
    </SettingsRow>
  );
}
export const Group: Story = {
  name: "SettingsGroup / switch and selector rows",
  render: () => {
    const [position, setPosition] = useState("bottom");
    return (
      <SettingsGroup title="Startup">
        <Login />
        <SettingsRow
          label="Overlay position"
          help="Where the recording overlay appears on your screen."
        >
          <Selector
            label="Overlay position"
            isLabelHidden
            value={position}
            options={[
              { value: "top", label: "Top" },
              { value: "bottom", label: "Bottom" },
            ]}
            onChange={setPosition}
          />
        </SettingsRow>
        <SettingsRow
          label="App data"
          help={<PathText>~/Library/Application Support/dictate</PathText>}
        >
          <Switch label="Unavailable" isLabelHidden value isDisabled />
        </SettingsRow>
      </SettingsGroup>
    );
  },
  play: async ({ canvasElement }) => {
    const toggle = within(canvasElement).getByRole("switch", {
      name: "Launch at login",
    });
    await userEvent.click(toggle);
    await expect(toggle).toBeChecked();
  },
};
