import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, userEvent, within } from "storybook/test";
import {
  Toggle,
  Select,
  Chip,
  FilterChips,
  SegmentedControl,
  SearchField,
} from "./Controls";
const options = [
  { value: "all", label: "All" },
  { value: "starred", label: "Starred" },
  { value: "meeting", label: "Meeting" },
  { value: "shared", label: "Shared", disabled: true },
];
const meta = { title: "Components/Controls" } satisfies Meta;
export default meta;
type Story = StoryObj<typeof meta>;
export const ToggleControl: Story = {
  name: "Toggle / off, on, disabled",
  render: () => {
    const [checked, setChecked] = useState(false);
    return (
      <div className="story-stack">
        <label className="story-row">
          Launch at login
          <Toggle
            checked={checked}
            onChange={(e) => setChecked(e.target.checked)}
          />
        </label>
        <label className="story-row">
          Disabled off
          <Toggle disabled />
        </label>
        <label className="story-row">
          Disabled on
          <Toggle disabled checked readOnly />
        </label>
      </div>
    );
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const toggle = c.getByRole("switch", { name: "Launch at login" });
    await userEvent.click(toggle);
    await expect(toggle).toBeChecked();
    toggle.focus();
    await userEvent.keyboard(" ");
    await expect(toggle).not.toBeChecked();
  },
};
export const SelectControl: Story = {
  name: "Select / enabled and disabled",
  render: () => (
    <div className="story-stack">
      <label>
        Microphone
        <Select defaultValue="system">
          <option value="system">System default</option>
          <option>USB Microphone</option>
        </Select>
      </label>
      <label>
        Unavailable microphone
        <Select disabled defaultValue="missing">
          <option value="missing">USB Microphone (unavailable)</option>
        </Select>
      </label>
    </div>
  ),
};
export const Chips: Story = {
  name: "Chip / selected, unselected, disabled",
  render: () => (
    <div className="story-row">
      <Chip selected>All</Chip>
      <Chip>Starred</Chip>
      <Chip disabled>Shared</Chip>
      <Chip selected disabled>
        Archived
      </Chip>
    </div>
  ),
};
export const Filters: Story = {
  name: "FilterChips / selection",
  render: () => {
    const [value, setValue] = useState("all");
    return (
      <FilterChips
        label="Filter notes"
        options={options}
        value={value}
        onChange={setValue}
      />
    );
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const starred = c.getByRole("button", { name: "Starred" });
    starred.focus();
    await userEvent.keyboard("{Enter}");
    await expect(starred).toHaveAttribute("aria-pressed", "true");
    await expect(c.getByRole("button", { name: "All" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    await expect(c.getByRole("button", { name: "Shared" })).toBeDisabled();
  },
};
export const Segments: Story = {
  name: "SegmentedControl / selection and disabled",
  render: () => {
    const [value, setValue] = useState("all");
    return (
      <SegmentedControl
        label="View"
        options={options}
        value={value}
        onChange={setValue}
      />
    );
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    c.getByRole("radio", { name: "All" }).focus();
    await userEvent.keyboard("{ArrowRight}");
    await expect(c.getByRole("radio", { name: "Starred" })).toBeChecked();
  },
};
export const Search: Story = {
  name: "SearchField / empty, filled, disabled",
  render: () => {
    const [value, setValue] = useState("");
    return (
      <div className="story-stack">
        <SearchField label="Search history" value={value} onChange={setValue} />
        <SearchField
          label="Unavailable search"
          value=""
          onChange={() => {}}
          disabled
        />
      </div>
    );
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    const input = c.getByRole("searchbox", { name: "Search history" });
    await userEvent.type(input, "meeting");
    await expect(input).toHaveValue("meeting");
    await userEvent.click(
      c.getByRole("button", { name: "Clear search history" }),
    );
    await expect(input).toHaveValue("");
    await expect(input).toHaveFocus();
  },
};
