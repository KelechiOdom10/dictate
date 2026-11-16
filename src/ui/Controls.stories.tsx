import { Mic } from "lucide-react";
import { Button, IconButton } from "./Button";
import { ShortcutRecorder } from "./ShortcutRecorder";
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
export const WrappedSegments: Story = {
  render: () => {
    const [value, setValue] = useState("all");
    return (
      <div style={{ width: 180 }}>
        <SegmentedControl
          label="Wrapped view"
          options={options}
          value={value}
          onChange={setValue}
        />
      </div>
    );
  },
  play: async ({ canvasElement }) => {
    const c = within(canvasElement);
    await userEvent.click(c.getByRole("radio", { name: "Meeting" }));
    const selected = c.getByRole("radio", { name: "Meeting" }).parentElement!;
    const group = selected.parentElement!;
    await expect(selected.offsetTop).toBeGreaterThan(3);
    await expect(group.style.getPropertyValue("--thumb-y")).toBe(
      `${selected.offsetTop}px`,
    );
    await expect(group.style.getPropertyValue("--thumb-x")).toBe(
      `${selected.offsetLeft}px`,
    );
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

export const ControlsSideBySide: Story = {
  name: "Controls side by side",
  render: () => {
    const [query, setQuery] = useState("");
    return (
      <div className="control-comparison">
        <h1>Controls side by side</h1>
        <p className="ui-preview">
          One scale · Geist · height / text / icon / gap
        </p>
        {(["sm", "md", "lg"] as const).map((size) => (
          <section key={size}>
            <h2>
              {size} ·{" "}
              {size === "sm"
                ? "28 / 12 / 14 / 6"
                : size === "md"
                  ? "32 / 13 / 16 / 8"
                  : "40 / 14 / 18 / 10"}{" "}
              px
            </h2>
            <div className="story-row">
              <Button size={size}>Test microphone</Button>
              <IconButton size={size} label={`${size} microphone`}>
                <Mic />
              </IconButton>
              <Select size={size} aria-label={`${size} mode`}>
                <option>Hold to talk</option>
                <option>Toggle</option>
              </Select>
              <SegmentedControl
                size={size}
                label={`${size} view`}
                options={options.slice(0, 2)}
                value="all"
                onChange={() => {}}
              />
              <ShortcutRecorder
                size={size}
                value="Alt+Shift+Space"
                recording={false}
                onStart={() => {}}
                onCancel={() => {}}
                onCommit={() => {}}
                onError={() => {}}
              />
              <SearchField
                size={size}
                label={`${size} search`}
                value={query}
                onChange={setQuery}
              />
              <Chip size={size}>Starred</Chip>
              <Toggle size={size} aria-label={`${size} launch at login off`} />
              <Toggle
                size={size}
                checked
                readOnly
                aria-label={`${size} launch at login on`}
              />
            </div>
          </section>
        ))}
      </div>
    );
  },
  play: async ({ canvasElement }) => {
    for (const [size, height, font, padding, gap] of [
      ["sm", 28, 12, 12, 6],
      ["md", 32, 13, 14, 8],
      ["lg", 40, 14, 18, 10],
    ] as const) {
      const section = canvasElement
        .querySelector(`.ui-button.ui-size-${size}`)
        ?.closest("section");
      if (!section) throw new Error(`Missing ${size} controls`);
      for (const control of section.querySelectorAll(
        "button:not(.ui-search button), select, .ui-search, .ui-segments, .ui-toggle",
      )) {
        await expect(control.getBoundingClientRect().height).toBe(height);
        if (!control.classList.contains("ui-toggle"))
          await expect(getComputedStyle(control).fontSize).toBe(`${font}px`);
      }
      for (const control of section.querySelectorAll(
        ".ui-button:not(.ui-icon-button):not(.ui-shortcut), select, .ui-search",
      )) {
        await expect(getComputedStyle(control).paddingInlineStart).toBe(
          `${padding}px`,
        );
      }
      for (const control of section.querySelectorAll(
        ".ui-button:not(.ui-icon-button):not(.ui-shortcut), .ui-search, .ui-segments",
      )) {
        await expect(getComputedStyle(control).gap).toBe(
          control.classList.contains("ui-segments") ? "2px" : `${gap}px`,
        );
      }
    }
  },
};
