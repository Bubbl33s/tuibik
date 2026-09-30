# Proposal

## Why

The unfolded cube net is a wall of solid color blocks: adjacent same-colored stickers merge into one blob, so individual pieces, face boundaries and orientation are hard to read (especially on a solved or near-solved cube). The user also wants to inspect the cube as a 3D object. That is feasible in a terminal with truecolor: a small software rasterizer drawing into half-block cells gives a usable interactive 3D view without any graphics protocol.

## What Changes

- Redesign the 2D net rendering: visible borders between every sticker, a stronger separation between faces, and orientation cues (face letter on each center sticker). Existing highlight behavior (scramble preview) and size-adaptive scaling are preserved.
- Add an interactive, rendered 3D view of the current cube (flat-shaded, back-face culled, painter's-algorithm rasterizer drawn with `▀` half-block truecolor cells).
- Add a 3D view overlay opened with `v` from the dashboard: arrow keys / `hjkl` rotate, `a` toggles auto-spin, `0` resets the orientation, `Esc`/`v` closes. It shows the live cube (and the previewed step while a scramble preview is open is out of scope).
- Add a small pure-Rust 3D math/raster module (no new dependencies).
- Update the help overlay and footer hints for the new key.

## Capabilities

### New Capabilities
- `cube-net-display`: How the unfolded 2D net is drawn: sticker/face separation, orientation cues, highlighting, adaptive scale.
- `cube-3d-view`: The interactive 3D cube view: what it shows, how it is opened/closed, and how it is rotated.

### Modified Capabilities

(none — no existing spec covers cube display)

## Impact

- `crates/tuibik-tui/src/cube_widget.rs` (net redesign), new `crates/tuibik-tui/src/cube3d.rs` (projection + rasterizer), `ui.rs` (render new overlay, help, footer), `app.rs` (new `Overlay::Cube3D`, rotation state, input handling), `event.rs` (`v` key and tick-driven auto-spin).
- No dependency changes; no changes to the `cube` crate model. Requires truecolor terminal (already assumed by the theme).
