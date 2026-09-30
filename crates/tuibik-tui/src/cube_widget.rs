//! Cube net widget: renders the unfolded cube as separated sticker tiles.
//!
//! The net is drawn on a [`PixelBuf`] of square "pixels" (one column wide,
//! half a row tall) that is turned into `▀` half-block cells. Stickers sit on
//! a dark grout so neighbours of the same color stay distinguishable, and
//! faces are spaced further apart than stickers so each 3×3 face reads as a
//! unit.

use cube::render::{net, NetCell, Rgb, NET_COLS, NET_ROWS};
use cube::Cube;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Color of the plastic between stickers.
pub const GROUT: Rgb = Rgb::new(0x14, 0x14, 0x16);
/// Brightness of stickers outside the highlight while one is shown.
const DIM: f32 = 0.35;
/// Face letters by face index (URFDLB order).
const FACE_LETTERS: [char; 6] = ['U', 'R', 'F', 'D', 'L', 'B'];

pub fn to_color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}

/// `rgb` with every channel multiplied by `k` (0..=1).
pub fn scale_rgb(rgb: Rgb, k: f32) -> Rgb {
    let f = |c: u8| (c as f32 * k).round().clamp(0.0, 255.0) as u8;
    Rgb::new(f(rgb.r), f(rgb.g), f(rgb.b))
}

/// A readable text color on top of `bg`, picked from its luminance.
pub fn contrast_fg(bg: Rgb) -> Color {
    let lum = 0.2126 * bg.r as f32 + 0.7152 * bg.g as f32 + 0.0722 * bg.b as f32;
    if lum > 140.0 {
        Color::Rgb(0x10, 0x10, 0x10)
    } else {
        Color::Rgb(0xF5, 0xF5, 0xF5)
    }
}

// ===== Half-block pixel canvas =====

/// A grid of square pixels; `None` is transparent (terminal background).
/// Two vertically adjacent pixels share one terminal cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PixelBuf {
    pub w: usize,
    pub h: usize,
    px: Vec<Option<Rgb>>,
}

impl PixelBuf {
    pub fn new(w: usize, h: usize) -> Self {
        PixelBuf {
            w,
            h,
            px: vec![None; w * h],
        }
    }

    pub fn get(&self, x: usize, y: usize) -> Option<Rgb> {
        self.px[y * self.w + x]
    }

    pub fn set(&mut self, x: usize, y: usize, c: Rgb) {
        if x < self.w && y < self.h {
            self.px[y * self.w + x] = Some(c);
        }
    }

    pub fn fill_rect(&mut self, x: usize, y: usize, w: usize, h: usize, c: Rgb) {
        for yy in y..(y + h).min(self.h) {
            for xx in x..(x + w).min(self.w) {
                self.px[yy * self.w + xx] = Some(c);
            }
        }
    }

    /// Every pixel, row-major.
    #[cfg(test)]
    pub fn pixels(&self) -> &[Option<Rgb>] {
        &self.px
    }

    /// Terminal rows needed to show the buffer.
    pub fn rows(&self) -> usize {
        self.h.div_ceil(2)
    }

    /// One styled cell per terminal position (`rows() × w`).
    pub fn cells(&self) -> Vec<Vec<(char, Style)>> {
        (0..self.rows())
            .map(|row| {
                (0..self.w)
                    .map(|x| {
                        let top = self.get(x, row * 2);
                        let bottom = if row * 2 + 1 < self.h {
                            self.get(x, row * 2 + 1)
                        } else {
                            None
                        };
                        halfblock(top, bottom)
                    })
                    .collect()
            })
            .collect()
    }

    /// One quadrant-block cell per 2×2 pixels (`h/2 × w/2`, rounded up).
    /// Where a cell holds more than two colors, `favor` is kept if present.
    pub fn quadrant_cells(&self, favor: Rgb) -> Vec<Vec<(char, Style)>> {
        let px = |x: usize, y: usize| {
            if x < self.w && y < self.h {
                self.get(x, y)
            } else {
                None
            }
        };
        (0..self.h.div_ceil(2))
            .map(|row| {
                (0..self.w.div_ceil(2))
                    .map(|col| {
                        let (x, y) = (col * 2, row * 2);
                        quadrant(
                            [px(x, y), px(x + 1, y), px(x, y + 1), px(x + 1, y + 1)],
                            favor,
                        )
                    })
                    .collect()
            })
            .collect()
    }

    /// The buffer as quadrant-block lines (2×2 pixels per cell).
    pub fn quadrant_lines(&self, favor: Rgb) -> Vec<Line<'static>> {
        cells_to_lines(self.quadrant_cells(favor))
    }
}

/// Quadrant glyphs indexed by a mask of filled quarters: bit 0 top-left,
/// bit 1 top-right, bit 2 bottom-left, bit 3 bottom-right.
const QUADRANTS: [char; 16] = [
    ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
];

/// The glyph and style showing 2×2 pixels (top-left, top-right, bottom-left,
/// bottom-right) with at most two colors. Extra colors fold into the nearer
/// of the two kept ones: `favor` if present, plus the most common other.
fn quadrant(px: [Option<Rgb>; 4], favor: Rgb) -> (char, Style) {
    let mut seen: Vec<(Option<Rgb>, usize)> = Vec::new();
    for p in px {
        match seen.iter_mut().find(|(c, _)| *c == p) {
            Some((_, n)) => *n += 1,
            None => seen.push((p, 1)),
        }
    }
    // Most common first (stable, so ties keep reading order).
    seen.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    let a = if seen.iter().any(|(c, _)| *c == Some(favor)) {
        Some(favor)
    } else {
        seen[0].0
    };
    let Some(b) = seen.iter().map(|(c, _)| *c).find(|c| *c != a) else {
        return match a {
            None => (' ', Style::default()),
            Some(c) => ('█', Style::default().fg(to_color(c))),
        };
    };
    let dist = |p: Rgb, q: Rgb| {
        let d = |x: u8, y: u8| (x as i32 - y as i32).pow(2);
        d(p.r, q.r) + d(p.g, q.g) + d(p.b, q.b)
    };
    // Which of `a`/`b` each pixel shows: transparency only where it was.
    let pick = |p: Option<Rgb>| match (p, a, b) {
        _ if p == a || p == b => p,
        (None, _, _) => a,
        (Some(_), None, other) | (Some(_), other, None) => other,
        (Some(p), Some(x), Some(y)) => Some(if dist(p, x) <= dist(p, y) { x } else { y }),
    };
    // The foreground is a color; the background may be transparent.
    let (fg, bg) = match (a, b) {
        (None, Some(c)) => (c, None),
        (Some(c), other) => (c, other),
        (None, None) => unreachable!("a and b differ"),
    };
    let mask = px
        .iter()
        .enumerate()
        .filter(|&(_, p)| pick(*p) == Some(fg))
        .fold(0, |m, (i, _)| m | 1 << i);
    let style = Style::default().fg(to_color(fg));
    let style = match bg {
        Some(c) => style.bg(to_color(c)),
        None => style,
    };
    (QUADRANTS[mask], style)
}

/// The glyph and style showing `top` over `bottom` in a single cell.
fn halfblock(top: Option<Rgb>, bottom: Option<Rgb>) -> (char, Style) {
    match (top, bottom) {
        (None, None) => (' ', Style::default()),
        (Some(t), None) => ('▀', Style::default().fg(to_color(t))),
        (None, Some(b)) => ('▄', Style::default().fg(to_color(b))),
        (Some(t), Some(b)) if t == b => ('█', Style::default().fg(to_color(t))),
        (Some(t), Some(b)) => ('▀', Style::default().fg(to_color(t)).bg(to_color(b))),
    }
}

/// Join cells into lines, merging runs of equally styled cells into one span.
pub fn cells_to_lines(cells: Vec<Vec<(char, Style)>>) -> Vec<Line<'static>> {
    cells
        .into_iter()
        .map(|row| {
            let mut spans: Vec<Span<'static>> = Vec::new();
            let mut text = String::new();
            let mut style: Option<Style> = None;
            for (ch, st) in row {
                if style != Some(st) {
                    if let Some(prev) = style {
                        spans.push(Span::styled(std::mem::take(&mut text), prev));
                    }
                    style = Some(st);
                }
                text.push(ch);
            }
            if let Some(prev) = style {
                spans.push(Span::styled(text, prev));
            }
            Line::from(spans)
        })
        .collect()
}

// ===== Net layout =====

/// Pixel geometry of one net layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NetGeom {
    /// Sticker size in pixels (columns × half-rows).
    sticker_w: usize,
    sticker_h: usize,
    /// Grout between two stickers of one face (pixels, both axes).
    gap: usize,
    /// Empty space between two faces (pixels, both axes).
    face_gap: usize,
    /// Whether center stickers carry their face letter.
    letters: bool,
}

/// Compact layout (scale 1): small tiles, still separated and grouped.
const COMPACT: NetGeom = NetGeom {
    sticker_w: 2,
    sticker_h: 1,
    gap: 1,
    face_gap: 2,
    letters: false,
};

/// Large layout (scale 2): roughly square 3×3-pixel tiles with face letters.
const LARGE: NetGeom = NetGeom {
    sticker_w: 3,
    sticker_h: 3,
    gap: 1,
    face_gap: 2,
    letters: true,
};

fn geom(scale: usize) -> NetGeom {
    if scale >= 2 {
        LARGE
    } else {
        COMPACT
    }
}

impl NetGeom {
    fn face_w(self) -> usize {
        3 * self.sticker_w + 2 * self.gap
    }

    fn face_h(self) -> usize {
        3 * self.sticker_h + 2 * self.gap
    }

    /// Pixel size of the whole net.
    fn px_size(self) -> (usize, usize) {
        let fc = NET_COLS / 3;
        let fr = NET_ROWS / 3;
        (
            fc * self.face_w() + (fc - 1) * self.face_gap,
            fr * self.face_h() + (fr - 1) * self.face_gap,
        )
    }

    /// Top-left pixel of the sticker at net grid position (`row`, `col`).
    fn origin(self, row: usize, col: usize) -> (usize, usize) {
        let x =
            (col / 3) * (self.face_w() + self.face_gap) + (col % 3) * (self.sticker_w + self.gap);
        let y =
            (row / 3) * (self.face_h() + self.face_gap) + (row % 3) * (self.sticker_h + self.gap);
        (x, y)
    }
}

/// Size in terminal cells (columns, rows) of the net drawn at `scale`.
pub fn net_size(scale: usize) -> (u16, u16) {
    let (w, h) = geom(scale).px_size();
    (w as u16, h.div_ceil(2) as u16)
}

/// Size of a bordered panel that holds the net at `scale`.
pub fn net_panel_size(scale: usize) -> (u16, u16) {
    let (w, h) = net_size(scale);
    (w + 2, h + 2)
}

/// The largest sticker scale (1 or 2) whose net fits inside `region` (a
/// bordered panel, so the inner area is two columns/rows smaller).
pub fn net_scale(region: Rect) -> usize {
    let (w, h) = net_panel_size(2);
    if region.width >= w && region.height >= h {
        2
    } else {
        1
    }
}

/// Build the lines that render the cube net using the given color scheme.
#[allow(dead_code)]
pub fn net_lines(cube: &Cube, scheme: &[Rgb; 6]) -> Vec<Line<'static>> {
    net_lines_scaled(cube, scheme, &[], 1)
}

/// Like [`net_lines`] but emphasises the given facelet indices (used by the
/// preview mode: all other stickers are dimmed) and uses the layout for
/// `scale` (1 = compact, 2 = large with face letters).
pub fn net_lines_scaled(
    cube: &Cube,
    scheme: &[Rgb; 6],
    highlight: &[usize],
    scale: usize,
) -> Vec<Line<'static>> {
    let g = geom(scale);
    let (pw, ph) = g.px_size();
    let mut buf = PixelBuf::new(pw, ph);
    let grid = net(cube);

    // Grout behind each face.
    for fr in 0..NET_ROWS / 3 {
        for fc in 0..NET_COLS / 3 {
            if matches!(grid[fr * 3][fc * 3], NetCell::Sticker { .. }) {
                let (x, y) = g.origin(fr * 3, fc * 3);
                buf.fill_rect(x, y, g.face_w(), g.face_h(), GROUT);
            }
        }
    }

    // Stickers, remembering where face letters go.
    let mut letters: Vec<(usize, usize, char, Rgb)> = Vec::new();
    for (r, row) in grid.iter().enumerate().take(NET_ROWS) {
        for (c, cell) in row.iter().enumerate().take(NET_COLS) {
            let NetCell::Sticker { color, facelet } = *cell else {
                continue;
            };
            let mut rgb = scheme[color as usize];
            if !highlight.is_empty() && !highlight.contains(&facelet) {
                rgb = scale_rgb(rgb, DIM);
            }
            let (x, y) = g.origin(r, c);
            buf.fill_rect(x, y, g.sticker_w, g.sticker_h, rgb);
            if g.letters && facelet % 9 == 4 {
                // A terminal row lying fully inside the sticker.
                let row = y.div_ceil(2);
                if row * 2 + 1 < y + g.sticker_h {
                    letters.push((row, x + g.sticker_w / 2, FACE_LETTERS[facelet / 9], rgb));
                }
            }
        }
    }

    let mut cells = buf.cells();
    for (row, col, letter, bg) in letters {
        cells[row][col] = (
            letter,
            Style::default()
                .fg(contrast_fg(bg))
                .bg(to_color(bg))
                .add_modifier(Modifier::BOLD),
        );
    }
    cells_to_lines(cells)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube::render::WCA_COLORS;
    use cube::Move;
    use std::str::FromStr;

    fn text(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The style of each cell of `line`, one per column.
    fn cell_styles(line: &Line) -> Vec<Style> {
        line.spans
            .iter()
            .flat_map(|s| std::iter::repeat_n(s.style, s.content.chars().count()))
            .collect()
    }

    #[test]
    fn solved_net_has_expected_size() {
        for scale in [1, 2] {
            let lines = net_lines_scaled(&Cube::solved(), &WCA_COLORS, &[], scale);
            let (w, h) = net_size(scale);
            assert_eq!(lines.len(), h as usize, "scale {scale}");
            assert!(
                lines.iter().all(|l| l.width() == w as usize),
                "scale {scale}"
            );
        }
    }

    #[test]
    fn moved_cube_changes_rendered_lines() {
        let solved = net_lines(&Cube::solved(), &WCA_COLORS);
        let mut c = Cube::solved();
        c.apply_move(Move::from_str("R").unwrap());
        let moved = net_lines(&c, &WCA_COLORS);
        assert_ne!(
            format!("{:?}", solved),
            format!("{:?}", moved),
            "R should change rendered net"
        );
    }

    #[test]
    fn large_layout_is_bigger_than_compact() {
        let (w1, h1) = net_size(1);
        let (w2, h2) = net_size(2);
        assert!(w2 > w1 && h2 > h1);
    }

    #[test]
    fn scale_two_needs_its_whole_panel() {
        let (w, h) = net_panel_size(2);
        assert_eq!(net_scale(Rect::new(0, 0, w, h)), 2);
        assert_eq!(net_scale(Rect::new(0, 0, w + 30, h + 10)), 2);
        assert_eq!(net_scale(Rect::new(0, 0, w - 1, h)), 1);
        assert_eq!(net_scale(Rect::new(0, 0, w, h - 1)), 1);
        assert_eq!(net_scale(Rect::new(0, 0, 26, 11)), 1);
    }

    #[test]
    fn lines_never_exceed_the_chosen_panel() {
        for w in 10..80u16 {
            for h in 5..30u16 {
                let region = Rect::new(0, 0, w, h);
                let scale = net_scale(region);
                if scale == 2 {
                    let lines = net_lines_scaled(&Cube::solved(), &WCA_COLORS, &[], 2);
                    assert!(lines.len() as u16 <= h - 2);
                    assert!(lines.iter().all(|l| l.width() as u16 <= w - 2));
                }
            }
        }
    }

    #[test]
    fn compact_fits_the_minimum_tools_panel() {
        // The medium dashboard at 80×24 leaves an 11-row, 50-column tools row.
        let (w, h) = net_panel_size(1);
        assert!(w <= 50 && h <= 13, "{w}x{h}");
    }

    #[test]
    fn adjacent_same_color_stickers_are_separated() {
        // U1 and U2 are both white on a solved cube; the columns between them
        // must be drawn differently from the stickers themselves.
        for scale in [1, 2] {
            let g = geom(scale);
            let lines = net_lines_scaled(&Cube::solved(), &WCA_COLORS, &[], scale);
            let (x0, y0) = g.origin(0, 3);
            let (x1, _) = g.origin(0, 4);
            let row = cell_styles(&lines[y0 / 2]);
            let sticker = row[x0];
            let border = row[x0 + g.sticker_w];
            assert!(x0 + g.sticker_w < x1);
            assert_ne!(sticker, border, "scale {scale}");
            assert_eq!(row[x1], sticker, "same color on both sides");
        }
    }

    #[test]
    fn face_gap_is_wider_than_sticker_gap() {
        for scale in [1, 2] {
            let g = geom(scale);
            let sticker_gap = g.origin(3, 1).0 - (g.origin(3, 0).0 + g.sticker_w);
            let face_gap = g.origin(3, 3).0 - (g.origin(3, 2).0 + g.sticker_w);
            assert!(face_gap > sticker_gap, "scale {scale}");
            let sticker_gap_v = g.origin(1, 3).1 - (g.origin(0, 3).1 + g.sticker_h);
            let face_gap_v = g.origin(3, 3).1 - (g.origin(2, 3).1 + g.sticker_h);
            assert!(face_gap_v > sticker_gap_v, "scale {scale}");
        }
    }

    #[test]
    fn large_layout_shows_face_letters() {
        let lines = net_lines_scaled(&Cube::solved(), &WCA_COLORS, &[], 2);
        let t = text(&lines);
        for letter in ['U', 'L', 'F', 'R', 'B', 'D'] {
            assert!(t.contains(letter), "missing {letter}\n{t}");
        }
        let compact = text(&net_lines_scaled(&Cube::solved(), &WCA_COLORS, &[], 1));
        assert!(!compact.chars().any(|c| c.is_ascii_alphabetic()));
    }

    #[test]
    fn face_letter_contrasts_with_its_sticker() {
        assert_eq!(contrast_fg(WCA_COLORS[0]), Color::Rgb(0x10, 0x10, 0x10));
        assert_eq!(contrast_fg(WCA_COLORS[5]), Color::Rgb(0xF5, 0xF5, 0xF5));
    }

    #[test]
    fn highlight_dims_everything_else() {
        let cube = Cube::solved();
        let plain = net_lines_scaled(&cube, &WCA_COLORS, &[], 2);
        let lit = net_lines_scaled(&cube, &WCA_COLORS, &(0..9).collect::<Vec<_>>(), 2);
        let g = geom(2);
        // A U sticker is unchanged, an F sticker is dimmed.
        let (ux, uy) = g.origin(0, 3);
        let (fx, fy) = g.origin(3, 3);
        assert_eq!(
            cell_styles(&plain[uy / 2])[ux],
            cell_styles(&lit[uy / 2])[ux]
        );
        assert_ne!(
            cell_styles(&plain[fy / 2])[fx],
            cell_styles(&lit[fy / 2])[fx]
        );
    }

    #[test]
    fn quadrant_patterns_map_to_glyphs() {
        let red = Rgb::new(255, 0, 0);
        let blue = Rgb::new(0, 0, 255);
        // (mask of red quarters: TL=1 TR=2 BL=4 BR=8, glyph with red in front)
        let expected = [
            (0b0001, '▘'),
            (0b0010, '▝'),
            (0b0011, '▀'),
            (0b0100, '▖'),
            (0b0101, '▌'),
            (0b0110, '▞'),
            (0b0111, '▛'),
            (0b1000, '▗'),
            (0b1001, '▚'),
            (0b1010, '▐'),
            (0b1011, '▜'),
            (0b1100, '▄'),
            (0b1101, '▙'),
            (0b1110, '▟'),
            (0b1111, '█'),
        ];
        for (mask, glyph) in expected {
            let px: [Option<Rgb>; 4] =
                std::array::from_fn(|i| Some(if mask & (1 << i) != 0 { red } else { blue }));
            // Red in front (or alone), blue behind.
            let (ch, style) = quadrant(px, red);
            assert_eq!(ch, glyph, "{mask:04b}");
            assert_eq!(style.fg, Some(to_color(red)), "{mask:04b}");
            if mask != 0b1111 {
                assert_eq!(style.bg, Some(to_color(blue)), "{mask:04b}");
            }
            // Transparent instead of blue: same glyph, no background.
            let px = px.map(|p| p.filter(|c| *c == red));
            let (ch, style) = quadrant(px, blue);
            assert_eq!((ch, style.bg), (glyph, None), "{mask:04b}");
        }
        assert_eq!(quadrant([None; 4], red), (' ', Style::default()));
    }

    #[test]
    fn quadrant_cells_fold_extra_colors_and_keep_favored() {
        let body = Rgb::new(10, 10, 10);
        let red = Rgb::new(255, 0, 0);
        let orange = Rgb::new(255, 120, 0);
        // Body, red, red, orange: body is kept, orange folds into red.
        let (ch, style) = quadrant([Some(body), Some(red), Some(red), Some(orange)], body);
        assert_eq!(ch, '▘');
        assert_eq!(style.fg, Some(to_color(body)));
        assert_eq!(style.bg, Some(to_color(red)));
        // Dimensions round up; pixels past the edge are transparent.
        let mut b = PixelBuf::new(5, 3);
        b.fill_rect(0, 0, 5, 3, red);
        let cells = b.quadrant_cells(body);
        assert_eq!((cells.len(), cells[0].len()), (2, 3));
        assert_eq!(cells[0][0].0, '█');
        assert_eq!(cells[0][2].0, '▌');
        assert_eq!(cells[1][2].0, '▘');
    }

    #[test]
    fn pixel_buffer_halfblocks() {
        let mut b = PixelBuf::new(3, 3);
        let red = Rgb::new(255, 0, 0);
        let blue = Rgb::new(0, 0, 255);
        b.set(0, 0, red);
        b.set(0, 1, red);
        b.set(1, 0, red);
        b.set(1, 1, blue);
        b.set(2, 1, blue);
        b.set(2, 2, blue);
        let cells = b.cells();
        assert_eq!(cells.len(), 2);
        assert_eq!(cells[0][0].0, '█');
        assert_eq!(cells[0][1].0, '▀');
        assert_eq!(cells[0][1].1.bg, Some(to_color(blue)));
        assert_eq!(cells[0][2].0, '▄');
        assert_eq!(
            cells[1][2].0, '▀',
            "odd height: last row has only a top pixel"
        );
        assert_eq!(cells[1][0].0, ' ');
    }
}
