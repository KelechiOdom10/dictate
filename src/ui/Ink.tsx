import { useEffect, useRef } from "react";

/** Same red and width as the ink drawn into Annotate's key frames. */
export const INK_COLOR = "#FF3B30";
export const INK_WIDTH = 4;
export const INK_HOLD_MS = 1500;
export const INK_FADE_MS = 500;

/** Points are fractions of the display, as Rust stores them ÷ 10,000. */
export type InkStroke = {
  id: number;
  points: Array<readonly [number, number]>;
  endedAt?: number;
};

export function inkOpacity(endedAt: number | undefined, now: number) {
  if (endedAt === undefined) return 1;
  const fading = now - endedAt - INK_HOLD_MS;
  return Math.max(0, Math.min(1, 1 - fading / INK_FADE_MS));
}

/** Fading is display only: Rust keeps every stroke for the package. */
export function InkCanvas({
  strokes,
  now = () => performance.now(),
}: {
  strokes: InkStroke[];
  now?: () => number;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const latest = useRef(strokes);
  latest.current = strokes;
  const clock = useRef(now);
  clock.current = now;
  useEffect(() => {
    const node = canvas.current;
    const context = node?.getContext("2d");
    if (!node || !context) return;
    let frame = 0;
    const draw = () => {
      const ratio = devicePixelRatio;
      const { width, height } = node.getBoundingClientRect();
      if (
        node.width !== Math.round(width * ratio) ||
        node.height !== Math.round(height * ratio)
      ) {
        node.width = Math.round(width * ratio);
        node.height = Math.round(height * ratio);
      }
      context.setTransform(ratio, 0, 0, ratio, 0, 0);
      context.clearRect(0, 0, width, height);
      context.strokeStyle = INK_COLOR;
      context.fillStyle = INK_COLOR;
      context.lineWidth = INK_WIDTH;
      context.lineCap = "round";
      context.lineJoin = "round";
      const time = clock.current();
      let visible = false;
      for (const stroke of latest.current) {
        const opacity = inkOpacity(stroke.endedAt, time);
        if (opacity === 0 || stroke.points.length === 0) continue;
        visible = true;
        context.globalAlpha = opacity;
        const [[x0, y0], ...rest] = stroke.points;
        context.beginPath();
        if (rest.length === 0) {
          context.arc(x0 * width, y0 * height, INK_WIDTH / 2, 0, Math.PI * 2);
          context.fill();
          continue;
        }
        context.moveTo(x0 * width, y0 * height);
        for (const [x, y] of rest) context.lineTo(x * width, y * height);
        context.stroke();
      }
      if (visible) frame = requestAnimationFrame(draw);
    };
    draw();
    return () => cancelAnimationFrame(frame);
  }, [strokes]);
  return <canvas ref={canvas} className="ui-ink" aria-hidden="true" />;
}
