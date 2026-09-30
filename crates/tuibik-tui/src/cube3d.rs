//! A tiny software rasterizer that draws the cube in 3D.
//!
//! The cube spans `[-1.5, 1.5]³` (one unit per cubie) with x to the right, y
//! up and z toward the viewer. It is rotated by yaw (about the vertical axis)
//! and then pitch (about the horizontal screen axis), projected
//! orthographically, back-face culled and painted far-to-near into a
//! [`PixelBuf`] whose pixels are half a terminal row tall, so they come out
//! square in `▀` half-block cells.

use cube::render::Rgb;
use cube::Cube;
use ratatui::text::Line;

use crate::cube_widget::{scale_rgb, PixelBuf};

/// Default orientation: shows the U, F and R faces.
pub const DEFAULT_YAW: f32 = -35.0;
pub const DEFAULT_PITCH: f32 = 25.0;
/// Smallest area (terminal cells) the view draws into.
pub const MIN_COLS: u16 = 16;
pub const MIN_ROWS: u16 = 8;

/// The black plastic between stickers.
const BODY: Rgb = Rgb::new(0x0C, 0x0C, 0x0E);
/// Half the side of a sticker (a cubie is 1 unit, so this leaves a gap).
const STICKER_HALF: f32 = 0.41;
const HALF: f32 = 1.5;

type V3 = [f32; 3];

/// Per face (URFDLB): outward normal, column axis and row axis, matching the
/// row-major facelet order of each face in the standard net.
const FACES: [(V3, V3, V3); 6] = [
    ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]), // U
    ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0]), // R
    ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]), // F
    ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]), // D
    ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]), // L
    ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]), // B
];

fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn mul(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

/// Rotate `p` by `yaw` about y, then by `pitch` about x (degrees).
fn rotate(p: V3, yaw: f32, pitch: f32) -> V3 {
    let (sy, cy) = yaw.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    let x = p[0] * cy + p[2] * sy;
    let z = -p[0] * sy + p[2] * cy;
    let y = p[1];
    [x, y * cp - z * sp, y * sp + z * cp]
}

/// Flat-shading brightness of a face: darker as it turns away from the viewer.
fn light(face: usize, yaw: f32, pitch: f32) -> f32 {
    0.6 + 0.4 * rotate(FACES[face].0, yaw, pitch)[2]
}

/// Center of facelet `index` (0..54) on the unrotated cube.
fn facelet_center(index: usize) -> V3 {
    let (n, a, b) = FACES[index / 9];
    let (r, c) = ((index % 9) / 3, index % 3);
    add(
        mul(n, HALF),
        add(mul(a, c as f32 - 1.0), mul(b, r as f32 - 1.0)),
    )
}

/// Corners of the square centered at `center` spanning `half` along `a`/`b`.
fn square(center: V3, a: V3, b: V3, half: f32) -> [V3; 4] {
    let (a, b) = (mul(a, half), mul(b, half));
    let neg = |v: V3| mul(v, -1.0);
    [
        add(center, add(neg(a), neg(b))),
        add(center, add(a, neg(b))),
        add(center, add(a, b)),
        add(center, add(neg(a), b)),
    ]
}

/// The faces (URFDLB indices) turned toward the viewer, farthest first.
pub fn visible_faces(yaw: f32, pitch: f32) -> Vec<usize> {
    let mut faces: Vec<(usize, f32)> = FACES
        .iter()
        .enumerate()
        .map(|(i, (n, _, _))| (i, rotate(*n, yaw, pitch)[2]))
        .filter(|&(_, z)| z > 1e-4)
        .collect();
    faces.sort_by(|a, b| a.1.total_cmp(&b.1));
    faces.into_iter().map(|(i, _)| i).collect()
}

/// Fill the triangle `p` (screen pixels) with `color`.
fn fill_triangle(buf: &mut PixelBuf, p: [[f32; 2]; 3], color: Rgb) {
    let edge = |a: [f32; 2], b: [f32; 2], x: f32, y: f32| {
        (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0])
    };
    let min_x = p
        .iter()
        .map(|v| v[0])
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.0) as usize;
    let min_y = p
        .iter()
        .map(|v| v[1])
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.0) as usize;
    let max_x = (p.iter().map(|v| v[0]).fold(f32::MIN, f32::max).ceil() as isize)
        .clamp(0, buf.w as isize) as usize;
    let max_y = (p.iter().map(|v| v[1]).fold(f32::MIN, f32::max).ceil() as isize)
        .clamp(0, buf.h as isize) as usize;
    for y in min_y..max_y {
        for x in min_x..max_x {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let w0 = edge(p[0], p[1], fx, fy);
            let w1 = edge(p[1], p[2], fx, fy);
            let w2 = edge(p[2], p[0], fx, fy);
            let inside =
                (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0);
            if inside {
                buf.set(x, y, color);
            }
        }
    }
}

/// Render `cube` at the given orientation into a `w × h` pixel buffer, scaled
/// to fit and centered. Colors come from `scheme` (indexed by face color).
pub fn render(
    cube: &Cube,
    scheme: &[Rgb; 6],
    yaw: f32,
    pitch: f32,
    w: usize,
    h: usize,
) -> PixelBuf {
    let mut buf = PixelBuf::new(w, h);
    // The rotated cube always fits in a circle of radius HALF·√3.
    let radius = HALF * 3f32.sqrt();
    let scale = ((w.min(h) as f32) / 2.0 - 1.0).max(0.0) / radius;
    let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
    let project = |p: V3| {
        let r = rotate(p, yaw, pitch);
        [cx + r[0] * scale, cy - r[1] * scale]
    };
    let mut fill_quad = |corners: [V3; 4], color: Rgb| {
        let q = corners.map(project);
        fill_triangle(&mut buf, [q[0], q[1], q[2]], color);
        fill_triangle(&mut buf, [q[0], q[2], q[3]], color);
    };

    let facelets = cube.facelets();
    for face in visible_faces(yaw, pitch) {
        let (n, a, b) = FACES[face];
        let light = light(face, yaw, pitch);
        fill_quad(square(mul(n, HALF), a, b, HALF), BODY);
        for i in face * 9..face * 9 + 9 {
            let color = scale_rgb(scheme[facelets[i] as usize], light);
            fill_quad(square(facelet_center(i), a, b, STICKER_HALF), color);
        }
    }
    buf
}

/// The 3D view as `rows` lines of `cols` half-block cells, or `None` when the
/// area is below the minimum drawable size.
pub fn render_lines(
    cube: &Cube,
    scheme: &[Rgb; 6],
    yaw: f32,
    pitch: f32,
    cols: u16,
    rows: u16,
) -> Option<Vec<Line<'static>>> {
    if cols < MIN_COLS || rows < MIN_ROWS {
        return None;
    }
    Some(render(cube, scheme, yaw, pitch, cols as usize, rows as usize * 2).lines())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cube_widget::scale_rgb;
    use cube::render::WCA_COLORS;
    use cube::Move;
    use std::collections::HashSet;
    use std::str::FromStr;

    fn dist(a: V3, b: V3) -> f32 {
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    /// Distinct non-body colors in a buffer.
    fn colors(buf: &PixelBuf) -> HashSet<(u8, u8, u8)> {
        buf.pixels()
            .iter()
            .flatten()
            .filter(|c| **c != BODY)
            .map(|c| (c.r, c.g, c.b))
            .collect()
    }

    #[test]
    fn stickers_of_one_piece_meet_at_its_corner() {
        // URF corner: U9 R1 F3; UF edge: U8 F2; DRB corner: D9 R9 B7.
        for piece in [&[8, 9, 20][..], &[7, 19], &[35, 17, 51]] {
            for &i in piece {
                for &j in piece {
                    assert!(dist(facelet_center(i), facelet_center(j)) < 0.8, "{i} {j}");
                }
            }
        }
        // All 54 centers are distinct.
        for i in 0..54 {
            for j in i + 1..54 {
                assert!(dist(facelet_center(i), facelet_center(j)) > 0.5);
            }
        }
    }

    #[test]
    fn default_view_shows_u_f_r() {
        let mut v = visible_faces(DEFAULT_YAW, DEFAULT_PITCH);
        v.sort();
        assert_eq!(v, vec![0, 1, 2]);
    }

    #[test]
    fn solved_cube_at_default_angle_shows_three_face_colors() {
        let buf = render(
            &Cube::solved(),
            &WCA_COLORS,
            DEFAULT_YAW,
            DEFAULT_PITCH,
            60,
            60,
        );
        let seen = colors(&buf);
        assert_eq!(seen.len(), 3, "{seen:?}");
        // They are shaded versions of U, R and F.
        for face in [0, 1, 2] {
            let c = scale_rgb(WCA_COLORS[face], light(face, DEFAULT_YAW, DEFAULT_PITCH));
            assert!(seen.contains(&(c.r, c.g, c.b)), "face {face}");
        }
    }

    #[test]
    fn half_turn_yaw_changes_the_visible_set() {
        let front = visible_faces(DEFAULT_YAW, DEFAULT_PITCH);
        let back = visible_faces(DEFAULT_YAW + 180.0, DEFAULT_PITCH);
        let mut back_sorted = back.clone();
        back_sorted.sort();
        assert_eq!(back_sorted, vec![0, 4, 5], "U, L, B");
        let a = colors(&render(
            &Cube::solved(),
            &WCA_COLORS,
            DEFAULT_YAW,
            DEFAULT_PITCH,
            60,
            60,
        ));
        let b = colors(&render(
            &Cube::solved(),
            &WCA_COLORS,
            DEFAULT_YAW + 180.0,
            DEFAULT_PITCH,
            60,
            60,
        ));
        assert_ne!(a, b);
        assert_ne!(front, back);
    }

    #[test]
    fn nearer_faces_are_painted_last() {
        let order = visible_faces(DEFAULT_YAW, DEFAULT_PITCH);
        let z: Vec<f32> = order
            .iter()
            .map(|&f| rotate(FACES[f].0, DEFAULT_YAW, DEFAULT_PITCH)[2])
            .collect();
        assert!(z.windows(2).all(|w| w[0] <= w[1]), "{z:?}");
    }

    #[test]
    fn at_most_three_faces_are_ever_visible() {
        for yaw in (0..360).step_by(15) {
            for pitch in (-89..=89).step_by(8) {
                let v = visible_faces(yaw as f32, pitch as f32);
                assert!((1..=3).contains(&v.len()), "{yaw} {pitch}");
            }
        }
    }

    #[test]
    fn moved_cube_renders_differently() {
        let solved = render(
            &Cube::solved(),
            &WCA_COLORS,
            DEFAULT_YAW,
            DEFAULT_PITCH,
            48,
            48,
        );
        let mut c = Cube::solved();
        c.apply_move(Move::from_str("R").unwrap());
        let moved = render(&c, &WCA_COLORS, DEFAULT_YAW, DEFAULT_PITCH, 48, 48);
        assert_ne!(solved, moved);
    }

    #[test]
    fn stickers_are_separated_by_the_body() {
        let buf = render(&Cube::solved(), &WCA_COLORS, 0.0, 0.0, 60, 60);
        // Looking straight at F: a horizontal scan through the middle crosses
        // sticker, body, sticker, body, sticker.
        let row: Vec<Option<Rgb>> = (0..60).map(|x| buf.get(x, 30)).collect();
        let mut runs = row.clone();
        runs.dedup();
        let runs: Vec<_> = runs.into_iter().flatten().collect();
        assert_eq!(runs.len(), 7, "{runs:?}");
        assert_eq!(runs.iter().filter(|c| **c == BODY).count(), 4);
    }

    #[test]
    fn cube_stays_inside_the_buffer_at_any_angle() {
        for yaw in (0..360).step_by(20) {
            for pitch in [-89.0, -45.0, 0.0, 30.0, 89.0] {
                let buf = render(&Cube::solved(), &WCA_COLORS, yaw as f32, pitch, 30, 20);
                // Edges of the buffer stay empty (the cube is fit with margin).
                for x in 0..30 {
                    assert!(buf.get(x, 0).is_none() && buf.get(x, 19).is_none());
                }
            }
        }
    }

    #[test]
    fn render_lines_dimensions_and_too_small() {
        let lines = render_lines(&Cube::solved(), &WCA_COLORS, 0.0, 0.0, 40, 15).unwrap();
        assert_eq!(lines.len(), 15);
        assert!(lines.iter().all(|l| l.width() == 40));
        assert!(render_lines(&Cube::solved(), &WCA_COLORS, 0.0, 0.0, MIN_COLS - 1, 20).is_none());
        assert!(render_lines(&Cube::solved(), &WCA_COLORS, 0.0, 0.0, 40, MIN_ROWS - 1).is_none());
        assert!(render_lines(&Cube::solved(), &WCA_COLORS, 0.0, 0.0, 0, 0).is_none());
        assert!(render_lines(&Cube::solved(), &WCA_COLORS, 0.0, 0.0, MIN_COLS, MIN_ROWS).is_some());
    }
}
