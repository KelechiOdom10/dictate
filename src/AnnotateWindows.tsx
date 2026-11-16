import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type EventCallback } from "@tauri-apps/api/event";
import { z } from "zod";
import { AnnotateBar, InkCanvas, inkOpacity, type InkStroke } from "./ui";

const pointSchema = z.object({
  id: z.number().int(),
  x: z.number().min(0).max(1),
  y: z.number().min(0).max(1),
});
const idSchema = z.number().int();
const barSchema = z.object({
  recording: z.boolean(),
  startedAtMs: z.number(),
  pen: z.boolean(),
});
type BarState = z.infer<typeof barSchema>;

function useEvents(handlers: Record<string, EventCallback<unknown>>) {
  useEffect(() => {
    const subscriptions = Object.entries(handlers).map(([event, handler]) =>
      listen(event, handler),
    );
    return () => {
      for (const subscription of subscriptions)
        void subscription.then((stop) => stop()).catch(console.error);
    };
  }, [handlers]);
}

function useTransparentWindow() {
  useEffect(() => {
    const root = document.documentElement;
    root.classList.add("overlay-window");
    return () => root.classList.remove("overlay-window");
  }, []);
}

const inkHandlers = (
  update: (change: (strokes: InkStroke[]) => InkStroke[]) => void,
): Record<string, EventCallback<unknown>> => ({
  "ink-point": ({ payload }) => {
    const point = pointSchema.safeParse(payload);
    if (!point.success) return console.error(point.error);
    const { id, x, y } = point.data;
    update((strokes) => {
      const now = performance.now();
      const live = strokes.filter(
        (stroke) => inkOpacity(stroke.endedAt, now) > 0,
      );
      if (!live.some((stroke) => stroke.id === id))
        return [...live, { id, points: [[x, y]] }];
      return live.map((stroke) =>
        stroke.id === id
          ? { ...stroke, points: [...stroke.points, [x, y]] }
          : stroke,
      );
    });
  },
  "ink-end": ({ payload }) => {
    const id = idSchema.safeParse(payload);
    if (!id.success) return console.error(id.error);
    update((strokes) =>
      strokes.map((stroke) =>
        stroke.id === id.data
          ? { ...stroke, endedAt: performance.now() }
          : stroke,
      ),
    );
  },
  "ink-discard": ({ payload }) => {
    const id = idSchema.safeParse(payload);
    if (!id.success) return console.error(id.error);
    update((strokes) => strokes.filter((stroke) => stroke.id !== id.data));
  },
  "ink-clear": () => update(() => []),
});

/**
 * The ink panel. Pointer events only arrive in pen mode, because Rust keeps
 * the panel click-through otherwise. Rust reads the pointer itself, so the
 * page only says when a drag starts and ends.
 */
export function InkWindow() {
  useTransparentWindow();
  const [strokes, setStrokes] = useState<InkStroke[]>([]);
  const [handlers] = useState(() => inkHandlers(setStrokes));
  useEvents(handlers);
  const up = () => void invoke("annotate_pen_up").catch(console.error);
  return (
    <main
      className="ink-window"
      onPointerDown={() =>
        void invoke("annotate_pen_down").catch(console.error)
      }
      onPointerUp={up}
      onPointerCancel={up}
    >
      <InkCanvas strokes={strokes} />
    </main>
  );
}

export function AnnotateBarWindow() {
  useTransparentWindow();
  const [state, setState] = useState<BarState | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [handlers] = useState(() => ({
    "annotate-bar": (({ payload }) => {
      const result = barSchema.safeParse(payload);
      if (result.success) setState(result.data);
      else console.error(result.error);
    }) satisfies EventCallback<unknown>,
  }));
  useEvents(handlers);
  useEffect(() => {
    invoke("annotate_bar_state")
      .then((payload) => setState(barSchema.parse(payload)))
      .catch(console.error);
  }, []);
  useEffect(() => {
    if (!state?.recording) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(timer);
  }, [state?.recording, state?.startedAtMs]);
  if (!state?.recording) return null;
  return (
    <main className="annotate-bar-window">
      <AnnotateBar
        seconds={Math.max(0, (now - state.startedAtMs) / 1000)}
        pen={state.pen}
        onPen={() => void invoke("annotate_pen").catch(console.error)}
        onStop={() => void invoke("annotate_stop").catch(console.error)}
      />
    </main>
  );
}
