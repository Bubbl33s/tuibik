//! A tiny software rasterizer that draws the cube in 3D.
//!
//! The cube spans `[-1.5, 1.5]³` (one unit per cubie) with x to the right, y
//! up and z toward the viewer. Its orientation is a rotation matrix taking cube
//! coordinates to screen coordinates; the rotated cube is projected
//! orthographically, back-face culled and painted far-to-near into a
//! [`PixelBuf`] with 2×2 pixels per terminal cell, shown as quadrant-block
//! glyphs. A cell is about twice as tall as it is wide, so each pixel is too;
//! the projection stretches x by [`PIXEL_ASPECT`] to keep stickers square.

use cube::render::Rgb;
use cube::Cube;
use ratatui::text::Line;

use crate::cube_widget::{scale_rgb, PixelBuf};

/// Default orientation (yaw about y, then pitch about x): shows U, F and R.
const DEFAULT_YAW: f32 = -35.0;
const DEFAULT_PITCH: f32 = 25.0;
/// Smallest area (terminal cells) the view draws into.
pub const MIN_COLS: u16 = 16;
pub const MIN_ROWS: u16 = 8;

/// The black plastic between stickers.
const BODY: Rgb = Rgb::new(0x0C, 0x0C, 0x0E);
/// Half the side of a sticker (a cubie is 1 unit, so this leaves a gap).
const STICKER_HALF: f32 = 0.41;
const HALF: f32 = 1.5;
/// Height of a pixel over its width (half a cell each way).
const PIXEL_ASPECT: f32 = 2.0;

type V3 = [f32; 3];
type P2 = [f32; 2];

/// A rotation matrix mapping cube coordinates to screen coordinates.
pub type Mat3 = [[f32; 3]; 3];

/// A coordinate axis. On screen: x to the right, y up, z toward the viewer;
/// on the cube: x toward R, y toward U, z toward F.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

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

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// `m · p`.
fn apply(m: &Mat3, p: V3) -> V3 {
    [dot(m[0], p), dot(m[1], p), dot(m[2], p)]
}

/// `a · b`.
pub fn mat_mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let col = |j: usize| [b[0][j], b[1][j], b[2][j]];
    let mut out = [[0.0; 3]; 3];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = dot(a[i], col(j));
        }
    }
    out
}

/// Rotation about `axis` given the angle's cosine and sine (right-handed).
fn rotation_cs(axis: Axis, c: f32, s: f32) -> Mat3 {
    match axis {
        Axis::X => [[1.0, 0.0, 0.0], [0.0, c, -s], [0.0, s, c]],
        Axis::Y => [[c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c]],
        Axis::Z => [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]],
    }
}

/// Rotation by `deg` degrees about `axis` (counter-clockwise seen from +axis).
pub fn rotation(axis: Axis, deg: f32) -> Mat3 {
    let (s, c) = deg.to_radians().sin_cos();
    rotation_cs(axis, c, s)
}

/// An exact quarter turn about `axis`: clockwise seen from +axis, like the
/// whole-cube rotations x, y and z (applied about the cube's own axes by
/// right-multiplying the orientation), or the opposite way when `reverse`.
pub fn quarter_turn(axis: Axis, reverse: bool) -> Mat3 {
    rotation_cs(axis, 0.0, if reverse { 1.0 } else { -1.0 })
}

/// The orientation showing the U, F and R faces.
pub fn default_orientation() -> Mat3 {
    mat_mul(
        &rotation(Axis::X, DEFAULT_PITCH),
        &rotation(Axis::Y, DEFAULT_YAW),
    )
}

/// Undo floating-point drift: Gram–Schmidt on the rows.
pub fn orthonormalize(m: &Mat3) -> Mat3 {
    let norm = |v: V3| mul(v, 1.0 / dot(v, v).sqrt());
    let r0 = norm(m[0]);
    let r1 = norm(add(m[1], mul(r0, -dot(m[1], r0))));
    let r2 = [
        r0[1] * r1[2] - r0[2] * r1[1],
        r0[2] * r1[0] - r0[0] * r1[2],
        r0[0] * r1[1] - r0[1] * r1[0],
    ];
    [r0, r1, r2]
}

/// `mᵀ`, the inverse of a rotation.
pub fn transpose(m: &Mat3) -> Mat3 {
    [0, 1, 2].map(|i| [0, 1, 2].map(|j| m[j][i]))
}

/// Rotation by `rad` radians about the unit vector `k` (Rodrigues).
pub fn rotation_about(k: [f32; 3], rad: f32) -> Mat3 {
    let (s, c) = rad.sin_cos();
    let t = 1.0 - c;
    let [x, y, z] = k;
    [
        [c + t * x * x, t * x * y - s * z, t * x * z + s * y],
        [t * x * y + s * z, c + t * y * y, t * y * z - s * x],
        [t * x * z - s * y, t * y * z + s * x, c + t * z * z],
    ]
}

/// Unit axis and angle (radians, 0..=π) of the rotation `r`.
pub fn axis_angle(r: &Mat3) -> ([f32; 3], f32) {
    let trace = r[0][0] + r[1][1] + r[2][2];
    let angle = ((trace - 1.0) / 2.0).clamp(-1.0, 1.0).acos();
    let v = [r[2][1] - r[1][2], r[0][2] - r[2][0], r[1][0] - r[0][1]];
    let len = dot(v, v).sqrt();
    if len > 1e-4 {
        return (mul(v, 1.0 / len), angle);
    }
    if angle < 0.5 {
        return ([0.0, 1.0, 0.0], 0.0);
    }
    // A half turn: (r + I) / 2 = k·kᵀ; its largest-diagonal column is ∥ k.
    let i = (0..3)
        .max_by(|&a, &b| r[a][a].total_cmp(&r[b][b]))
        .unwrap_or(0);
    let col = [0, 1, 2].map(|j| (r[j][i] + if i == j { 1.0 } else { 0.0 }) / 2.0);
    (mul(col, 1.0 / dot(col, col).sqrt()), angle)
}

/// `v` rotated by `m`.
pub fn rotate_vec(m: &Mat3, v: [f32; 3]) -> [f32; 3] {
    apply(m, v)
}

/// Flat-shading brightness of a face: darker as it turns away from the viewer.
fn light(face: usize, m: &Mat3) -> f32 {
    0.6 + 0.4 * apply(m, FACES[face].0)[2]
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

/// The faces (URFDLB indices) turned toward the viewer, farthest first.
pub fn visible_faces(m: &Mat3) -> Vec<usize> {
    let mut faces: Vec<(usize, f32)> = FACES
        .iter()
        .enumerate()
        .map(|(i, (n, _, _))| (i, apply(m, *n)[2]))
        .filter(|&(_, z)| z > 1e-4)
        .collect();
    faces.sort_by(|a, b| a.1.total_cmp(&b.1));
    faces.into_iter().map(|(i, _)| i).collect()
}

/// Orthographic projection of the rotated cube, scaled to fit a `w × h`
/// pixel buffer and centered.
struct Camera {
    m: Mat3,
    /// Pixels per cube unit vertically (horizontally × [`PIXEL_ASPECT`]).
    scale: f32,
    center: P2,
}

impl Camera {
    fn new(m: &Mat3, w: usize, h: usize) -> Self {
        // The rotated cube always fits in a circle of radius HALF·√3.
        let radius = HALF * 3f32.sqrt();
        let fit_x = (w as f32 / 2.0 - 1.0) / PIXEL_ASPECT;
        let fit_y = h as f32 / 2.0 - 1.0;
        Camera {
            m: *m,
            scale: fit_x.min(fit_y).max(0.0) / radius,
            center: [w as f32 / 2.0, h as f32 / 2.0],
        }
    }

    /// Screen offset of the cube-space direction `v`.
    fn dir(&self, v: V3) -> P2 {
        let r = apply(&self.m, v);
        [r[0] * self.scale * PIXEL_ASPECT, -r[1] * self.scale]
    }

    /// Screen position of the cube-space point `p`.
    fn project(&self, p: V3) -> P2 {
        let d = self.dir(p);
        [self.center[0] + d[0], self.center[1] + d[1]]
    }
}

/// Corners of the square centered at `c` spanning `half` along `a`/`b`.
fn square(c: P2, a: P2, b: P2, half: f32) -> [P2; 4] {
    let at = |u: f32, v: f32| {
        [
            c[0] + (a[0] * u + b[0] * v) * half,
            c[1] + (a[1] * u + b[1] * v) * half,
        ]
    };
    [at(-1.0, -1.0), at(1.0, -1.0), at(1.0, 1.0), at(-1.0, 1.0)]
}

/// Fill the convex quad `q` (screen pixels) with `color`: every pixel whose
/// center is inside, with hard edges.
fn fill_quad(buf: &mut PixelBuf, q: [P2; 4], color: Rgb) {
    let cross = |a: P2, b: P2, p: P2| (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0]);
    let area = cross(q[0], q[1], q[2]) + cross(q[0], q[2], q[3]);
    if area.abs() < 1e-6 {
        return;
    }
    let sign = area.signum();
    let bound = |f: fn(f32, f32) -> f32, init: f32, k: usize| q.iter().map(|v| v[k]).fold(init, f);
    let lo =
        |k: usize, max: usize| (bound(f32::min, f32::MAX, k).floor().max(0.0) as usize).min(max);
    let hi =
        |k: usize, max: usize| (bound(f32::max, f32::MIN, k).ceil().max(0.0) as usize).min(max);
    for y in lo(1, buf.h)..hi(1, buf.h) {
        for x in lo(0, buf.w)..hi(0, buf.w) {
            let p = [x as f32 + 0.5, y as f32 + 0.5];
            if (0..4).all(|i| cross(q[i], q[(i + 1) % 4], p) * sign >= 0.0) {
                buf.set(x, y, color);
            }
        }
    }
}

/// Render `cube` at orientation `m` into a `w × h` pixel buffer, scaled to fit
/// and centered. Colors come from `scheme` (indexed by face color).
pub fn render(cube: &Cube, scheme: &[Rgb; 6], m: &Mat3, w: usize, h: usize) -> PixelBuf {
    let mut buf = PixelBuf::new(w, h);
    let cam = Camera::new(m, w, h);
    let facelets = cube.facelets();
    for face in visible_faces(m) {
        // Project the face's frame once; stickers are offsets within it.
        let (n, a, b) = FACES[face];
        let (a2, b2) = (cam.dir(a), cam.dir(b));
        let light = light(face, m);
        fill_quad(
            &mut buf,
            square(cam.project(mul(n, HALF)), a2, b2, HALF),
            BODY,
        );
        for i in face * 9..face * 9 + 9 {
            let color = scale_rgb(scheme[facelets[i] as usize], light);
            let c = cam.project(facelet_center(i));
            fill_quad(&mut buf, square(c, a2, b2, STICKER_HALF), color);
        }
    }
    buf
}

/// The 3D view as `rows` lines of `cols` quadrant-block cells, or `None` when the
/// area is below the minimum drawable size.
pub fn render_lines(
    cube: &Cube,
    scheme: &[Rgb; 6],
    m: &Mat3,
    cols: u16,
    rows: u16,
) -> Option<Vec<Line<'static>>> {
    if cols < MIN_COLS || rows < MIN_ROWS {
        return None;
    }
    let (w, h) = (cols as usize * 2, rows as usize * 2);
    Some(render(cube, scheme, m, w, h).quadrant_lines(BODY))
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

    /// Orientation from yaw (about y) then pitch (about x), in degrees.
    fn yaw_pitch(yaw: f32, pitch: f32) -> Mat3 {
        mat_mul(&rotation(Axis::X, pitch), &rotation(Axis::Y, yaw))
    }

    const IDENTITY: Mat3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

    /// Distinct non-body colors in a buffer.
    fn colors(buf: &PixelBuf) -> HashSet<(u8, u8, u8)> {
        buf.pixels()
            .iter()
            .flatten()
            .filter(|c| **c != BODY)
            .map(|c| (c.r, c.g, c.b))
            .collect()
    }

    /// The exact shaded sticker colors of a solved cube at orientation `m`.
    fn shaded(m: &Mat3) -> HashSet<(u8, u8, u8)> {
        visible_faces(m)
            .into_iter()
            .map(|f| scale_rgb(WCA_COLORS[f], light(f, m)))
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
    fn quarter_turns_are_exact() {
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            let q = quarter_turn(axis, false);
            let four = (0..4).fold(IDENTITY, |m, _| mat_mul(&q, &m));
            assert_eq!(four, IDENTITY, "{axis:?}");
            let back = mat_mul(&quarter_turn(axis, true), &q);
            assert_eq!(back, IDENTITY, "{axis:?}");
        }
        // x brings F to U, like the cube notation.
        assert_eq!(apply(&quarter_turn(Axis::X, false), FACES[2].0), FACES[0].0);
    }

    #[test]
    fn axis_angle_round_trips_through_rodrigues() {
        let close =
            |a: &Mat3, b: &Mat3| (0..9).all(|k| (a[k / 3][k % 3] - b[k / 3][k % 3]).abs() < 1e-5);
        for (axis, deg) in [
            ([0.0, 0.0, 1.0], 90.0),
            ([0.6, 0.0, 0.8], 33.0),
            ([0.0, 1.0, 0.0], 180.0),
            ([0.48, 0.6, 0.64], 179.0),
        ] {
            let r = rotation_about(axis, f32::to_radians(deg));
            let (k, angle) = axis_angle(&r);
            assert!((angle.to_degrees() - deg).abs() < 0.05, "{deg}");
            assert!(close(&rotation_about(k, angle), &r), "{axis:?} {deg}");
        }
        assert!(close(
            &rotation_about([1.0, 0.0, 0.0], 0.3),
            &rotation(Axis::X, 0.3f32.to_degrees())
        ));
        assert_eq!(axis_angle(&IDENTITY).1, 0.0);
        let m = default_orientation();
        assert!(close(&mat_mul(&m, &transpose(&m)), &IDENTITY));
    }

    #[test]
    fn default_view_shows_u_f_r() {
        let mut v = visible_faces(&default_orientation());
        v.sort();
        assert_eq!(v, vec![0, 1, 2]);
    }

    #[test]
    fn solved_cube_at_default_angle_shows_three_face_colors() {
        let m = default_orientation();
        let seen = colors(&render(&Cube::solved(), &WCA_COLORS, &m, 120, 60));
        // Exactly the shaded U, R and F colors: no blending.
        assert_eq!(seen, shaded(&m));
        assert_eq!(seen.len(), 3);
    }

    #[test]
    fn half_turn_changes_the_visible_set() {
        let m = default_orientation();
        let turned = yaw_pitch(DEFAULT_YAW + 180.0, DEFAULT_PITCH);
        let mut back = visible_faces(&turned);
        back.sort();
        assert_eq!(back, vec![0, 4, 5], "U, L, B");
        let a = colors(&render(&Cube::solved(), &WCA_COLORS, &m, 60, 60));
        let b = colors(&render(&Cube::solved(), &WCA_COLORS, &turned, 60, 60));
        assert_ne!(a, b);
    }

    #[test]
    fn nearer_faces_are_painted_last() {
        let m = default_orientation();
        let z: Vec<f32> = visible_faces(&m)
            .iter()
            .map(|&f| apply(&m, FACES[f].0)[2])
            .collect();
        assert!(z.windows(2).all(|w| w[0] <= w[1]), "{z:?}");
    }

    #[test]
    fn at_most_three_faces_are_ever_visible() {
        for yaw in (0..360).step_by(15) {
            for pitch in (-180..180).step_by(8) {
                for roll in [0.0, 30.0, 90.0] {
                    let m = mat_mul(
                        &rotation(Axis::Z, roll),
                        &yaw_pitch(yaw as f32, pitch as f32),
                    );
                    let v = visible_faces(&m);
                    assert!((1..=3).contains(&v.len()), "{yaw} {pitch} {roll}");
                }
            }
        }
    }

    #[test]
    fn moved_cube_renders_differently() {
        let m = default_orientation();
        let solved = render(&Cube::solved(), &WCA_COLORS, &m, 48, 48);
        let mut c = Cube::solved();
        c.apply_move(Move::from_str("R").unwrap());
        let moved = render(&c, &WCA_COLORS, &m, 48, 48);
        assert_ne!(solved, moved);
    }

    #[test]
    fn stickers_are_separated_by_the_body() {
        let buf = render(&Cube::solved(), &WCA_COLORS, &IDENTITY, 120, 60);
        // Looking straight at F: a horizontal scan through the middle crosses
        // sticker, body, sticker, body, sticker.
        let mut runs: Vec<Rgb> = (0..120).filter_map(|x| buf.get(x, 30)).collect();
        runs.dedup();
        assert_eq!(runs.len(), 7, "{runs:?}");
        assert_eq!(runs.iter().filter(|c| **c == BODY).count(), 4);
    }

    #[test]
    fn stickers_keep_square_proportions() {
        // Face-on, a sticker spans twice as many pixels across as down.
        let buf = render(&Cube::solved(), &WCA_COLORS, &IDENTITY, 120, 60);
        let center = scale_rgb(WCA_COLORS[2], light(2, &IDENTITY));
        let (w, h) = (120, 60);
        let across = (0..w)
            .filter(|&x| buf.get(x, h / 2) == Some(center))
            .count();
        let down = (0..h)
            .filter(|&y| buf.get(w / 2, y) == Some(center))
            .count();
        assert!(across > 0 && down > 0);
        let ratio = across as f32 / down as f32;
        assert!((ratio - PIXEL_ASPECT).abs() < 0.35, "{across}×{down}");
    }

    #[test]
    fn sticker_interior_is_full_blocks_of_its_exact_color() {
        let m = default_orientation();
        let (cols, rows) = (60u16, 30u16);
        let lines = render_lines(&Cube::solved(), &WCA_COLORS, &m, cols, rows).unwrap();
        assert_eq!(lines.len(), rows as usize);
        assert!(lines.iter().all(|l| l.width() == cols as usize));
        // The cell holding the U center sticker's center.
        let cam = Camera::new(&m, cols as usize * 2, rows as usize * 2);
        let p = cam.project(facelet_center(4));
        let (cx, cy) = ((p[0] / 2.0) as usize, (p[1] / 2.0) as usize);
        let line = &lines[cy];
        let mut x = 0;
        let span = line
            .spans
            .iter()
            .find(|s| {
                x += s.content.chars().count();
                x > cx
            })
            .unwrap();
        let shaded = scale_rgb(WCA_COLORS[0], light(0, &m));
        let c = crate::cube_widget::to_color(shaded);
        assert!(span.content.chars().all(|ch| ch == '█'), "{span:?}");
        assert_eq!(span.style.fg, Some(c));
    }

    #[test]
    fn finer_than_half_blocks() {
        let m = default_orientation();
        let lines = render_lines(&Cube::solved(), &WCA_COLORS, &m, 60, 30).unwrap();
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.to_string())
            .collect();
        // Quadrant glyphs (not just half blocks) appear along the edges.
        assert!(text.chars().any(|c| "▘▝▖▗▌▐▞▚▛▜▙▟".contains(c)), "{text}");
    }

    #[test]
    fn cube_stays_inside_the_buffer_at_any_angle() {
        let mut orientations = vec![];
        for yaw in (0..360).step_by(20) {
            for pitch in [-135.0, -89.0, -45.0, 0.0, 30.0, 89.0, 180.0] {
                orientations.push(yaw_pitch(yaw as f32, pitch));
            }
        }
        // Mixed fixed and free turns.
        let mut m = default_orientation();
        for axis in [Axis::X, Axis::Z, Axis::Y, Axis::X] {
            m = mat_mul(&m, &quarter_turn(axis, false));
            m = mat_mul(&rotation(Axis::Z, 37.0), &m);
            orientations.push(m);
        }
        for m in orientations {
            let buf = render(&Cube::solved(), &WCA_COLORS, &m, 30, 20);
            // Edges of the buffer stay empty (the cube is fit with margin).
            for x in 0..30 {
                assert!(buf.get(x, 0).is_none() && buf.get(x, 19).is_none());
            }
            for y in 0..20 {
                assert!(buf.get(0, y).is_none() && buf.get(29, y).is_none());
            }
        }
    }

    #[test]
    fn render_lines_dimensions_and_too_small() {
        let m = IDENTITY;
        let lines = render_lines(&Cube::solved(), &WCA_COLORS, &m, 40, 15).unwrap();
        assert_eq!(lines.len(), 15);
        assert!(lines.iter().all(|l| l.width() == 40));
        assert!(render_lines(&Cube::solved(), &WCA_COLORS, &m, MIN_COLS - 1, 20).is_none());
        assert!(render_lines(&Cube::solved(), &WCA_COLORS, &m, 40, MIN_ROWS - 1).is_none());
        assert!(render_lines(&Cube::solved(), &WCA_COLORS, &m, 0, 0).is_none());
        assert!(render_lines(&Cube::solved(), &WCA_COLORS, &m, MIN_COLS, MIN_ROWS).is_some());
    }
}
