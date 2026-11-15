//! Annotate package builder, pure core.
//!
//! Turns one finished session (strokes, timed words, metadata) into the
//! `annotation.json` and `summary.md` of the package folder. Everything here is
//! plain data in, plain data out; the only I/O is `write_package`, which writes
//! those two files. Frames, the contact sheet and the clipboard come later and
//! read the `Annotation` this module returns.
//!
//! All times are ms since the session started. All positions are fractions of
//! the recorded display, top-left origin.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::transcribe::{self, Segment, Word};

pub const SCHEMA: &str = "dictate.annotation/1";
pub const CONTACT_SHEET: &str = "contact-sheet.png";

/// A stroke joins the previous mark if it starts at most this long after the
/// mark's last stroke ended, and their boxes overlap or touch.
pub const MARK_JOIN_GAP_MS: u64 = 2_500;
/// A mark is a circle when its first and last points are this close, as a
/// fraction of its box's diagonal.
pub const CIRCLE_CLOSE_FRACTION: f64 = 0.15;
/// A mark's frame is taken this long before it ends: the ink is done, and the
/// screen has not moved on yet.
pub const FRAME_LEAD_MS: u64 = 100;
/// Marks that end at most this long after the first mark in a frame ends share
/// that frame. Measured from the first mark, so a run of marks cannot chain
/// into one long frame.
pub const MARK_MERGE_MS: u64 = 1_500;
/// Words that start up to this long after a frame still belong to it: people
/// draw first and then say what the mark means. Owner's call, 15 Nov 2026;
/// tune here.
pub const SAID_GRACE_MS: u64 = 1_500;
/// A speech segment gets its own frame when it ends more than this far from
/// every other frame.
pub const SPEECH_FRAME_GAP_MS: u64 = 4_000;
pub const MAX_FRAMES: usize = 12;

/// Stored point coordinates are fractions ×10,000.
pub const POINT_SCALE: f64 = 10_000.0;

/// What the recorder hands over when a session stops.
#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    /// RFC 3339, UTC.
    pub started_at: String,
    pub duration_ms: u64,
    pub display: Display,
    pub frontmost_app: Option<App>,
    pub video: Option<Video>,
    pub audio: Audio,
    pub transcript_model: String,
    /// On the audio clock, as `transcribe::transcribe_timed` returns them.
    /// `build` moves them to session time with `audio.offset_ms`.
    pub words: Vec<Word>,
    /// On the session clock already: the overlay stamps strokes with ms since
    /// the session started, so `build` does not shift them.
    pub strokes: Vec<Stroke>,
    pub missing_permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub schema: String,
    pub id: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub display: Display,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frontmost_app: Option<App>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video: Option<Video>,
    pub audio: Audio,
    pub transcript: Transcript,
    pub strokes: Vec<Stroke>,
    pub marks: Vec<Mark>,
    pub frames: Vec<Frame>,
    pub contact_sheet: String,
    pub missing_permissions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Display {
    pub id: u32,
    /// In points, in global display space.
    pub frame: Rect,
    pub scale: f64,
}

impl Display {
    /// A point in global display space (points, as `CGEvent::location` gives
    /// it) as a stored stroke point: fractions of this display ×10,000,
    /// top-left origin, clamped to the display. Works for displays whose
    /// origin is not (0, 0), such as one left of or above the main display.
    pub fn stroke_point(&self, global_x: f64, global_y: f64) -> (u32, u32) {
        let fraction = |value: f64, origin: f64, size: f64| {
            let f = if size > 0.0 {
                (value - origin) / size
            } else {
                0.0
            };
            (f.clamp(0.0, 1.0) * POINT_SCALE).round() as u32
        };
        (
            fraction(global_x, self.frame.x, self.frame.w),
            fraction(global_y, self.frame.y, self.frame.h),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub name: String,
    pub bundle_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Video {
    pub file: String,
    pub offset_ms: u64,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Audio {
    pub file: String,
    pub offset_ms: u64,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transcript {
    pub model: String,
    pub text: String,
    pub segments: Vec<TimedText>,
    pub words: Vec<TimedText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimedText {
    pub t0: u64,
    pub t1: u64,
    pub text: String,
}

/// One pen-down to pen-up. `pts` is flat `[x, y, dt]` triples: x and y are
/// fractions of the display ×10,000, dt is ms since `t0`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stroke {
    pub t0: u64,
    pub t1: u64,
    pub pts: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mark {
    pub id: String,
    /// Indexes into `Annotation::strokes`.
    pub strokes: Vec<usize>,
    pub t0: u64,
    pub t1: u64,
    /// `[x0, y0, x1, y1]` as fractions of the display.
    #[serde(rename = "box")]
    pub bounds: [f64; 4],
    pub shape: Shape,
    pub description: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Circle,
    Stroke,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// 1-based, in time order.
    pub n: usize,
    pub t: u64,
    pub reason: FrameReason,
    /// Ids of the marks this frame shows. The frame image draws the ink of
    /// every stroke in these marks, whenever it was drawn, and no other ink:
    /// a frame merged to fit `MAX_FRAMES` still shows each mark it lists.
    /// Empty for a speech frame, which has no ink.
    pub marks: Vec<String>,
    pub file: String,
    pub full: String,
    /// The words that started after the previous frame's cut-off, up to this
    /// frame's time + `SAID_GRACE_MS` (or the next frame's time, if sooner).
    pub said: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameReason {
    Mark,
    Speech,
}

pub fn build(session: Session) -> Annotation {
    let words: Vec<Word> = session
        .words
        .into_iter()
        .map(|word| Word {
            start_ms: word.start_ms + session.audio.offset_ms,
            end_ms: word.end_ms + session.audio.offset_ms,
            text: word.text,
        })
        .collect();
    let segments = transcribe::segments(&words);
    let marks = group_marks(&session.strokes);
    let frames = choose_frames(&marks, &segments, &words, session.duration_ms);
    let text = segments
        .iter()
        .map(|segment| segment.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    Annotation {
        schema: SCHEMA.to_string(),
        id: session.id,
        started_at: session.started_at,
        duration_ms: session.duration_ms,
        display: session.display,
        frontmost_app: session.frontmost_app,
        video: session.video,
        audio: session.audio,
        transcript: Transcript {
            model: session.transcript_model,
            text,
            segments: segments
                .into_iter()
                .map(|s| TimedText {
                    t0: s.start_ms,
                    t1: s.end_ms,
                    text: s.text,
                })
                .collect(),
            words: words
                .into_iter()
                .map(|w| TimedText {
                    t0: w.start_ms,
                    t1: w.end_ms,
                    text: w.text,
                })
                .collect(),
        },
        strokes: session.strokes,
        marks,
        frames,
        contact_sheet: CONTACT_SHEET.to_string(),
        missing_permissions: session.missing_permissions,
    }
}

/// Groups strokes into marks, in the order they were drawn. Strokes with no
/// points are skipped.
pub fn group_marks(strokes: &[Stroke]) -> Vec<Mark> {
    let mut order: Vec<usize> = (0..strokes.len()).collect();
    order.sort_by_key(|&index| strokes[index].t0);

    let mut groups: Vec<StrokeGroup> = Vec::new();
    for index in order {
        let stroke = &strokes[index];
        let Some(bounds) = Bounds::of(stroke) else {
            continue;
        };
        if let Some(group) = groups.last_mut()
            && stroke.t0 <= group.t1 + MARK_JOIN_GAP_MS
            && group.bounds.touches(&bounds)
        {
            group.strokes.push(index);
            group.t1 = group.t1.max(stroke.t1);
            group.bounds = group.bounds.union(&bounds);
            continue;
        }
        groups.push(StrokeGroup {
            strokes: vec![index],
            t0: stroke.t0,
            t1: stroke.t1,
            bounds,
        });
    }

    groups
        .into_iter()
        .enumerate()
        .map(|(position, group)| group.into_mark(position + 1, strokes))
        .collect()
}

/// Picks the key frames, numbers them in time order and fills in `said`.
pub fn choose_frames(
    marks: &[Mark],
    segments: &[Segment],
    words: &[Word],
    duration_ms: u64,
) -> Vec<Frame> {
    let mut candidates = mark_frames(marks);
    for segment in segments {
        let t = segment.end_ms.min(duration_ms);
        if candidates
            .iter()
            .all(|c| c.t.abs_diff(t) > SPEECH_FRAME_GAP_MS)
        {
            candidates.push(Candidate {
                t,
                reason: FrameReason::Speech,
                marks: Vec::new(),
            });
        }
    }
    drop_speech_frames_over_limit(&mut candidates);
    candidates.sort_by_key(|c| c.t);

    let cut_offs: Vec<u64> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let grace_end = c.t + SAID_GRACE_MS;
            candidates
                .get(i + 1)
                .map_or(grace_end, |next| grace_end.min(next.t))
        })
        .collect();
    let mut said: Vec<Vec<&str>> = vec![Vec::new(); candidates.len()];
    for word in words {
        let index = cut_offs
            .iter()
            .position(|&cut_off| word.start_ms <= cut_off)
            .unwrap_or(candidates.len().saturating_sub(1));
        if let Some(bucket) = said.get_mut(index) {
            bucket.push(&word.text);
        }
    }

    candidates
        .into_iter()
        .zip(said)
        .enumerate()
        .map(|(position, (candidate, words))| Frame {
            n: position + 1,
            t: candidate.t,
            reason: candidate.reason,
            marks: candidate.marks,
            file: format!("frames/frame-{:02}.png", position + 1),
            full: format!("frames-full/frame-{:02}.png", position + 1),
            said: words.join(" "),
        })
        .collect()
}

/// The pasted text. It names the folder by absolute path so an agent that can
/// read files opens the images.
pub fn summary(annotation: &Annotation, folder: &Path) -> String {
    let mut header = vec![format!("annotation {}", clock(annotation.duration_ms))];
    if let Some(app) = &annotation.frontmost_app {
        header.push(app.name.clone());
    }
    header.push(folder.display().to_string());

    let mut out = format!("[{}]\n", header.join(" · "));
    out.push_str(&format!(
        "Open {CONTACT_SHEET} and frames/ in that folder first: the marks only make sense on the images.\n"
    ));
    out.push_str(
        "Positions are fractions of the screen, x left→right, y top→bottom. Full data: annotation.json.\n",
    );
    for frame in &annotation.frames {
        let marks: Vec<&str> = frame
            .marks
            .iter()
            .filter_map(|id| annotation.marks.iter().find(|mark| &mark.id == id))
            .map(|mark| mark.description.as_str())
            .collect();
        let mut parts: Vec<String> = Vec::new();
        if !marks.is_empty() {
            parts.push(marks.join("; "));
        }
        if !frame.said.is_empty() {
            parts.push(format!("“{}”", frame.said));
        }
        let mut line = format!("- #{} ({})", frame.n, clock(frame.t));
        if !parts.is_empty() {
            line.push(' ');
            line.push_str(&parts.join(" — "));
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Writes `annotation.json` and `summary.md` into `folder`, creating it if
/// needed. Returns the summary text, which is also what gets copied.
pub fn write_package(folder: &Path, annotation: &Annotation) -> io::Result<String> {
    std::fs::create_dir_all(folder)?;
    let mut json = serde_json::to_string_pretty(annotation).map_err(io::Error::other)?;
    json.push('\n');
    std::fs::write(folder.join("annotation.json"), json)?;
    let summary = summary(annotation, folder);
    std::fs::write(folder.join("summary.md"), &summary)?;
    Ok(summary)
}

struct Candidate {
    t: u64,
    reason: FrameReason,
    marks: Vec<String>,
}

/// One frame per mark, merging marks that end within `MARK_MERGE_MS` of the
/// first mark in the frame. If that still leaves more than `MAX_FRAMES`, the
/// two closest neighbouring frames are merged until it fits: a mark is never
/// dropped. A merged frame takes the later time and lists both frames' marks,
/// and its image draws the ink of every mark it lists (see `Frame::marks`).
fn mark_frames(marks: &[Mark]) -> Vec<Candidate> {
    let mut by_end: Vec<&Mark> = marks.iter().collect();
    by_end.sort_by_key(|mark| mark.t1);

    let mut frames: Vec<Candidate> = Vec::new();
    let mut frame_first_end: Option<u64> = None;
    for mark in by_end {
        let t = mark.t1.saturating_sub(FRAME_LEAD_MS).max(mark.t0);
        if let Some(first_end) = frame_first_end
            && mark.t1 - first_end <= MARK_MERGE_MS
            && let Some(frame) = frames.last_mut()
        {
            frame.t = t;
            frame.marks.push(mark.id.clone());
        } else {
            frames.push(Candidate {
                t,
                reason: FrameReason::Mark,
                marks: vec![mark.id.clone()],
            });
            frame_first_end = Some(mark.t1);
        }
    }

    while frames.len() > MAX_FRAMES {
        let closest = (1..frames.len())
            .min_by_key(|&i| frames[i].t - frames[i - 1].t)
            .expect("more than MAX_FRAMES frames");
        let later = frames.remove(closest);
        let earlier = &mut frames[closest - 1];
        earlier.t = later.t;
        earlier.marks.extend(later.marks);
    }
    frames
}

/// Removes speech frames until at most `MAX_FRAMES` remain, starting with the
/// one closest to another frame, since it adds the least new screen.
fn drop_speech_frames_over_limit(candidates: &mut Vec<Candidate>) {
    while candidates.len() > MAX_FRAMES {
        let nearest_neighbour = |i: usize| {
            candidates
                .iter()
                .enumerate()
                .filter(|&(j, _)| j != i)
                .map(|(_, other)| other.t.abs_diff(candidates[i].t))
                .min()
                .unwrap_or(u64::MAX)
        };
        let Some(most_redundant) = (0..candidates.len())
            .filter(|&i| candidates[i].reason == FrameReason::Speech)
            .min_by_key(|&i| (nearest_neighbour(i), std::cmp::Reverse(candidates[i].t)))
        else {
            return;
        };
        candidates.remove(most_redundant);
    }
}

struct StrokeGroup {
    strokes: Vec<usize>,
    t0: u64,
    t1: u64,
    bounds: Bounds,
}

impl StrokeGroup {
    fn into_mark(self, number: usize, strokes: &[Stroke]) -> Mark {
        let first = points(&strokes[self.strokes[0]]).next();
        let last = points(&strokes[*self.strokes.last().expect("a group has a stroke")]).last();
        let diagonal = (self.bounds.x1 - self.bounds.x0).hypot(self.bounds.y1 - self.bounds.y0);
        let closed = match (first, last) {
            (Some((x0, y0)), Some((x1, y1))) => {
                diagonal > 0.0 && (x1 - x0).hypot(y1 - y0) <= CIRCLE_CLOSE_FRACTION * diagonal
            }
            _ => false,
        };
        let shape = if closed { Shape::Circle } else { Shape::Stroke };
        let bounds = [
            self.bounds.x0,
            self.bounds.y0,
            self.bounds.x1,
            self.bounds.y1,
        ]
        .map(|v| (v * POINT_SCALE).round() / POINT_SCALE);
        let verb = match shape {
            Shape::Circle => "circled region",
            Shape::Stroke => "stroke across",
        };
        let mut description = format!(
            "{verb} x {:.2}–{:.2}, y {:.2}–{:.2}",
            bounds[0], bounds[2], bounds[1], bounds[3]
        );
        if self.strokes.len() > 1 {
            description.push_str(&format!(" ({} strokes)", self.strokes.len()));
        }
        Mark {
            id: format!("m{number}"),
            strokes: self.strokes,
            t0: self.t0,
            t1: self.t1,
            bounds,
            shape,
            description,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Bounds {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Bounds {
    fn of(stroke: &Stroke) -> Option<Self> {
        points(stroke).fold(None, |bounds: Option<Bounds>, (x, y)| {
            Some(match bounds {
                None => Bounds {
                    x0: x,
                    y0: y,
                    x1: x,
                    y1: y,
                },
                Some(b) => Bounds {
                    x0: b.x0.min(x),
                    y0: b.y0.min(y),
                    x1: b.x1.max(x),
                    y1: b.y1.max(y),
                },
            })
        })
    }

    fn touches(&self, other: &Bounds) -> bool {
        self.x0 <= other.x1 && other.x0 <= self.x1 && self.y0 <= other.y1 && other.y0 <= self.y1
    }

    fn union(&self, other: &Bounds) -> Bounds {
        Bounds {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }
}

fn points(stroke: &Stroke) -> impl Iterator<Item = (f64, f64)> + '_ {
    stroke
        .pts
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&[x, y, _dt]| (f64::from(x) / POINT_SCALE, f64::from(y) / POINT_SCALE))
}

fn clock(ms: u64) -> String {
    let seconds = ms / 1_000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stroke through `points` (fractions), spread evenly over `t0..t1`.
    fn stroke(t0: u64, t1: u64, points: &[(f64, f64)]) -> Stroke {
        let step = (t1 - t0) / (points.len() as u64 - 1).max(1);
        let pts = points
            .iter()
            .enumerate()
            .flat_map(|(i, &(x, y))| {
                [
                    (x * POINT_SCALE).round() as u32,
                    (y * POINT_SCALE).round() as u32,
                    (i as u64 * step) as u32,
                ]
            })
            .collect();
        Stroke { t0, t1, pts }
    }

    /// A closed loop inside the box `[x0, y0, x1, y1]`.
    fn circle(t0: u64, t1: u64, [x0, y0, x1, y1]: [f64; 4]) -> Stroke {
        stroke(t0, t1, &[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)])
    }

    fn mark(id: &str, t0: u64, t1: u64) -> Mark {
        Mark {
            id: id.to_string(),
            strokes: vec![0],
            t0,
            t1,
            bounds: [0.1, 0.1, 0.2, 0.2],
            shape: Shape::Circle,
            description: format!("circled {id}"),
        }
    }

    fn segment(start_ms: u64, end_ms: u64) -> Segment {
        Segment {
            start_ms,
            end_ms,
            text: String::new(),
        }
    }

    fn words(spoken: &[(u64, &str)]) -> Vec<Word> {
        spoken
            .iter()
            .map(|&(start_ms, text)| Word {
                start_ms,
                end_ms: start_ms + 250,
                text: text.to_string(),
            })
            .collect()
    }

    #[test]
    fn strokes_group_into_marks() {
        struct Case {
            name: &'static str,
            strokes: Vec<Stroke>,
            marks: Vec<Vec<usize>>,
        }
        let cases = [
            Case {
                name: "three overlapping strokes close in time make one mark",
                strokes: vec![
                    stroke(4_900, 5_800, &[(0.30, 0.10), (0.80, 0.10), (0.85, 0.50)]),
                    stroke(7_000, 8_500, &[(0.85, 0.50), (0.60, 0.85), (0.20, 0.80)]),
                    stroke(10_000, 11_500, &[(0.20, 0.80), (0.15, 0.40), (0.31, 0.11)]),
                ],
                marks: vec![vec![0, 1, 2]],
            },
            Case {
                name: "a stroke starting more than 2.5 s after the mark starts a new one",
                strokes: vec![
                    circle(0, 1_000, [0.1, 0.1, 0.3, 0.3]),
                    circle(3_600, 4_000, [0.2, 0.2, 0.4, 0.4]),
                ],
                marks: vec![vec![0], vec![1]],
            },
            Case {
                name: "a stroke elsewhere on screen starts a new mark",
                strokes: vec![
                    circle(0, 1_000, [0.1, 0.1, 0.3, 0.3]),
                    circle(1_200, 2_000, [0.6, 0.6, 0.9, 0.9]),
                ],
                marks: vec![vec![0], vec![1]],
            },
            Case {
                name: "boxes that only touch still join",
                strokes: vec![
                    circle(0, 1_000, [0.1, 0.1, 0.3, 0.3]),
                    circle(1_200, 2_000, [0.3, 0.1, 0.5, 0.3]),
                ],
                marks: vec![vec![0, 1]],
            },
            Case {
                name: "an empty stroke is skipped",
                strokes: vec![
                    circle(0, 1_000, [0.1, 0.1, 0.3, 0.3]),
                    Stroke {
                        t0: 1_100,
                        t1: 1_100,
                        pts: vec![],
                    },
                ],
                marks: vec![vec![0]],
            },
        ];
        for case in cases {
            let marks = group_marks(&case.strokes);
            let grouped: Vec<Vec<usize>> = marks.iter().map(|m| m.strokes.clone()).collect();
            assert_eq!(grouped, case.marks, "{}", case.name);
        }
    }

    #[test]
    fn a_three_stroke_mark_is_described_from_its_box() {
        let marks = group_marks(&[
            stroke(4_900, 5_800, &[(0.30, 0.10), (0.80, 0.10), (0.85, 0.50)]),
            stroke(7_000, 8_500, &[(0.85, 0.50), (0.60, 0.85), (0.20, 0.80)]),
            stroke(10_000, 11_500, &[(0.20, 0.80), (0.15, 0.40), (0.31, 0.11)]),
        ]);
        let mark = &marks[0];
        assert_eq!((mark.id.as_str(), mark.t0, mark.t1), ("m1", 4_900, 11_500));
        assert_eq!(mark.bounds, [0.15, 0.10, 0.85, 0.85]);
        assert_eq!(mark.shape, Shape::Circle);
        assert_eq!(
            mark.description,
            "circled region x 0.15–0.85, y 0.10–0.85 (3 strokes)"
        );
    }

    #[test]
    fn an_open_line_is_a_stroke_not_a_circle() {
        let marks = group_marks(&[stroke(0, 500, &[(0.1, 0.5), (0.5, 0.5), (0.9, 0.52)])]);
        assert_eq!(marks[0].shape, Shape::Stroke);
        assert_eq!(
            marks[0].description,
            "stroke across x 0.10–0.90, y 0.50–0.52"
        );
    }

    #[test]
    fn frames_follow_the_rules() {
        struct Case {
            name: &'static str,
            marks: Vec<Mark>,
            segments: Vec<Segment>,
            frames: Vec<(u64, FrameReason, Vec<&'static str>)>,
        }
        use FrameReason::{Mark as M, Speech as S};
        let cases = [
            Case {
                name: "one frame per mark, 100 ms before it ends",
                marks: vec![mark("m1", 1_000, 5_000), mark("m2", 9_000, 12_000)],
                segments: vec![],
                frames: vec![(4_900, M, vec!["m1"]), (11_900, M, vec!["m2"])],
            },
            Case {
                name: "two marks ending within 1.5 s share one frame",
                marks: vec![mark("m1", 1_000, 10_000), mark("m2", 10_200, 11_500)],
                segments: vec![],
                frames: vec![(11_400, M, vec!["m1", "m2"])],
            },
            Case {
                name: "a chain of marks 1.4 s apart is measured from the frame's first mark",
                marks: (0..8)
                    .map(|i| {
                        let end = 1_000 + i * 1_400;
                        mark(&format!("m{}", i + 1), end - 500, end)
                    })
                    .collect(),
                segments: vec![],
                frames: vec![
                    (2_300, M, vec!["m1", "m2"]),
                    (5_100, M, vec!["m3", "m4"]),
                    (7_900, M, vec!["m5", "m6"]),
                    (10_700, M, vec!["m7", "m8"]),
                ],
            },
            Case {
                name: "marks ending 1.6 s apart get a frame each",
                marks: vec![mark("m1", 1_000, 10_000), mark("m2", 10_200, 11_600)],
                segments: vec![],
                frames: vec![(9_900, M, vec!["m1"]), (11_500, M, vec!["m2"])],
            },
            Case {
                name: "speech far from any mark gets its own frame",
                marks: vec![mark("m1", 1_000, 3_000)],
                segments: vec![segment(5_000, 9_000)],
                frames: vec![(2_900, M, vec!["m1"]), (9_000, S, vec![])],
            },
            Case {
                name: "speech with no marks at all gets frames",
                marks: vec![],
                segments: vec![
                    segment(500, 4_000),
                    segment(5_000, 6_000),
                    segment(9_000, 12_000),
                ],
                frames: vec![(4_000, S, vec![]), (12_000, S, vec![])],
            },
            Case {
                name: "speech ending within 4 s of a mark frame adds nothing",
                marks: vec![mark("m1", 1_000, 7_000)],
                segments: vec![segment(1_000, 3_000)],
                frames: vec![(6_900, M, vec!["m1"])],
            },
            Case {
                name: "a speech frame never lands past the end of the recording",
                marks: vec![],
                segments: vec![segment(1_000, 21_000)],
                frames: vec![(20_000, S, vec![])],
            },
        ];
        for case in cases {
            let frames = choose_frames(&case.marks, &case.segments, &[], 20_000);
            let got: Vec<(u64, FrameReason, Vec<&str>)> = frames
                .iter()
                .map(|f| (f.t, f.reason, f.marks.iter().map(String::as_str).collect()))
                .collect();
            assert_eq!(got, case.frames, "{}", case.name);
            let numbers: Vec<usize> = frames.iter().map(|f| f.n).collect();
            assert_eq!(
                numbers,
                (1..=frames.len()).collect::<Vec<_>>(),
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn the_limit_drops_speech_frames_and_keeps_every_mark() {
        let marks: Vec<Mark> = (0..10)
            .map(|i| mark(&format!("m{}", i + 1), i * 10_000, i * 10_000 + 2_000))
            .collect();
        let segments: Vec<Segment> = (0..6)
            .map(|i| segment(i * 10_000 + 5_000, i * 10_000 + 7_000))
            .collect();
        let frames = choose_frames(&marks, &segments, &[], 120_000);

        assert_eq!(frames.len(), MAX_FRAMES);
        let mark_frames = frames
            .iter()
            .filter(|f| f.reason == FrameReason::Mark)
            .count();
        assert_eq!(mark_frames, 10);
        let speech: Vec<u64> = frames
            .iter()
            .filter(|f| f.reason == FrameReason::Speech)
            .map(|f| f.t)
            .collect();
        assert_eq!(
            speech,
            vec![7_000, 17_000],
            "ties keep the earliest speech frames"
        );
    }

    #[test]
    fn more_marks_than_the_limit_share_frames_instead_of_being_dropped() {
        let ends = [
            2_000, 10_000, 12_000, 20_000, 30_000, 40_000, 50_000, 52_000, 60_000, 70_000, 80_000,
            90_000, 100_000, 110_000,
        ];
        let marks: Vec<Mark> = ends
            .iter()
            .enumerate()
            .map(|(i, &end)| mark(&format!("m{}", i + 1), end - 1_000, end))
            .collect();
        let frames = choose_frames(&marks, &[], &[], 120_000);

        assert_eq!(frames.len(), MAX_FRAMES);
        let shown: Vec<&str> = frames
            .iter()
            .flat_map(|f| f.marks.iter().map(String::as_str))
            .collect();
        let all: Vec<String> = marks.iter().map(|m| m.id.clone()).collect();
        assert_eq!(shown, all.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(frames[1].marks, vec!["m2", "m3"]);
        assert_eq!(frames[1].t, 11_900);
        assert_eq!(frames[5].marks, vec!["m7", "m8"]);
    }

    #[test]
    fn said_is_the_words_since_the_previous_frame() {
        struct Case {
            name: &'static str,
            words: Vec<Word>,
            said: Vec<&'static str>,
        }
        let marks = [mark("m1", 1_000, 5_100), mark("m2", 8_000, 10_100)];
        let cases = [
            Case {
                name: "words split at each frame time plus the grace window",
                words: words(&[
                    (500, "look"),
                    (4_000, "here"),
                    (7_000, "and"),
                    (9_000, "there"),
                ]),
                said: vec!["look here", "and there"],
            },
            Case {
                name: "speech just after drawing belongs to that mark",
                words: words(&[
                    (5_600, "this"),
                    (5_850, "toggle"),
                    (6_100, "is"),
                    (6_350, "wrong."),
                    (8_200, "and"),
                    (8_450, "this"),
                ]),
                said: vec!["this toggle is wrong.", "and this"],
            },
            Case {
                name: "a word starting exactly at the cut-off belongs to the earlier frame",
                words: words(&[(6_500, "now"), (6_501, "next")]),
                said: vec!["now", "next"],
            },
            Case {
                name: "words after the last frame go to the last frame",
                words: words(&[
                    (2_000, "first"),
                    (12_000, "after"),
                    (13_000, "the"),
                    (13_500, "end"),
                ]),
                said: vec!["first", "after the end"],
            },
            Case {
                name: "a frame with no words says nothing",
                words: words(&[(9_000, "only")]),
                said: vec!["", "only"],
            },
        ];
        for case in cases {
            let frames = choose_frames(&marks, &[], &case.words, 20_000);
            let said: Vec<&str> = frames.iter().map(|f| f.said.as_str()).collect();
            assert_eq!(said, case.said, "{}", case.name);
        }
    }

    #[test]
    fn the_grace_window_stops_at_the_next_frame() {
        let marks = [
            mark("m1", 500, 1_000),
            mark("m2", 1_900, 2_400),
            mark("m3", 3_300, 3_800),
        ];
        let frames = choose_frames(
            &marks,
            &[],
            &words(&[(3_600, "first"), (3_750, "second")]),
            20_000,
        );
        let got: Vec<(u64, &str)> = frames.iter().map(|f| (f.t, f.said.as_str())).collect();
        assert_eq!(got, [(2_300, "first"), (3_700, "second")]);
    }

    #[test]
    fn no_marks_and_no_speech_means_no_frames() {
        assert!(choose_frames(&[], &[], &[], 5_000).is_empty());
    }

    fn example_session() -> Session {
        let spoken = [
            (3_000, "Here"),
            (3_300, "is"),
            (3_500, "the"),
            (3_700, "settings"),
            (4_100, "page."),
            (5_200, "This"),
            (5_500, "toggle"),
            (5_900, "should"),
            (6_100, "be"),
            (6_300, "on"),
            (6_500, "the"),
            (6_700, "right."),
            (19_500, "And"),
            (19_800, "this"),
            (20_000, "list"),
            (20_300, "is"),
            (20_500, "too"),
            (20_700, "dense."),
            (27_000, "The"),
            (27_300, "error"),
            (27_700, "at"),
            (27_900, "the"),
            (28_100, "bottom"),
            (28_500, "is"),
            (28_700, "cut"),
            (29_000, "off."),
        ];
        Session {
            id: "2026-11-14_14-56-36".to_string(),
            started_at: "2026-11-14T14:56:36.159Z".to_string(),
            duration_ms: 31_400,
            display: Display {
                id: 1,
                frame: Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 1512.0,
                    h: 982.0,
                },
                scale: 2.0,
            },
            frontmost_app: Some(App {
                name: "System Settings".to_string(),
                bundle_id: "com.apple.systempreferences".to_string(),
            }),
            video: Some(Video {
                file: "recording.mp4".to_string(),
                offset_ms: 140,
                width: 3024,
                height: 1964,
                fps: 15,
            }),
            audio: Audio {
                file: "audio.wav".to_string(),
                offset_ms: 35,
                sample_rate: 16_000,
            },
            transcript_model: "ggml-small.en".to_string(),
            words: words(&spoken),
            strokes: vec![
                stroke(4_900, 5_800, &[(0.62, 0.20), (0.90, 0.20), (0.91, 0.30)]),
                stroke(6_400, 7_100, &[(0.91, 0.30), (0.75, 0.33), (0.63, 0.21)]),
                stroke(7_500, 8_300, &[(0.63, 0.21), (0.70, 0.18), (0.62, 0.20)]),
                circle(19_900, 21_200, [0.20, 0.18, 0.74, 0.70]),
                stroke(21_400, 22_000, &[(0.80, 0.75), (0.95, 0.76)]),
            ],
            missing_permissions: vec![],
        }
    }

    const EXAMPLE_FOLDER: &str = "/Users/me/Library/Application Support/com.kelechiodom.dictate/annotations/2026-11-14_14-56-36";

    #[test]
    fn the_example_session_builds_the_expected_summary() {
        let annotation = build(example_session());
        assert_eq!(
            summary(&annotation, Path::new(EXAMPLE_FOLDER)),
            "\
[annotation 0:31 · System Settings · /Users/me/Library/Application Support/com.kelechiodom.dictate/annotations/2026-11-14_14-56-36]
Open contact-sheet.png and frames/ in that folder first: the marks only make sense on the images.
Positions are fractions of the screen, x left→right, y top→bottom. Full data: annotation.json.
- #1 (0:08) circled region x 0.62–0.91, y 0.18–0.33 (3 strokes) — “Here is the settings page. This toggle should be on the right.”
- #2 (0:21) circled region x 0.20–0.74, y 0.18–0.70; stroke across x 0.80–0.95, y 0.75–0.76 — “And this list is too dense.”
- #3 (0:29) “The error at the bottom is cut off.”
"
        );
    }

    #[test]
    fn build_moves_words_onto_the_session_clock() {
        let annotation = build(example_session());
        assert_eq!(
            annotation.transcript.words[0],
            TimedText {
                t0: 3_035,
                t1: 3_285,
                text: "Here".to_string()
            }
        );
        assert_eq!(annotation.transcript.segments.len(), 4);
        assert_eq!(annotation.frames[2].t, 29_285);
        assert_eq!(annotation.frames[2].reason, FrameReason::Speech);
    }

    #[test]
    fn annotation_json_round_trips_with_the_schema_names() {
        let annotation = build(example_session());
        let json = serde_json::to_string_pretty(&annotation).unwrap();
        let back: Annotation = serde_json::from_str(&json).unwrap();
        assert_eq!(back, annotation);

        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["schema"], SCHEMA);
        assert_eq!(value["durationMs"], 31_400);
        assert_eq!(
            value["frontmostApp"]["bundleId"],
            "com.apple.systempreferences"
        );
        assert_eq!(value["video"]["offsetMs"], 140);
        assert_eq!(
            value["marks"][0]["box"],
            serde_json::json!([0.62, 0.18, 0.91, 0.33])
        );
        assert_eq!(value["marks"][0]["shape"], "circle");
        assert_eq!(value["frames"][0]["reason"], "mark");
        assert_eq!(value["frames"][0]["file"], "frames/frame-01.png");
        assert_eq!(value["frames"][0]["full"], "frames-full/frame-01.png");
        assert_eq!(value["contactSheet"], CONTACT_SHEET);
    }

    #[test]
    fn a_session_without_video_or_app_leaves_those_keys_out() {
        let mut session = example_session();
        session.video = None;
        session.frontmost_app = None;
        let annotation = build(session);
        let value = serde_json::to_value(&annotation).unwrap();
        assert!(value.get("video").is_none());
        assert!(value.get("frontmostApp").is_none());
        assert!(
            summary(&annotation, Path::new("/tmp/a")).starts_with("[annotation 0:31 · /tmp/a]\n")
        );
        let back: Annotation = serde_json::from_value(value).unwrap();
        assert_eq!(back, annotation);
    }

    #[test]
    fn write_package_writes_both_files() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().join("2026-11-14_14-56-36");
        let annotation = build(example_session());

        let copied = write_package(&folder, &annotation).unwrap();

        let on_disk: Annotation =
            serde_json::from_str(&std::fs::read_to_string(folder.join("annotation.json")).unwrap())
                .unwrap();
        assert_eq!(on_disk, annotation);
        assert_eq!(
            std::fs::read_to_string(folder.join("summary.md")).unwrap(),
            copied
        );
        assert_eq!(copied, summary(&annotation, &folder));
    }
}
