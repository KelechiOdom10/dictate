import { useState } from "react";
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { EventCallback } from "@tauri-apps/api/event";
import { expect, waitFor, within } from "storybook/test";
import { inkHandlers } from "./AnnotateWindows";
import type { InkStroke } from "./ui";

type Sent = { event: string; payload: unknown };

/** Feeds ink events to the overlay's handlers and prints the strokes. */
function InkEvents({ events }: { events: Sent[] }) {
  const [strokes, setStrokes] = useState<InkStroke[]>([]);
  const [sent, setSent] = useState(false);
  const [handlers] = useState(() => inkHandlers(setStrokes, () => 0));
  if (!sent) {
    setSent(true);
    queueMicrotask(() => {
      for (const { event, payload } of events)
        (handlers[event] as EventCallback<unknown>)({
          event: event as never,
          id: 0,
          payload,
        });
    });
  }
  return (
    <ul aria-label="Strokes">
      {strokes.map((stroke) => (
        <li key={stroke.id}>
          {stroke.id}: {stroke.points.map(([x]) => x).join(" ")}
        </li>
      ))}
    </ul>
  );
}

const meta = {
  title: "Annotate/Ink events",
  component: InkEvents,
} satisfies Meta<typeof InkEvents>;
export default meta;
type Story = StoryObj<typeof meta>;

const point = (id: number, seq: number, x: number) => ({
  event: "ink-point",
  payload: { id, seq, x, y: 0.5 },
});

export const OutOfOrder: Story = {
  name: "A late press point goes first",
  args: {
    // The poll's samples (seq 2, 3) beat the press point (seq 1) here.
    events: [point(1, 2, 0.2), point(1, 3, 0.3), point(1, 1, 0.1)],
  },
  play: async ({ canvasElement }) => {
    const list = within(canvasElement).getByRole("list", { name: "Strokes" });
    await waitFor(() => expect(list).toHaveTextContent("1: 0.1 0.2 0.3"));
  },
};

export const LateAfterDiscard: Story = {
  name: "A point after a discard does not bring the stroke back",
  args: {
    events: [
      point(1, 1, 0.1),
      { event: "ink-discard", payload: 1 },
      point(1, 2, 0.2),
      point(2, 3, 0.7),
    ],
  },
  play: async ({ canvasElement }) => {
    const list = within(canvasElement).getByRole("list", { name: "Strokes" });
    await waitFor(() => expect(list).toHaveTextContent("2: 0.7"));
    await expect(list).not.toHaveTextContent("1:");
  },
};
