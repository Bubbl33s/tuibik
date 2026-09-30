# Design

## Context

`View3d` stores `yaw` and `pitch`; `cube3d::render` rotates points by yaw then pitch and rasterizes triangles into a half-block `PixelBuf` (1×2 pixels per cell). `main::run` reads one crossterm event per loop iteration and redraws after each, so a held key (auto-repeat ~30/s) with a full software render per event builds a backlog and the view keeps moving after release. See proposal.md - Why.

## Goals / Non-Goals

**Goals:** roll-capable orientation, exact animated 90° turns about the cube's own axes, free small-step rotation, drained input, higher-resolution rendering.

**Non-Goals:** mouse dragging, applying real cube moves from the 3D view, changing the theme or color logic.

## Decisions

- **Orientation as a 3×3 rotation matrix** (`[[f32;3];3]`) instead of yaw/pitch. Yaw/pitch cannot express roll and forces a pole clamp. Free steps are left-multiplications about screen axes. Fixed axis turns are right-multiplications (about the cube's own axes), so `x`/`y`/`z` always follow the cube however it has been rotated. Alternative: quaternion — rejected, a matrix is what the projection needs anyway. The matrix is re-orthonormalized every N operations to stop drift. The *target* orientation is composed with exact integer 90° matrices so four turns are an exact identity.
- **Animation**: `View3d` keeps a `current` and a `target` matrix. A fixed turn multiplies the target; each tick the current orientation advances toward the target along the axis-angle path (ease-in-out, ~200 ms per 90°, computed as a rotation about the relative axis by an eased fraction of the remaining angle), and snaps to the target when done. Presses during an animation just compose onto the target, so turns chain smoothly. Free arrow steps apply to both matrices immediately (no easing) so they stay responsive. While animating, `needs_fast_ticks` is true and the loop tick drops to ~16 ms. No new dependency: the math is a few lines of 3×3 matrix code. Alternative: a tweening crate — rejected as overkill.
- **Default orientation** is expressed as the matrix equal to the current yaw −35° / pitch 25° so reset and the "U, F, R visible" behavior are unchanged.
- **Auto-spin** rotates about the screen vertical axis (same visual as today).
- **Fixed controls keys**: `x`/`X`, `z`/`Z`, `Y` map to new `Input::ViewAxis(axis, reverse)`. `y` already classifies as `Input::Yes` (delete confirm), so the 3D handler treats `Yes` as the `y` turn; no classifier conflict since 3D is a separate overlay. Alternative: brand-new keys — rejected, x/y/z match cube notation.
- **Reset**: `0` stays; `Home` (currently `JumpTop`) is handled as reset in the 3D overlay.
- **Free step size**: keep `VIEW_STEP_DEG` (15°) per press; with input drained, held keys feel continuous.
- **Input draining**: in `run`, after the first `poll`, loop `poll(Duration::ZERO)` and handle every queued event, then tick and redraw once. Applies to all screens, which also helps other lists. Alternative: throttle redraws — rejected, draining fixes the cause.
- **Rendering**: raise resolution with quadrant-block glyphs. `PixelBuf` becomes 2 pixels per cell horizontally and 2 vertically (still square). Each cell's 2×2 sub-pixels are reduced to two colors (foreground/background) and the matching glyph from `▘▝▀▖▌▞▛▗▚▐▜▄▙▟█` plus space. Face culling and flat shading stay as they are; the rotated basis is precomputed once per frame. No anti-aliasing blur. Alternatives: sextants (2×3, needs recent fonts), kitty/sixel graphics via `ratatui-image` (true pixels, but unsupported in Alacritty and others) — see Open Questions.
- **Fit**: radius stays HALF·√3 so any orientation fits.

## Risks / Trade-offs

- [Matrix drift after many free rotations] → re-orthonormalize (Gram–Schmidt) after each free step; axis turns snap entries to exact values.
- [A cell holds only two colors, so cell corners where 3+ colors meet can bleed] → pick the two most distinct colors per cell (body color favored) and check visually at oblique angles.
- [Animation at ~60 fps re-runs the rasterizer per frame] → precomputed basis and small buffers keep this cheap; measure in verification.
- [`Home` no longer "jump top" inside 3D] → only within the overlay, which has no list.
- [Draining events on all screens changes timing of hold-to-arm timer] → events are still processed in order with their original kinds; `tick` runs after the batch, and timer tests cover press/release.

## Open Questions

- Optional graphics-protocol backend (kitty/sixel via `ratatui-image`) with automatic fallback to quadrant blocks, for true-pixel definition on supporting terminals; can be added later without changing the specs.
