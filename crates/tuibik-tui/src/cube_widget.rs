//! Cube net widget: renders the unfolded cube as colored blocks.

use cube::render::{net, NetCell, Rgb, NET_COLS, NET_ROWS};
use cube::Cube;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::{Line, Span};

/// Two characters wide per sticker gives a roughly square cell in most fonts.
const STICKER: &str = "██";
const GAP: &str = "  ";

fn to_color(rgb: Rgb) -> Color {
    Color::Rgb(rgb.r, rgb.g, rgb.b)
}

/// The largest integer sticker scale (1 or 2) whose net fits inside `region`
/// (a bordered panel, so the inner area is two columns/rows smaller). Scale 1
/// is 2 columns × 1 row per sticker; scale 2 is 4 columns × 2 rows.
pub fn net_scale(region: Rect) -> usize {
    let inner_w = region.width.saturating_sub(2) as usize;
    let inner_h = region.height.saturating_sub(2) as usize;
    if inner_w >= NET_COLS * 4 && inner_h >= NET_ROWS * 2 {
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

/// Like [`net_lines`] but highlights the given facelet indices (used by the
/// preview mode) and repeats each sticker `scale` times in both directions.
pub fn net_lines_scaled(
    cube: &Cube,
    scheme: &[Rgb; 6],
    highlight: &[usize],
    scale: usize,
) -> Vec<Line<'static>> {
    let scale = scale.max(1);
    let sticker = STICKER.repeat(scale);
    let gap = GAP.repeat(scale);
    let grid = net(cube);
    let mut lines = Vec::with_capacity(NET_ROWS * scale);
    for row in grid.iter().take(NET_ROWS) {
        let mut spans: Vec<Span<'static>> = Vec::with_capacity(NET_COLS);
        for cell in row.iter().take(NET_COLS) {
            match cell {
                NetCell::Sticker { color, facelet } => {
                    let rgb = scheme[*color as usize];
                    let mut style = ratatui::style::Style::default().fg(to_color(rgb));
                    if highlight.contains(facelet) {
                        // Highlighted sticker: add a bold/underlined accent.
                        style = style
                            .add_modifier(ratatui::style::Modifier::BOLD)
                            .add_modifier(ratatui::style::Modifier::REVERSED);
                    }
                    spans.push(Span::styled(sticker.clone(), style));
                }
                NetCell::Gap => spans.push(Span::raw(gap.clone())),
            }
        }
        let line = Line::from(spans);
        for _ in 0..scale {
            lines.push(line.clone());
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube::render::WCA_COLORS;
    use cube::Move;
    use std::str::FromStr;

    #[test]
    fn solved_net_produces_nine_lines() {
        let lines = net_lines(&Cube::solved(), &WCA_COLORS);
        assert_eq!(lines.len(), NET_ROWS);
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
    fn scale_two_doubles_lines_and_width() {
        let one = net_lines_scaled(&Cube::solved(), &WCA_COLORS, &[], 1);
        let two = net_lines_scaled(&Cube::solved(), &WCA_COLORS, &[], 2);
        assert_eq!(two.len(), 2 * NET_ROWS);
        assert_eq!(two[0].width(), 2 * one[0].width());
    }

    #[test]
    fn region_50x20_uses_scale_two() {
        assert_eq!(net_scale(Rect::new(0, 0, 50, 20)), 2);
        let lines = net_lines_scaled(
            &Cube::solved(),
            &WCA_COLORS,
            &[],
            net_scale(Rect::new(0, 0, 50, 20)),
        );
        assert_eq!(lines.len(), 2 * NET_ROWS);
    }

    #[test]
    fn smaller_regions_use_scale_one() {
        assert_eq!(net_scale(Rect::new(0, 0, 49, 20)), 1);
        assert_eq!(net_scale(Rect::new(0, 0, 50, 19)), 1);
        assert_eq!(net_scale(Rect::new(0, 0, 26, 11)), 1);
    }
}
