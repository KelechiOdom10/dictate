import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { Plus } from "lucide-react";
import { Button, IconButton } from "./Button";
const meta = {
  title: "Components/Button",
  component: Button,
  args: { children: "Start dictating" },
} satisfies Meta<typeof Button>;
export default meta;
type Story = StoryObj<typeof meta>;
export const Primary: Story = { args: { variant: "primary" } };
export const Secondary: Story = {};
export const Ghost: Story = { args: { variant: "ghost" } };
export const Disabled: Story = { args: { disabled: true } };
export const Icon: Story = {
  render: () => (
    <IconButton label="Add note">
      <Plus size={16} aria-hidden="true" />
    </IconButton>
  ),
};
export const AllStates: Story = {
  render: () => (
    <div className="story-stack">
      {(["primary", "secondary", "ghost"] as const).map((variant) => (
        <div className="story-row" key={variant}>
          <Button variant={variant}>{variant}</Button>
          <Button variant={variant} disabled>
            {variant} disabled
          </Button>
        </div>
      ))}
      <IconButton label="Add note">
        <Plus size={16} aria-hidden="true" />
      </IconButton>
      <IconButton label="Add note unavailable" disabled>
        <Plus size={16} aria-hidden="true" />
      </IconButton>
    </div>
  ),
};

export const Focused: Story = {
  args: { variant: "primary" },
  play: async ({ canvasElement }) => {
    const button = within(canvasElement).getByRole("button");
    button.focus();
    await expect(button).toHaveFocus();
  },
};
