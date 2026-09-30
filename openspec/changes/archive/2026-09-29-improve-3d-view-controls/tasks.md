# Tasks

## 1. Orientation model

- [x] 1.1 Rework `View3d` (`app.rs`) to hold `current` and `target` rotation matrices: `rotate_free` (immediate, both matrices), `turn_axis(axis, reverse)` (right-multiplies target by an exact 90° matrix about the cube's own axis), `advance(now)` (eased axis-angle interpolation toward target, snapping when done) and `reset`; default equals the current default view. Verify with unit tests: four axis turns give the identity, `x` then `X` cancels, a turn after a free rotation is about the cube's own axis, animation ends exactly on the target with differing intermediate frames, chained turns end at 180°, reset restores default, matrix stays orthonormal after 1000 free rotations.
- [x] 1.2 Update `cube3d::render`/`render_lines`/`visible_faces` to take the matrix; port existing cube3d tests (default shows U/F/R, back-to-front painting, ≤3 faces visible at sampled orientations, stays inside buffer) and run `cargo test -p tuibik-tui`.

## 2. Controls

- [x] 2.1 Add `Input::ViewAxis(axis, reverse)` in `event.rs` for `x`/`X`/`z`/`Z`/`Y`; add classifier tests and confirm existing key tests still pass.
- [x] 2.2 Update `handle_cube3d_input` so axis turns are animated via `turn_axis` (with `Yes` acting as `y`), arrows/hjkl stay immediate unclamped free rotation, `a` spins about the vertical axis, `0`/`Home` reset; call `advance` from the tick. Verify with app tests for animated fixed turns, pass-through-pole rotation, and reset after mixed input.
- [x] 2.3 Update footer hints and help overlay in `ui.rs` to list axis, free, spin, reset and close keys; verify with the existing UI render tests / a new footer test.

## 3. Responsiveness

- [x] 3.1 Keep the drain-all-pending-events loop in `main.rs::run`; make `needs_fast_ticks` true while a turn animates and use a ~16 ms tick then. Verify with `cargo test` and a manual check that holding an arrow key stops immediately on release and turns animate at a steady frame rate.

## 4. Rendering quality

- [x] 4.1 Remove the edge anti-aliasing. Change `PixelBuf` to 2×2 sub-pixels per cell and add conversion to quadrant-block glyphs with two colors per cell in `cube3d.rs`/`cube_widget.rs`. Verify with tests: each of the 16 sub-pixel patterns maps to the right glyph, output dimensions are `cols × rows`, a solid sticker interior renders as full blocks of its exact shaded color, and the cube stays inside the buffer at all sampled orientations.
- [x] 4.2 Run the app (`/run`) and visually check default, oblique, upside-down and resized views and the 90° turn animation for clipping, color bleed or artifacts; run `cargo clippy` and `cargo test` for the whole workspace.
