//! Annotate ink: the rules for drawing while a session records.
//!
//! Two ways to draw, and neither takes focus from the app being annotated:
//!
//! - **Hold to draw.** Hold the Annotate shortcut and move the pointer. The
//!   overlay stays click-through; a 120 Hz poll of the global pointer feeds
//!   the open stroke. Letting go quickly without moving counts as a tap, which
//!   stops the session instead.
//! - **Pen mode.** The pen on the floating bar makes the overlay take the
//!   mouse, so a drag draws. Esc, the pen again, or a timeout backstop end it.
//!
//! Only one of the two records at a time. A press of the shortcut while a
//! drag is open (or a drag that starts while the shortcut is held) would
//! otherwise follow the same pointer into two strokes, so the held stroke is
//! *shadowed*: it stops recording, still decides tap-or-hold, and is dropped
//! on release.
//!
//! Every point handed to the overlay carries a sequence number, because the
//! press and the pointer poll emit from different threads and can arrive out
//! of order. The overlay draws each stroke in sequence order.
//!
//! Everything here is pure: the shell in `annotate_ui.rs` reads the pointer
//! and the session clock and passes them in. Times are ms on the session
//! clock, positions are global display points (as `CGEvent::location` gives
//! them), and finished strokes are normalised to the recorded display.

use std::time::Duration;

use crate::package::{Display, Rect, Stroke};
use crate::settings::OverlayPosition;

/// A press shorter than this, that also moved less than `TAP_MAX_MOVE_PT`, is
/// a tap. Tune both from the hands-on R2 run (`docs/spike-results.md`).
pub const TAP_MAX_MS: u64 = 250;
/// Furthest the pointer may get from where the press started and still count
/// as a tap. Measured along the whole path, not only at release, so a quick
/// scribble that ends where it began is still a stroke.
pub const TAP_MAX_MOVE_PT: f64 = 6.0;
/// About 120 Hz, the rate the reference app samples at.
pub const POLL_INTERVAL: Duration = Duration::from_micros(8_333);
/// Pen mode turns itself off after this long. A stuck overlay that blocks
/// clicks on the whole screen is the worst failure Annotate can have.
pub const PEN_TIMEOUT_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Press {
    Tap,
    Hold,
}

pub fn classify(held_ms: u64, max_move_pt: f64) -> Press {
    if held_ms < TAP_MAX_MS && max_move_pt < TAP_MAX_MOVE_PT {
        Press::Tap
    } else {
        Press::Hold
    }
}

/// A stroke being drawn: global points with session times.
#[derive(Clone, Debug, PartialEq)]
struct OpenStroke {
    id: u64,
    t0: u64,
    start: (f64, f64),
    max_move: f64,
    points: Vec<(f64, f64, u64)>,
    /// A held stroke that a pen drag overlapped: it records nothing more and
    /// is dropped on release, so one pointer path never becomes two strokes.
    shadowed: bool,
}

impl OpenStroke {
    fn new(id: u64, t: u64, x: f64, y: f64) -> Self {
        Self {
            id,
            t0: t,
            start: (x, y),
            max_move: 0.0,
            points: vec![(x, y, t)],
            shadowed: false,
        }
    }

    /// Adds a sample. Returns false when the pointer has not moved, so a still
    /// pointer does not fill the stroke with copies.
    fn push(&mut self, x: f64, y: f64, t: u64) -> bool {
        self.max_move = self
            .max_move
            .max((x - self.start.0).hypot(y - self.start.1));
        if self.shadowed
            || self
                .points
                .last()
                .is_some_and(|&(lx, ly, _)| lx == x && ly == y)
        {
            return false;
        }
        self.points.push((x, y, t.max(self.t0)));
        true
    }

    fn finish(self, display: &Display, t1: u64) -> Stroke {
        let t1 = t1.max(self.t0);
        let mut pts = Vec::with_capacity(self.points.len() * 3);
        for (x, y, t) in self.points {
            let (px, py) = display.stroke_point(x, y);
            pts.extend([px, py, (t - self.t0) as u32]);
        }
        Stroke {
            t0: self.t0,
            t1,
            pts,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pen {
    #[default]
    Off,
    On {
        since_ms: u64,
    },
}

impl Pen {
    /// The overlay lets clicks through to the app below unless pen mode is on.
    pub fn click_through(self) -> bool {
        self == Pen::Off
    }

    /// Esc is a global shortcut only while pen mode is on, so it never steals
    /// Esc from other apps the rest of the time.
    pub fn wants_escape(self) -> bool {
        !self.click_through()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PenInput {
    /// The pen button on the floating bar.
    Button {
        now_ms: u64,
    },
    Escape,
    /// The backstop's clock.
    Tick {
        now_ms: u64,
    },
    SessionEnded,
}

pub fn pen(pen: Pen, input: PenInput) -> Pen {
    match (pen, input) {
        (Pen::Off, PenInput::Button { now_ms }) => Pen::On { since_ms: now_ms },
        (Pen::On { .. }, PenInput::Button { .. } | PenInput::Escape | PenInput::SessionEnded) => {
            Pen::Off
        }
        (Pen::On { since_ms }, PenInput::Tick { now_ms })
            if now_ms.saturating_sub(since_ms) >= PEN_TIMEOUT_MS =>
        {
            Pen::Off
        }
        (pen, _) => pen,
    }
}

/// What the overlay should draw: a point added to stroke `id`, at fractions of
/// the display. These are the stored numbers ÷ 10,000, so what is on screen is
/// what lands in `annotation.json`.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub struct InkPoint {
    pub id: u64,
    /// Order across every point of the session. The overlay sorts by it.
    pub seq: u64,
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    /// No stroke was open: the release that follows the start tap.
    Nothing,
    /// A tap: the provisional stroke `id` is dropped and the session stops.
    Tap { id: u64 },
    /// A hold: stroke `id` is kept.
    Kept { id: u64 },
    /// A hold that a pen drag overlapped: the drag has the path, so stroke
    /// `id` is dropped and the session goes on.
    Shadowed { id: u64 },
}

/// Ink for one session.
#[derive(Clone, Debug)]
pub struct Ink {
    display: Display,
    held: Option<OpenStroke>,
    dragged: Option<OpenStroke>,
    pen: Pen,
    strokes: Vec<Stroke>,
    next_id: u64,
    next_seq: u64,
}

impl Ink {
    pub fn new(display: Display) -> Self {
        Self {
            display,
            held: None,
            dragged: None,
            pen: Pen::Off,
            strokes: Vec::new(),
            next_id: 0,
            next_seq: 0,
        }
    }

    pub fn pen(&self) -> Pen {
        self.pen
    }

    pub fn drawing(&self) -> bool {
        self.held.is_some() || self.dragged.is_some()
    }

    fn open(&mut self, t: u64, x: f64, y: f64) -> OpenStroke {
        self.next_id += 1;
        OpenStroke::new(self.next_id, t, x, y)
    }

    fn point(&mut self, id: u64, x: f64, y: f64) -> InkPoint {
        let (px, py) = self.display.stroke_point(x, y);
        self.next_seq += 1;
        InkPoint {
            id,
            seq: self.next_seq,
            x: f64::from(px) / crate::package::POINT_SCALE,
            y: f64::from(py) / crate::package::POINT_SCALE,
        }
    }

    /// The shortcut went down. Opens a provisional stroke, unless one is open
    /// already: key auto-repeat must not start a second stroke. During a pen
    /// drag the stroke opens shadowed and draws nothing, since the drag is
    /// already recording the pointer.
    pub fn press(&mut self, t: u64, x: f64, y: f64) -> Option<InkPoint> {
        if self.held.is_some() {
            return None;
        }
        let mut stroke = self.open(t, x, y);
        stroke.shadowed = self.dragged.is_some();
        let point = (!stroke.shadowed).then(|| self.point(stroke.id, x, y));
        self.held = Some(stroke);
        point
    }

    /// The shortcut came up: keep the stroke, or drop it because it was a tap.
    pub fn release(&mut self, t: u64) -> Release {
        let Some(stroke) = self.held.take() else {
            return Release::Nothing;
        };
        let id = stroke.id;
        match classify(t.saturating_sub(stroke.t0), stroke.max_move) {
            Press::Tap => Release::Tap { id },
            Press::Hold if stroke.shadowed => Release::Shadowed { id },
            Press::Hold => {
                let stroke = stroke.finish(&self.display, t);
                self.strokes.push(stroke);
                Release::Kept { id }
            }
        }
    }

    /// The mouse went down on the overlay. Only draws in pen mode.
    pub fn pen_down(&mut self, t: u64, x: f64, y: f64) -> Option<InkPoint> {
        if self.pen.click_through() || self.dragged.is_some() {
            return None;
        }
        if let Some(held) = self.held.as_mut() {
            held.shadowed = true;
        }
        let stroke = self.open(t, x, y);
        let point = self.point(stroke.id, x, y);
        self.dragged = Some(stroke);
        Some(point)
    }

    /// The mouse came up, or pen mode ended mid-drag. Every drag is kept.
    pub fn pen_up(&mut self, t: u64) -> Option<u64> {
        let stroke = self.dragged.take()?;
        let id = stroke.id;
        let stroke = stroke.finish(&self.display, t);
        self.strokes.push(stroke);
        Some(id)
    }

    /// One poll of the pointer. Returns the points to draw.
    pub fn sample(&mut self, t: u64, x: f64, y: f64) -> Vec<InkPoint> {
        let mut moved = Vec::new();
        for stroke in [&mut self.held, &mut self.dragged].into_iter().flatten() {
            if stroke.push(x, y, t) {
                moved.push(stroke.id);
            }
        }
        moved.into_iter().map(|id| self.point(id, x, y)).collect()
    }

    /// Moves pen mode. Leaving it closes a drag that is still open.
    pub fn set_pen(&mut self, input: PenInput, t: u64) -> (Pen, Option<u64>) {
        self.pen = pen(self.pen, input);
        let closed = if self.pen.click_through() {
            self.pen_up(t)
        } else {
            None
        };
        (self.pen, closed)
    }

    /// The session stopped: pen mode ends and every stroke comes out, in the
    /// order they were finished. A drag still open is kept; a held stroke is
    /// not, because the session only stops on a tap or the bar's stop button.
    pub fn finish(mut self, t: u64) -> Vec<Stroke> {
        self.set_pen(PenInput::SessionEnded, t);
        self.strokes
    }
}

/// The display that contains a global point; the caller chooses the fallback.
pub fn display_at(displays: &[Display], x: f64, y: f64) -> Option<&Display> {
    displays
        .iter()
        .find(|display| contains(&display.frame, x, y))
}

fn contains(frame: &Rect, x: f64, y: f64) -> bool {
    x >= frame.x && x < frame.x + frame.w && y >= frame.y && y < frame.y + frame.h
}

/// A frame in CG's global space (top-left origin, y down) as an AppKit window
/// frame (bottom-left origin of the primary display, y up). `primary_height`
/// is the height of the display at (0, 0).
pub fn cocoa_frame(frame: &Rect, primary_height: f64) -> Rect {
    Rect {
        x: frame.x,
        y: primary_height - (frame.y + frame.h),
        w: frame.w,
        h: frame.h,
    }
}

/// Where the floating bar sits: centred near the selected display edge.
pub const BAR_WIDTH: f64 = 168.0;
pub const BAR_HEIGHT: f64 = 44.0;
const BAR_MARGIN: f64 = 48.0;
/// Distance from the display edge to the bar's far side.
pub const BAR_CLEARANCE: f64 = BAR_MARGIN + BAR_HEIGHT;

pub fn bar_frame(display: &Rect, position: OverlayPosition) -> Rect {
    Rect {
        x: display.x + (display.w - BAR_WIDTH) / 2.0,
        y: match position {
            OverlayPosition::Top => display.y + BAR_MARGIN,
            OverlayPosition::Bottom => display.y + display.h - BAR_HEIGHT - BAR_MARGIN,
        },
        w: BAR_WIDTH,
        h: BAR_HEIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(x: f64, y: f64, w: f64, h: f64) -> Display {
        Display {
            id: 2,
            frame: Rect { x, y, w, h },
            scale: 2.0,
        }
    }

    fn main() -> Display {
        display(0.0, 0.0, 1000.0, 500.0)
    }

    #[test]
    fn tap_or_hold_follows_the_250_ms_and_6_pt_rule() {
        let cases = [
            (0, 0.0, Press::Tap),
            (249, 5.9, Press::Tap),
            (250, 0.0, Press::Hold),
            (100, 6.0, Press::Hold),
            (2_000, 0.0, Press::Hold),
        ];
        for (held_ms, moved, expected) in cases {
            assert_eq!(
                classify(held_ms, moved),
                expected,
                "{held_ms} ms, {moved} pt"
            );
        }
    }

    #[test]
    fn a_quick_press_without_movement_is_a_tap_and_keeps_nothing() {
        let mut ink = Ink::new(main());
        let first = ink.press(1_000, 100.0, 100.0).unwrap();
        ink.sample(1_008, 102.0, 101.0);
        assert_eq!(ink.release(1_100), Release::Tap { id: first.id });
        assert!(ink.finish(1_200).is_empty());
    }

    #[test]
    fn a_quick_scribble_that_returns_to_its_start_is_a_stroke() {
        // Net displacement is zero; the furthest point is 40 pt away.
        let mut ink = Ink::new(main());
        let first = ink.press(1_000, 100.0, 100.0).unwrap();
        ink.sample(1_050, 140.0, 100.0);
        ink.sample(1_100, 100.0, 100.0);
        assert_eq!(ink.release(1_150), Release::Kept { id: first.id });
        let strokes = ink.finish(2_000);
        assert_eq!(strokes.len(), 1);
        assert_eq!((strokes[0].t0, strokes[0].t1), (1_000, 1_150));
    }

    #[test]
    fn a_long_still_hold_is_a_stroke_not_a_tap() {
        let mut ink = Ink::new(main());
        let first = ink.press(0, 10.0, 10.0).unwrap();
        assert!(
            ink.sample(8, 10.0, 10.0).is_empty(),
            "still pointer adds no point"
        );
        assert_eq!(ink.release(400), Release::Kept { id: first.id });
    }

    #[test]
    fn auto_repeat_presses_do_not_open_a_second_stroke() {
        let mut ink = Ink::new(main());
        assert!(ink.press(0, 10.0, 10.0).is_some());
        assert!(ink.press(30, 50.0, 50.0).is_none());
        assert!(ink.press(60, 90.0, 90.0).is_none());
        ink.sample(500, 200.0, 200.0);
        ink.release(600);
        let strokes = ink.finish(700);
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].t0, 0);
    }

    #[test]
    fn a_release_with_no_open_stroke_does_nothing() {
        // The Released that follows the Pressed which started the session.
        let mut ink = Ink::new(main());
        assert_eq!(ink.release(80), Release::Nothing);
    }

    #[test]
    fn strokes_store_fractions_x10000_with_ms_since_the_stroke_began() {
        let mut ink = Ink::new(main());
        ink.press(4_956, 500.0, 250.0);
        let drawn = ink.sample(4_964, 750.0, 0.0);
        ink.sample(4_972, 1_200.0, -40.0);
        ink.release(5_826);
        let stroke = &ink.finish(6_000)[0];
        assert_eq!(stroke.t0, 4_956);
        assert_eq!(stroke.t1, 5_826);
        // Off-display points clamp to the edge.
        assert_eq!(
            stroke.pts,
            vec![5_000, 5_000, 0, 7_500, 0, 8, 10_000, 0, 16]
        );
        // The overlay draws the same numbers ÷ 10,000.
        assert_eq!(
            drawn,
            vec![InkPoint {
                id: 1,
                seq: 2,
                x: 0.75,
                y: 0.0
            }]
        );
    }

    #[test]
    fn a_second_display_at_a_non_zero_origin_normalises_to_its_own_frame() {
        // Left of and above the main display, as macOS lays out an external
        // monitor dragged to the top-left in Displays settings.
        let external = display(-1920.0, -1080.0, 1920.0, 1080.0);
        let mut ink = Ink::new(external);
        let first = ink.press(0, -1920.0, -1080.0).unwrap();
        assert_eq!((first.x, first.y), (0.0, 0.0));
        let centre = ink.sample(10, -960.0, -540.0);
        assert_eq!((centre[0].x, centre[0].y), (0.5, 0.5));
        ink.sample(20, -1.0, -1.0);
        ink.release(300);
        let stroke = &ink.finish(400)[0];
        assert_eq!(&stroke.pts[..6], &[0, 0, 0, 5_000, 5_000, 10]);
        assert_eq!(&stroke.pts[6..8], &[9_995, 9_991]);

        // To the right of the main display, below its top edge.
        let right = display(1512.0, 200.0, 2560.0, 1440.0);
        assert_eq!(
            right.stroke_point(1512.0 + 640.0, 200.0 + 1080.0),
            (2_500, 7_500)
        );
    }

    #[test]
    fn the_display_under_the_pointer_is_chosen() {
        let displays = [main(), display(1000.0, -200.0, 800.0, 600.0)];
        assert_eq!(display_at(&displays, 10.0, 10.0).unwrap().frame.x, 0.0);
        assert_eq!(
            display_at(&displays, 1500.0, -100.0).unwrap().frame.x,
            1000.0
        );
        // The right and bottom edges belong to the next display.
        assert_eq!(display_at(&displays, 1000.0, 0.0).unwrap().frame.x, 1000.0);
        assert!(display_at(&displays, -50.0, 900.0).is_none());
        assert!(display_at(&[], 0.0, 0.0).is_none());
    }

    #[test]
    fn cocoa_frames_flip_y_against_the_primary_display() {
        let primary = 982.0;
        assert_eq!(
            cocoa_frame(
                &Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 1512.0,
                    h: 982.0
                },
                primary
            ),
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1512.0,
                h: 982.0
            }
        );
        // An external display above the main one has a negative CG y and sits
        // above y = 982 in AppKit.
        assert_eq!(
            cocoa_frame(
                &Rect {
                    x: -400.0,
                    y: -1080.0,
                    w: 1920.0,
                    h: 1080.0
                },
                primary
            ),
            Rect {
                x: -400.0,
                y: 982.0,
                w: 1920.0,
                h: 1080.0
            }
        );
        // One below the main display.
        assert_eq!(
            cocoa_frame(
                &Rect {
                    x: 0.0,
                    y: 982.0,
                    w: 1920.0,
                    h: 1080.0
                },
                primary
            )
            .y,
            -1080.0
        );
    }

    #[test]
    fn the_bar_sits_centred_near_the_bottom_of_the_recorded_display() {
        let frame = bar_frame(
            &Rect {
                x: -1920.0,
                y: -1080.0,
                w: 1920.0,
                h: 1080.0,
            },
            OverlayPosition::Bottom,
        );
        assert_eq!(frame.x, -1920.0 + (1920.0 - BAR_WIDTH) / 2.0);
        assert_eq!(frame.y + frame.h, -BAR_MARGIN);
    }

    #[test]
    fn the_bar_sits_centred_near_the_top_when_the_overlay_is_set_to_top() {
        let display = Rect {
            x: -1920.0,
            y: -1080.0,
            w: 1920.0,
            h: 1080.0,
        };
        let top = bar_frame(&display, OverlayPosition::Top);
        assert_eq!(top.x, -1920.0 + (1920.0 - BAR_WIDTH) / 2.0);
        assert_eq!(top.y, -1080.0 + BAR_MARGIN);
        assert_eq!((top.w, top.h), (BAR_WIDTH, BAR_HEIGHT));
        assert!(top.y < bar_frame(&display, OverlayPosition::Bottom).y);
    }

    #[test]
    fn pen_mode_toggles_with_the_button_and_ends_on_escape() {
        let on = pen(Pen::Off, PenInput::Button { now_ms: 5_000 });
        assert_eq!(on, Pen::On { since_ms: 5_000 });
        assert!(!on.click_through() && on.wants_escape());
        assert_eq!(pen(on, PenInput::Button { now_ms: 6_000 }), Pen::Off);
        assert_eq!(pen(on, PenInput::Escape), Pen::Off);
        assert_eq!(pen(on, PenInput::SessionEnded), Pen::Off);
        assert!(Pen::Off.click_through() && !Pen::Off.wants_escape());
    }

    #[test]
    fn pen_mode_times_out_so_the_overlay_never_stays_blocking() {
        let on = Pen::On { since_ms: 1_000 };
        assert_eq!(
            pen(
                on,
                PenInput::Tick {
                    now_ms: 1_000 + PEN_TIMEOUT_MS - 1
                }
            ),
            on
        );
        assert_eq!(
            pen(
                on,
                PenInput::Tick {
                    now_ms: 1_000 + PEN_TIMEOUT_MS
                }
            ),
            Pen::Off
        );
        // A clock that went backwards must not panic or keep the overlay.
        assert_eq!(pen(on, PenInput::Tick { now_ms: 0 }), on);
    }

    #[test]
    fn escape_and_ticks_do_nothing_when_pen_mode_is_off() {
        for input in [
            PenInput::Escape,
            PenInput::Tick { now_ms: u64::MAX },
            PenInput::SessionEnded,
        ] {
            assert_eq!(pen(Pen::Off, input), Pen::Off);
        }
    }

    #[test]
    fn drags_only_draw_in_pen_mode_and_leaving_pen_mode_keeps_an_open_drag() {
        let mut ink = Ink::new(main());
        assert!(
            ink.pen_down(0, 10.0, 10.0).is_none(),
            "click-through: no drag"
        );

        assert_eq!(
            ink.set_pen(PenInput::Button { now_ms: 100 }, 100).0,
            Pen::On { since_ms: 100 }
        );
        let first = ink.pen_down(200, 10.0, 10.0).unwrap();
        ink.sample(208, 20.0, 20.0);
        assert_eq!(ink.pen_up(300), Some(first.id));

        let second = ink.pen_down(400, 50.0, 50.0).unwrap();
        ink.sample(408, 60.0, 60.0);
        assert_eq!(
            ink.set_pen(PenInput::Escape, 500),
            (Pen::Off, Some(second.id))
        );
        assert!(ink.pen_down(600, 0.0, 0.0).is_none());

        let strokes = ink.finish(700);
        assert_eq!(strokes.len(), 2);
        assert_eq!((strokes[1].t0, strokes[1].t1), (400, 500));
    }

    #[test]
    fn a_short_drag_in_pen_mode_is_kept_even_though_it_looks_like_a_tap() {
        let mut ink = Ink::new(main());
        ink.set_pen(PenInput::Button { now_ms: 0 }, 0);
        ink.pen_down(10, 10.0, 10.0);
        ink.pen_up(20);
        assert_eq!(ink.finish(30).len(), 1);
    }

    #[test]
    fn finishing_the_session_keeps_an_open_drag_and_ends_pen_mode() {
        let mut ink = Ink::new(main());
        ink.set_pen(PenInput::Button { now_ms: 0 }, 0);
        ink.pen_down(10, 10.0, 10.0);
        ink.sample(18, 30.0, 30.0);
        assert!(ink.drawing());
        let strokes = ink.finish(900);
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].t1, 900);
    }

    #[test]
    fn holding_the_shortcut_during_a_pen_drag_records_the_path_once() {
        let mut ink = Ink::new(main());
        ink.set_pen(PenInput::Button { now_ms: 0 }, 0);
        let drag = ink.pen_down(100, 10.0, 10.0).unwrap();
        // The shortcut goes down mid-drag: nothing new is drawn.
        assert_eq!(ink.press(150, 20.0, 20.0), None);
        let drawn = ink.sample(158, 40.0, 40.0);
        assert_eq!(drawn.iter().map(|p| p.id).collect::<Vec<_>>(), [drag.id]);
        ink.sample(400, 80.0, 80.0);
        assert!(matches!(ink.release(450), Release::Shadowed { .. }));
        ink.pen_up(500);
        assert_eq!(ink.finish(600).len(), 1);
    }

    #[test]
    fn a_drag_that_starts_while_the_shortcut_is_held_takes_over_the_path() {
        let mut ink = Ink::new(main());
        ink.set_pen(PenInput::Button { now_ms: 0 }, 0);
        let held = ink.press(100, 10.0, 10.0).unwrap();
        ink.sample(108, 20.0, 20.0);
        let drag = ink.pen_down(150, 20.0, 20.0).unwrap();
        let drawn = ink.sample(158, 60.0, 60.0);
        assert_eq!(drawn.iter().map(|p| p.id).collect::<Vec<_>>(), [drag.id]);
        assert_eq!(ink.release(500), Release::Shadowed { id: held.id });
        ink.pen_up(520);
        let strokes = ink.finish(600);
        assert_eq!(strokes.len(), 1);
        assert_eq!(strokes[0].t0, 150);
    }

    #[test]
    fn a_quick_tap_during_a_pen_drag_still_stops_the_session() {
        let mut ink = Ink::new(main());
        ink.set_pen(PenInput::Button { now_ms: 0 }, 0);
        ink.pen_down(100, 10.0, 10.0);
        ink.press(200, 10.0, 10.0);
        assert!(matches!(ink.release(300), Release::Tap { .. }));
    }

    #[test]
    fn every_point_carries_a_rising_sequence_number() {
        let mut ink = Ink::new(main());
        let first = ink.press(0, 10.0, 10.0).unwrap();
        let next = ink.sample(8, 20.0, 20.0);
        let later = ink.sample(16, 30.0, 30.0);
        assert!(first.seq < next[0].seq && next[0].seq < later[0].seq);
    }
}
