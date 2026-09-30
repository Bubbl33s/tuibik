# Tasks

## 1. 2D net redesign

- [x] 1.1 Rework layout constants and `net_scale` in `cube_widget.rs` to include sticker borders and wider face gaps; verify `cargo test -p tuibik-tui` scale/fit tests (updated) pass and lines never exceed the panel
- [x] 1.2 Render sticker borders with block glyphs over a dark grout background at both scales, with a border-only fallback at scale 1; verify with a unit test that two adjacent same-color stickers produce differing border spans
- [x] 1.3 Draw face letters on center stickers at scale 2 with luminance-based contrasting color; verify a unit test finds U/L/F/R/B/D in the rendered lines
- [x] 1.4 Confirm preview highlight still works and visually check the net in the dashboard (`cargo run`) at compact and large sizes

## 2. 3D rendering core

- [x] 2.1 Create `cube3d.rs` with rotation (yaw/pitch), projection, back-face culling and depth-sorted triangle rasterizer into a pixel buffer; verify unit tests: solved cube at default angle shows exactly 3 face colors, and rotating 180° yaw changes the visible set
- [x] 2.2 Map `Cube::facelets()` and theme sticker colors onto inset sticker quads with black cubie gaps; verify a test that a moved cube renders a different buffer than a solved one
- [x] 2.3 Convert the pixel buffer to half-block (`▀`) `Line`s and fit/center to an area; verify tests for output dimensions and the too-small case (no panic)

## 3. Interaction and integration

- [x] 3.1 Add `Input::Toggle3d`, `ToggleSpin`, `ResetView` and `v`/`a`/`0` key classification in `event.rs`; verify classify tests
- [x] 3.2 Add `Overlay::Cube3D` and `View3d` state in `app.rs` with open (idle dashboard only), close, rotate, clamped pitch, spin toggle, reset and tick-driven auto-spin; verify unit tests including "not openable while timer active" and pitch clamp
- [x] 3.3 Render the overlay in `ui.rs`, add footer hints and help-overlay entries; verify ui tests (footer differs when open, tiny-terminal message, existing overlay-cycling tests updated to include the new overlay)
- [x] 3.4 Make the event loop poll quickly only while spinning; verify by running the app that idle CPU stays low with the view closed/not spinning

## 4. Verification

- [x] 4.1 Run `cargo test --workspace`, `cargo clippy --workspace -- -D warnings` and `cargo fmt --check`; all pass
- [x] 4.2 Manually run the app: scramble, open `v`, rotate with arrows/hjkl, spin, reset, resize, close; behavior matches the specs
