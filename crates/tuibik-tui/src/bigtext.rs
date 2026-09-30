//! Built-in big-digit font for the timer.
//!
//! Glyphs exist for `0-9 . : + D N F`. There are two designs: a 5-row block
//! font (drawn with full blocks, optionally scaled horizontally/vertically)
//! and a 3-row half-block font for tight spaces. [`render_big`] picks the
//! largest size whose width and height fit the area and falls back to one
//! bold line when nothing fits (or a character has no glyph).

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// A size the text can be drawn at, largest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// 5-row font, each pixel 4 columns × 2 rows (10 rows tall).
    Huge,
    /// 5-row font, each pixel 2 columns × 1 row (5 rows tall).
    Large,
    /// 5-row font, 1 column per pixel (5 rows tall).
    Normal,
    /// 3-row half-block font.
    Medium,
    /// Single bold line of plain text.
    Line,
}

const SIZES: [Size; 4] = [Size::Huge, Size::Large, Size::Normal, Size::Medium];

/// The rendered text plus its dimensions.
#[derive(Debug, Clone)]
pub struct BigText {
    pub lines: Vec<Line<'static>>,
    #[allow(dead_code)] // inspected by tests
    pub width: u16,
    pub height: u16,
    #[allow(dead_code)] // inspected by tests
    pub size: Size,
}

/// 5-row glyphs; `#` is a filled pixel.
fn large_glyph(c: char) -> Option<[&'static str; 5]> {
    Some(match c {
        '0' => ["#####", "#   #", "#   #", "#   #", "#####"],
        '1' => [" ##  ", "  #  ", "  #  ", "  #  ", "#####"],
        '2' => ["#####", "    #", "#####", "#    ", "#####"],
        '3' => ["#####", "    #", " ####", "    #", "#####"],
        '4' => ["#   #", "#   #", "#####", "    #", "    #"],
        '5' => ["#####", "#    ", "#####", "    #", "#####"],
        '6' => ["#####", "#    ", "#####", "#   #", "#####"],
        '7' => ["#####", "    #", "   # ", "  #  ", "  #  "],
        '8' => ["#####", "#   #", "#####", "#   #", "#####"],
        '9' => ["#####", "#   #", "#####", "    #", "#####"],
        '.' => ["  ", "  ", "  ", "##", "##"],
        ':' => ["  ", "##", "  ", "##", "  "],
        '+' => ["  #  ", "  #  ", "#####", "  #  ", "  #  "],
        'D' => ["#### ", "#   #", "#   #", "#   #", "#### "],
        'N' => ["#   #", "##  #", "# # #", "#  ##", "#   #"],
        'F' => ["#####", "#    ", "#### ", "#    ", "#    "],
        _ => return None,
    })
}

/// 3-row half-block glyphs.
fn medium_glyph(c: char) -> Option<[&'static str; 3]> {
    Some(match c {
        '0' => ["█▀█", "█ █", "▀▀▀"],
        '1' => ["▄█ ", " █ ", "▄█▄"],
        '2' => ["▀▀█", "▄▀▀", "▀▀▀"],
        '3' => ["▀▀█", " ▀▄", "▀▀▀"],
        '4' => ["█ █", "▀▀█", "  ▀"],
        '5' => ["█▀▀", "▀▀█", "▀▀▀"],
        '6' => ["█▀▀", "█▀█", "▀▀▀"],
        '7' => ["▀▀█", "  █", "  ▀"],
        '8' => ["█▀█", "█▀█", "▀▀▀"],
        '9' => ["█▀█", "▀▀█", "▀▀▀"],
        '.' => [" ", " ", "▀"],
        ':' => ["▀", " ", "▀"],
        '+' => [" ▄ ", "▀█▀", "   "],
        'D' => ["█▀▄", "█ █", "▀▀ "],
        'N' => ["█▀▄█", "█ ▀█", "█  █"],
        'F' => ["█▀▀", "█▀ ", "▀  "],
        _ => return None,
    })
}

fn scale_of(size: Size) -> (usize, usize) {
    match size {
        Size::Huge => (4, 2),
        Size::Large => (2, 1),
        _ => (1, 1),
    }
}

/// `(width, height)` of `text` at `size`, or `None` if a character has no glyph.
pub fn measure(text: &str, size: Size) -> Option<(u16, u16)> {
    let n = text.chars().count();
    if n == 0 {
        return None;
    }
    match size {
        Size::Line => Some((n as u16, 1)),
        Size::Medium => {
            let mut w = 0usize;
            for c in text.chars() {
                w += medium_glyph(c)?[0].chars().count();
            }
            Some(((w + (n - 1)) as u16, 3))
        }
        _ => {
            let (h, v) = scale_of(size);
            let mut w = 0usize;
            for c in text.chars() {
                w += large_glyph(c)?[0].chars().count() * h;
            }
            Some(((w + h * (n - 1)) as u16, (5 * v) as u16))
        }
    }
}

fn build(text: &str, size: Size, style: Style) -> Vec<Line<'static>> {
    match size {
        Size::Line => vec![Line::from(Span::styled(
            text.to_string(),
            style.add_modifier(Modifier::BOLD),
        ))],
        Size::Medium => (0..3)
            .map(|row| {
                let parts: Vec<&str> = text
                    .chars()
                    .filter_map(medium_glyph)
                    .map(|g| g[row])
                    .collect();
                Line::from(Span::styled(parts.join(" "), style))
            })
            .collect(),
        _ => {
            let (h, v) = scale_of(size);
            let gap = " ".repeat(h);
            let mut lines = Vec::with_capacity(5 * v);
            for row in 0..5 {
                let parts: Vec<String> = text
                    .chars()
                    .filter_map(large_glyph)
                    .map(|g| {
                        g[row]
                            .chars()
                            .map(|p| {
                                if p == '#' {
                                    "█".repeat(h)
                                } else {
                                    " ".repeat(h)
                                }
                            })
                            .collect::<String>()
                    })
                    .collect();
                let s = parts.join(&gap);
                for _ in 0..v {
                    lines.push(Line::from(Span::styled(s.clone(), style)));
                }
            }
            lines
        }
    }
}

/// Render `text` with the largest size that fits `area`; a single bold line if
/// none fits.
pub fn render_big(text: &str, area: Rect, style: Style) -> BigText {
    for size in SIZES {
        if let Some((w, h)) = measure(text, size) {
            if w <= area.width && h <= area.height {
                return BigText {
                    lines: build(text, size, style),
                    width: w,
                    height: h,
                    size,
                };
            }
        }
    }
    BigText {
        lines: build(text, Size::Line, style),
        width: text.chars().count() as u16,
        height: 1,
        size: Size::Line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_glyph_row_has_uniform_width() {
        for c in "0123456789.:+DNF".chars() {
            let g = large_glyph(c).unwrap();
            let w = g[0].chars().count();
            assert!(g.iter().all(|r| r.chars().count() == w), "large {c}");
            let m = medium_glyph(c).unwrap();
            let mw = m[0].chars().count();
            assert!(m.iter().all(|r| r.chars().count() == mw), "medium {c}");
        }
    }

    #[test]
    fn all_lines_of_a_rendering_have_equal_width() {
        for size in SIZES {
            let (w, _) = measure("12:34.56+", size).unwrap();
            for l in build("12:34.56+", size, Style::default()) {
                assert_eq!(l.width() as u16, w, "{size:?}");
            }
        }
    }

    #[test]
    fn fits_at_100x30_and_is_tall_and_within_bounds() {
        let area = Rect::new(0, 0, 100, 30);
        let b = render_big("12.34", area, Style::default());
        assert_ne!(b.size, Size::Line);
        assert!(b.height >= 3);
        assert!(b.width <= 100 && b.height <= 30);
        assert_eq!(b.lines.len() as u16, b.height);
    }

    #[test]
    fn picks_larger_size_when_there_is_room() {
        let big = render_big("12.34", Rect::new(0, 0, 160, 40), Style::default());
        assert_eq!(big.size, Size::Huge);
        let mid = render_big("12.34", Rect::new(0, 0, 60, 20), Style::default());
        assert_eq!(mid.size, Size::Large);
        let small = render_big("12.34", Rect::new(0, 0, 30, 20), Style::default());
        assert_eq!(small.size, Size::Normal);
        let tiny = render_big("12.34", Rect::new(0, 0, 20, 3), Style::default());
        assert_eq!(tiny.size, Size::Medium);
    }

    #[test]
    fn tiny_area_falls_back_to_one_line() {
        let b = render_big("12.34", Rect::new(0, 0, 10, 2), Style::default());
        assert_eq!(b.size, Size::Line);
        assert_eq!(b.height, 1);
        assert_eq!(b.lines.len(), 1);
    }

    #[test]
    fn short_area_uses_single_line_even_if_wide() {
        let b = render_big("12.34", Rect::new(0, 0, 200, 2), Style::default());
        assert_eq!(b.size, Size::Line);
    }

    #[test]
    fn unsupported_characters_fall_back_to_a_line() {
        let b = render_big("SOLVING", Rect::new(0, 0, 200, 40), Style::default());
        assert_eq!(b.size, Size::Line);
        assert_eq!(b.lines[0].to_string(), "SOLVING");
    }

    #[test]
    fn penalty_glyphs_render() {
        for t in ["DNF", "14.34+", "+2", "1:05.00"] {
            let b = render_big(t, Rect::new(0, 0, 200, 40), Style::default());
            assert_ne!(b.size, Size::Line, "{t}");
        }
    }
}
