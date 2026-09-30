## Why

The 3D cube view is hard to use: arrows only offer yaw/pitch (no roll, and pitch is clamped), so reaching a specific face is fiddly. Holding an arrow key also makes the view lag and "stick", because each key event triggers a full software re-render and queued events pile up.

## What Changes

- Add **fixed axis controls**: `x`/`X`, `y`/`Y`, `z`/`Z` turn the whole cube exactly 90° about the cube's own R–L, U–D and F–B axes (lowercase = one direction, uppercase = the opposite), like whole-cube rotations in cube notation. Each turn is animated with easing instead of jumping.
- Keep the arrows (and `h`/`j`/`k`/`l`) as **free rotation** in small steps, now unclamped (the pole clamp goes away as orientation gains roll).
- Keep `0` as reset and add `Home` as an alias; reset restores the default orientation.
- Fix the arrow-key stalls: drain all pending input events before each redraw, so held keys never build a backlog.
- Improve rendering: higher resolution using quadrant-block glyphs (2×2 sub-pixels per terminal cell instead of 1×2) and cheaper per-frame cost.
- Update footer and help overlay with the new controls.
- **BREAKING**: the pitch clamp requirement is removed; the cube can now be rotated freely through the poles.

## Capabilities

### New Capabilities

_None._

### Modified Capabilities

- `cube-3d-view`: interactive rotation requirement is replaced by fixed axis controls plus free rotation and reset; input responsiveness requirement added; rendering quality and discoverability requirements updated.

## Impact

- `crates/tuibik-tui/src/cube3d.rs`: orientation as a rotation matrix, animated turns, quadrant-block high-resolution rendering.
- `crates/tuibik-tui/src/app.rs`: `View3d` state and `handle_cube3d_input`.
- `crates/tuibik-tui/src/event.rs`: new axis-rotation inputs.
- `crates/tuibik-tui/src/main.rs`: event loop drains queued events.
- `crates/tuibik-tui/src/ui.rs`: footer/help text.
- No new dependencies.
