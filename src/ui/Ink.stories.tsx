import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect } from "storybook/test";
import {
  INK_FADE_MS,
  INK_HOLD_MS,
  InkCanvas,
  inkOpacity,
  type InkStroke,
} from "./Ink";
import { AnnotateBar } from "./AnnotateBar";

const NOW = 10_000;

function loop(
  id: number,
  cx: number,
  cy: number,
  rx: number,
  ry: number,
  endedAt?: number,
): InkStroke {
  const points = Array.from({ length: 49 }, (_, step) => {
    const angle = (step / 44) * Math.PI * 2 - Math.PI / 2;
    return [cx + Math.cos(angle) * rx, cy + Math.sin(angle) * ry] as const;
  });
  return { id, points, endedAt };
}

const strokes = [
  loop(1, 0.28, 0.32, 0.16, 0.18),
  loop(2, 0.7, 0.36, 0.12, 0.14, NOW - 200),
  loop(3, 0.5, 0.74, 0.22, 0.1, NOW - INK_HOLD_MS - INK_FADE_MS / 2),
];

function Screen() {
  return (
    <div className="story-screen" aria-hidden="true">
      <div className="story-screen-bar" />
      {Array.from({ length: 7 }, (_, row) => (
        <div
          key={row}
          className="story-screen-line"
          style={{ width: `${40 + ((row * 37) % 50)}%` }}
        />
      ))}
    </div>
  );
}

const meta = {
  title: "Components/Ink",
  component: InkCanvas,
  args: { strokes, now: () => NOW },
  decorators: [
    (Story) => (
      <div className="story-ink">
        <Screen />
        <div className="story-ink-layer">
          <Story />
        </div>
      </div>
    ),
  ],
} satisfies Meta<typeof InkCanvas>;
export default meta;
type Story = StoryObj<typeof meta>;

export const ThreeCircles: Story = {
  name: "Ink / drawing, holding, fading",
  play: async ({ canvasElement }) => {
    await expect(inkOpacity(undefined, NOW)).toBe(1);
    await expect(inkOpacity(NOW - INK_HOLD_MS, NOW)).toBe(1);
    await expect(
      inkOpacity(NOW - INK_HOLD_MS - INK_FADE_MS / 2, NOW),
    ).toBeCloseTo(0.5);
    await expect(inkOpacity(NOW - INK_HOLD_MS - INK_FADE_MS, NOW)).toBe(0);
    const canvas = canvasElement.querySelector("canvas");
    if (!canvas) throw new Error("No ink canvas");
    const context = canvas.getContext("2d");
    if (!context) throw new Error("No 2D context");
    const { width, height } = canvas;
    const pixel = (x: number, y: number) =>
      context.getImageData(Math.round(x * width), Math.round(y * height), 1, 1)
        .data;
    // The top of the first loop is solid ink red.
    const top = pixel(0.28, 0.32 - 0.18);
    await expect([top[0], top[1], top[2], top[3]]).toEqual([255, 59, 48, 255]);
    // The fading loop is about half opaque; the centre has no ink.
    await expect(pixel(0.5, 0.74 + 0.1)[3]).toBeGreaterThan(100);
    await expect(pixel(0.5, 0.74 + 0.1)[3]).toBeLessThan(160);
    await expect(pixel(0.28, 0.32)[3]).toBe(0);
  },
};

export const Faded: Story = {
  name: "Ink / fully faded",
  args: {
    strokes: [loop(1, 0.5, 0.5, 0.2, 0.2, NOW - INK_HOLD_MS - INK_FADE_MS)],
  },
  play: async ({ canvasElement }) => {
    const canvas = canvasElement.querySelector("canvas");
    const context = canvas?.getContext("2d");
    if (!canvas || !context) throw new Error("No ink canvas");
    const data = context.getImageData(0, 0, canvas.width, canvas.height).data;
    await expect(data.some((value, index) => index % 4 === 3 && value)).toBe(
      false,
    );
  },
};

export const WithBar: Story = {
  name: "Ink / with the floating bar",
  render: (args) => (
    <>
      <InkCanvas {...args} />
      <div className="story-ink-bar">
        <AnnotateBar seconds={23} pen onPen={() => {}} onStop={() => {}} />
      </div>
    </>
  ),
};
