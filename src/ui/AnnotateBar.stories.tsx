import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, fn, userEvent, within } from "storybook/test";
import { AnnotateBar } from "./AnnotateBar";

const meta = {
  title: "Components/AnnotateBar",
  component: AnnotateBar,
  args: { seconds: 42, pen: false, onPen: fn(), onStop: fn() },
  decorators: [
    (Story) => (
      // The size of the floating bar's panel (`ink::BAR_WIDTH` × `BAR_HEIGHT`).
      <div className="story-annotate-bar" style={{ width: 168, height: 44 }}>
        <Story />
      </div>
    ),
  ],
} satisfies Meta<typeof AnnotateBar>;
export default meta;
type Story = StoryObj<typeof meta>;

export const Recording: Story = {
  play: async ({ canvasElement, args }) => {
    const bar = within(canvasElement);
    await expect(bar.getByRole("timer")).toHaveTextContent("00:42");
    const pen = bar.getByRole("button", { name: "Draw with the mouse" });
    await expect(pen).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(pen);
    await expect(args.onPen).toHaveBeenCalledOnce();
    await userEvent.click(bar.getByRole("button", { name: "Stop recording" }));
    await expect(args.onStop).toHaveBeenCalledOnce();
    const toolbar = bar.getByRole("toolbar", { name: "Annotate" });
    await expect(toolbar.scrollWidth).toBeLessThanOrEqual(toolbar.clientWidth);
  },
};

export const PenMode: Story = {
  args: { seconds: 75, pen: true },
  play: async ({ canvasElement }) => {
    const bar = within(canvasElement);
    await expect(bar.getByRole("timer")).toHaveTextContent("01:15");
    await expect(
      bar.getByRole("button", { name: "Stop drawing (Esc)" }),
    ).toHaveAttribute("aria-pressed", "true");
  },
};

export const LongSession: Story = {
  args: { seconds: 9 * 60 + 59 },
  play: async ({ canvasElement }) => {
    const toolbar = within(canvasElement).getByRole("toolbar");
    await expect(toolbar).toHaveTextContent("09:59");
    await expect(toolbar.scrollWidth).toBeLessThanOrEqual(toolbar.clientWidth);
  },
};

export const TogglePen: Story = {
  name: "Pen / toggles on click",
  render: (args) => {
    const [pen, setPen] = useState(false);
    return <AnnotateBar {...args} pen={pen} onPen={() => setPen(!pen)} />;
  },
  play: async ({ canvasElement }) => {
    const bar = within(canvasElement);
    await userEvent.click(
      bar.getByRole("button", { name: "Draw with the mouse" }),
    );
    await expect(
      bar.getByRole("button", { name: "Stop drawing (Esc)" }),
    ).toHaveAttribute("aria-pressed", "true");
  },
};
