//! The contact sheet: every key frame in one grid, each with a badge carrying
//! its number from summary.md. `layout` is the pure part (sizes and places);
//! `render` paints it.

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, PixmapPaint, Rect,
    Transform,
};

use crate::package::{Display, Stroke};

/// Long edge of the whole sheet. Twelve frames in four columns stay about
/// 580 px wide each, enough to read UI text, and the PNG stays a few MB.
pub const SHEET_MAX_EDGE: u32 = 2400;
pub const SHEET_GAP: u32 = 16;
const BACKGROUND: [u8; 3] = [0x1C, 0x1C, 0x1E];
const BADGE_FILL: [u8; 3] = [0xFF, 0xFF, 0xFF];
const BADGE_TEXT: [u8; 3] = [0x1C, 0x1C, 0x1E];
/// Badge height as a fraction of the cell width, clamped to stay legible on a
/// crowded sheet and unobtrusive on a sparse one.
const BADGE_FRACTION: u32 = 12;
const BADGE_MIN: u32 = 32;
const BADGE_MAX: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Area {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Area {
    fn right(&self) -> u32 {
        self.x + self.w
    }

    fn bottom(&self) -> u32 {
        self.y + self.h
    }

    pub fn contains(&self, other: &Area) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub number: usize,
    pub image: Area,
    pub badge: Area,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetLayout {
    pub width: u32,
    pub height: u32,
    pub columns: usize,
    pub cells: Vec<Cell>,
}

/// Up to four in a row; five and six as two rows of three; then four columns.
pub fn columns(count: usize) -> usize {
    match count {
        0..=4 => count,
        5 | 6 => 3,
        _ => 4,
    }
}

/// Sizes the grid so the sheet's long edge is at most `max_edge`, without
/// scaling frames up. A short last row is centred. Each badge sits in its
/// cell's top-right corner and widens for two-digit numbers.
pub fn layout(numbers: &[usize], frame_w: u32, frame_h: u32, max_edge: u32) -> SheetLayout {
    let empty = SheetLayout {
        width: 0,
        height: 0,
        columns: 0,
        cells: Vec::new(),
    };
    if numbers.is_empty() || frame_w == 0 || frame_h == 0 {
        return empty;
    }
    let count = numbers.len();
    let cols = columns(count);
    let rows = count.div_ceil(cols);
    let (cols32, rows32) = (cols as u32, rows as u32);
    let room = |slots: u32| max_edge.saturating_sub(SHEET_GAP * (slots + 1)) / slots;
    let by_width = room(cols32);
    let by_height = (u64::from(room(rows32)) * u64::from(frame_w) / u64::from(frame_h)) as u32;
    let cell_w = frame_w.min(by_width).min(by_height).max(1);
    let cell_h = ((u64::from(cell_w) * u64::from(frame_h) / u64::from(frame_w)) as u32).max(1);

    let badge_h = (cell_w / BADGE_FRACTION)
        .clamp(BADGE_MIN, BADGE_MAX)
        .min(cell_h / 2)
        .max(1);
    let margin = badge_h / 4;

    let cells = numbers
        .iter()
        .enumerate()
        .map(|(index, &number)| {
            let (row, col) = ((index / cols) as u32, (index % cols) as u32);
            let in_row = (count - row as usize * cols).min(cols) as u32;
            let indent = (cols32 - in_row) * (cell_w + SHEET_GAP) / 2;
            let image = Area {
                x: SHEET_GAP + indent + col * (cell_w + SHEET_GAP),
                y: SHEET_GAP + row * (cell_h + SHEET_GAP),
                w: cell_w,
                h: cell_h,
            };
            let digits = number.max(1).ilog10() + 1;
            let badge_w = (badge_h + (digits - 1) * badge_h * 11 / 20).min(cell_w);
            let badge = Area {
                x: image.right().saturating_sub(badge_w + margin).max(image.x),
                y: image.y + margin,
                w: badge_w,
                h: badge_h,
            };
            Cell {
                number,
                image,
                badge,
            }
        })
        .collect();

    SheetLayout {
        width: cols32 * cell_w + SHEET_GAP * (cols32 + 1),
        height: rows32 * cell_h + SHEET_GAP * (rows32 + 1),
        columns: cols,
        cells,
    }
}

/// One key frame as the sheet needs it.
pub struct Frame<'a> {
    /// Already inked; all frames have the size of the first.
    pub image: &'a Pixmap,
    /// The badge, as in summary.md.
    pub number: usize,
    /// Redrawn over the scaled cell at `SHEET_INK_MIN_PX` or more.
    pub strokes: Vec<&'a Stroke>,
}

pub fn render(frames: &[Frame], display: &Display) -> Result<Pixmap, String> {
    let first = frames.first().ok_or("No frames for the contact sheet")?;
    let numbers: Vec<usize> = frames.iter().map(|frame| frame.number).collect();
    let plan = layout(
        &numbers,
        first.image.width(),
        first.image.height(),
        SHEET_MAX_EDGE,
    );
    let mut sheet = Pixmap::new(plan.width, plan.height).ok_or("Allocate the contact sheet")?;
    sheet.fill(Color::from_rgba8(
        BACKGROUND[0],
        BACKGROUND[1],
        BACKGROUND[2],
        255,
    ));
    for (frame, cell) in frames.iter().zip(&plan.cells) {
        let mut scaled = super::resize(frame.image, cell.image.w, cell.image.h)
            .ok_or("Scale a frame for the sheet")?;
        let width = super::ink_width(display, scaled.width()).max(super::SHEET_INK_MIN_PX);
        super::draw_strokes(&mut scaled, &frame.strokes, width);
        sheet.draw_pixmap(
            cell.image.x as i32,
            cell.image.y as i32,
            scaled.as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        draw_badge(&mut sheet, cell.badge, cell.number);
    }
    Ok(sheet)
}

fn paint(rgb: [u8; 3]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(rgb[0], rgb[1], rgb[2], 255);
    paint.anti_alias = true;
    paint
}

fn draw_badge(sheet: &mut Pixmap, area: Area, number: usize) {
    let (x, y, w, h) = (area.x as f32, area.y as f32, area.w as f32, area.h as f32);
    if let Some(shape) = rounded_rect(x, y, w, h, h * 0.28) {
        sheet.fill_path(
            &shape,
            &paint(BADGE_FILL),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    let text = number.to_string();
    let glyph_h = h * 0.54;
    let advance = (DIGIT_WIDTH + DIGIT_SPACING) * glyph_h;
    let text_w = text.len() as f32 * advance - DIGIT_SPACING * glyph_h;
    let mut origin_x = x + (w - text_w) / 2.0;
    let origin_y = y + (h - glyph_h) / 2.0;
    let style = tiny_skia::Stroke {
        width: DIGIT_STROKE,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        ..tiny_skia::Stroke::default()
    };
    for digit in text.bytes().map(|b| b - b'0') {
        if let Some(path) = digit_path(digit) {
            let at = Transform::from_row(glyph_h, 0.0, 0.0, glyph_h, origin_x, origin_y);
            sheet.stroke_path(&path, &paint(BADGE_TEXT), &style, at, None);
        }
        origin_x += advance;
    }
}

fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    pb.finish()
}

/// Digits are drawn as strokes in a 0.6 × 1 box (y down), then scaled to the
/// badge. Only 0–9 are needed, so this avoids bundling a font file.
const DIGIT_WIDTH: f32 = 0.6;
const DIGIT_SPACING: f32 = 0.2;
const DIGIT_STROKE: f32 = 0.15;

fn digit_path(digit: u8) -> Option<Path> {
    let mut pb = PathBuilder::new();
    let oval = |pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32| {
        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            pb.push_oval(rect);
        }
    };
    match digit {
        0 => oval(&mut pb, 0.0, 0.0, 0.6, 1.0),
        1 => {
            pb.move_to(0.1, 0.22);
            pb.line_to(0.36, 0.0);
            pb.line_to(0.36, 1.0);
        }
        2 => {
            pb.move_to(0.03, 0.24);
            pb.cubic_to(0.08, -0.06, 0.6, -0.06, 0.58, 0.28);
            pb.cubic_to(0.56, 0.52, 0.2, 0.66, 0.02, 1.0);
            pb.line_to(0.6, 1.0);
        }
        3 => {
            pb.move_to(0.04, 0.12);
            pb.cubic_to(0.2, -0.05, 0.58, -0.02, 0.56, 0.25);
            pb.cubic_to(0.54, 0.44, 0.36, 0.48, 0.22, 0.48);
            pb.cubic_to(0.46, 0.48, 0.61, 0.58, 0.59, 0.76);
            pb.cubic_to(0.57, 1.03, 0.14, 1.05, 0.01, 0.87);
        }
        4 => {
            pb.move_to(0.45, 1.0);
            pb.line_to(0.45, 0.0);
            pb.line_to(0.0, 0.7);
            pb.line_to(0.62, 0.7);
        }
        5 => {
            pb.move_to(0.55, 0.0);
            pb.line_to(0.1, 0.0);
            pb.line_to(0.06, 0.46);
            pb.cubic_to(0.3, 0.32, 0.61, 0.4, 0.59, 0.7);
            pb.cubic_to(0.57, 1.03, 0.14, 1.05, 0.01, 0.87);
        }
        6 => {
            pb.move_to(0.52, 0.04);
            pb.cubic_to(0.24, -0.04, 0.02, 0.22, 0.02, 0.66);
            oval(&mut pb, 0.02, 0.38, 0.56, 0.62);
        }
        7 => {
            pb.move_to(0.0, 0.0);
            pb.line_to(0.6, 0.0);
            pb.line_to(0.2, 1.0);
        }
        8 => {
            oval(&mut pb, 0.06, 0.0, 0.48, 0.46);
            oval(&mut pb, 0.02, 0.46, 0.56, 0.54);
        }
        9 => {
            oval(&mut pb, 0.02, 0.0, 0.56, 0.62);
            pb.move_to(0.58, 0.34);
            pb.cubic_to(0.58, 0.78, 0.36, 1.04, 0.08, 0.96);
        }
        _ => return None,
    }
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(count: usize) -> Vec<usize> {
        (1..=count).collect()
    }

    fn assert_well_formed(plan: &SheetLayout, max_edge: u32) {
        let sheet = Area {
            x: 0,
            y: 0,
            w: plan.width,
            h: plan.height,
        };
        assert!(plan.width.max(plan.height) <= max_edge, "{plan:?}");
        for (i, cell) in plan.cells.iter().enumerate() {
            assert!(sheet.contains(&cell.image), "cell {i} off the sheet");
            assert!(cell.image.contains(&cell.badge), "badge {i} outside cell");
            for other in &plan.cells[i + 1..] {
                let apart = cell.image.right() <= other.image.x
                    || other.image.right() <= cell.image.x
                    || cell.image.bottom() <= other.image.y
                    || other.image.bottom() <= cell.image.y;
                assert!(apart, "cells overlap: {cell:?} {other:?}");
            }
        }
    }

    #[test]
    fn column_counts() {
        let got: Vec<usize> = (0..=12).map(columns).collect();
        assert_eq!(got, [0, 1, 2, 3, 4, 3, 3, 4, 4, 4, 4, 4, 4]);
    }

    #[test]
    fn nothing_to_lay_out() {
        assert!(layout(&[], 1568, 980, SHEET_MAX_EDGE).cells.is_empty());
        assert!(layout(&[1], 0, 980, SHEET_MAX_EDGE).cells.is_empty());
    }

    #[test]
    fn one_small_frame_is_not_scaled_up() {
        let plan = layout(&[1], 800, 500, SHEET_MAX_EDGE);
        assert_eq!((plan.width, plan.height), (832, 532));
        assert_eq!(
            plan.cells[0].image,
            Area {
                x: 16,
                y: 16,
                w: 800,
                h: 500
            }
        );
        assert_well_formed(&plan, SHEET_MAX_EDGE);
    }

    #[test]
    fn twelve_frames_fill_four_columns_within_the_limit() {
        let plan = layout(&numbers(12), 1568, 980, SHEET_MAX_EDGE);
        assert_eq!(plan.columns, 4);
        assert_eq!((plan.width, plan.height), (2400, 1150));
        assert_eq!(plan.cells[0].image.w, 580);
        assert_well_formed(&plan, SHEET_MAX_EDGE);
    }

    #[test]
    fn a_short_last_row_is_centred() {
        let plan = layout(&numbers(7), 1568, 980, SHEET_MAX_EDGE);
        let first_row_left = plan.cells[0].image.x;
        let first_row_right = plan.cells[3].image.right();
        let last_row_left = plan.cells[4].image.x;
        let last_row_right = plan.cells[6].image.right();
        assert_eq!(
            last_row_left - first_row_left,
            first_row_right - last_row_right
        );
        assert_eq!(plan.cells[4].image.y, plan.cells[5].image.y);
        assert_well_formed(&plan, SHEET_MAX_EDGE);
    }

    #[test]
    fn portrait_frames_are_bounded_by_height() {
        let plan = layout(&numbers(12), 980, 1568, SHEET_MAX_EDGE);
        assert!(plan.height <= SHEET_MAX_EDGE && plan.height > 2300);
        assert!(plan.width < SHEET_MAX_EDGE);
        assert_well_formed(&plan, SHEET_MAX_EDGE);
    }

    #[test]
    fn badges_sit_top_right_and_widen_for_two_digits() {
        let plan = layout(&numbers(12), 1568, 980, SHEET_MAX_EDGE);
        let nine = &plan.cells[8];
        let ten = &plan.cells[9];
        assert_eq!(nine.number, 9);
        assert_eq!(ten.number, 10);
        assert_eq!(nine.badge.h, 48);
        assert_eq!(nine.badge.w, 48);
        assert!(ten.badge.w > nine.badge.w);
        for cell in [nine, ten] {
            let margin = cell.badge.h / 4;
            assert_eq!(cell.badge.y, cell.image.y + margin);
            assert_eq!(cell.badge.right(), cell.image.right() - margin);
        }
    }

    #[test]
    fn badges_stay_legible_on_a_crowded_sheet_and_fit_tiny_cells() {
        let crowded = layout(&numbers(12), 1568, 980, 1200);
        assert_eq!(crowded.cells[0].badge.h, BADGE_MIN);
        assert_well_formed(&crowded, 1200);

        let tiny = layout(&numbers(12), 40, 25, SHEET_MAX_EDGE);
        assert_well_formed(&tiny, SHEET_MAX_EDGE);
    }

    #[test]
    fn every_digit_has_a_shape() {
        for digit in 0..=9 {
            let path = digit_path(digit).expect("digit path");
            let bounds = path.bounds();
            assert!(bounds.left() >= -0.1 && bounds.right() <= 0.7, "{digit}");
            assert!(bounds.top() >= -0.1 && bounds.bottom() <= 1.1, "{digit}");
        }
        assert!(digit_path(10).is_none());
    }
}
