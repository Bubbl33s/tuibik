# Design

## Context

The TUI is ratatui 0.29 + crossterm. The net is built from `cube::render::net()` (9×12 cells of `Sticker{color,facelet}`/`Gap`) and drawn by `cube_widget::net_lines_scaled` as `██` spans at scale 1 or 2 (`net_scale`). The dashboard has an `Overlay` enum (Preview, Help, …) handled in `app.rs` and drawn in `ui.rs`. There is no mouse handling; the theme already assumes truecolor. `Up/Down/h/j/k/l` are already classified as `Input::Left/Right/Up/Down` in `event.rs`. See proposal.md for motivation.

## Goals / Non-Goals

**Goals:**
- Readable pieces in the 2D net without changing the `cube` crate.
- A smooth-enough interactive 3D view using only existing dependencies.

**Non-Goals:**
- Mouse drag rotation, animated face turns, or applying moves from the 3D view.
- Sixel/kitty graphics, textures or lighting beyond flat shading.
- Showing scramble-preview steps in 3D.

## Decisions

1. **Net borders via glyph + background, not extra grid cells.** Each sticker is drawn with the sticker color as foreground over a dark "grout" background using edge-inset half/eighth blocks (e.g. `▐█▌`-style edges at scale 2; a single dark column between stickers at scale 1). Faces are separated by a wider gap column/row than stickers. Rationale: keeps the `net()` data model and its tests untouched; only the widget changes. Alternative: insert gap cells into `net()` — rejected, it changes the model crate and the layout constants used by `net_scale`. Layout constants (columns/rows per sticker + gaps) move into `cube_widget` and `net_scale` is updated to match; a scale-1 fallback must still fit today's minimum panel where possible, otherwise it drops face gaps but keeps sticker borders.
2. **Face letter on centers** at scale 2 only (there is room for one glyph with contrasting fg/bg picked from sticker luminance).
3. **3D via a tiny software rasterizer in `cube3d.rs`.** Model: the 6 faces × 9 stickers as inset quads on a unit cube (sticker inset leaves black cubie gaps); a rotation matrix from yaw/pitch; orthographic-ish projection (mild perspective is optional); back-face culling; painter's algorithm (sort by depth) with scanline/edge-function triangle fill into a pixel buffer of `w × 2h` colors. Rationale: no dependency, deterministic, unit-testable as pure functions (buffer in, buffer out). Alternatives: Braille canvas (ratatui `Canvas`) — only 1 color per cell, colors bleed; a 3D crate — heavy for a single cube.
4. **Half-block output.** Each terminal cell renders `▀` with fg = top pixel, bg = bottom pixel, giving 2× vertical resolution and square-ish pixels. Buffer converted to `Vec<Line>` and drawn with `Paragraph` in the overlay area.
5. **State and input.** `App` gains `Overlay::Cube3D` and a `View3d { yaw, pitch, spin }`. Opened by `v` (new `Input::Toggle3d`), only from the idle dashboard branch (timing input already suppresses shortcuts). Reuses `Input::Left/Right/Up/Down` for rotation, `a` → `Input::ToggleSpin`, `0` → `Input::ResetView`; pitch clamped to ±89°. Default orientation yaw −35°, pitch 25° shows U/F/R.
6. **Auto-spin uses the existing tick.** When `spin` is on, `tick()` advances yaw by a time-based delta; the event loop must poll fast enough while this overlay is open (reuse whatever redraw cadence the timer already uses; add a short poll timeout only while spinning to avoid idle CPU otherwise).
7. **Sizing.** The view uses the whole dashboard body inside a bordered panel; cell aspect is treated as 1:2 (half-block pixels square). Cube size = min(width, 2·height)·k. Below a minimum area, draw the existing "too small" style message.

## Risks / Trade-offs

- [Terminals without truecolor render wrong colors] → Same assumption as the current theme; no new fallback.
- [Painter's algorithm mis-sorts at extreme angles] → For a convex cube with back-face culling, only ≤3 faces are visible and don't overlap, so sorting is exact within faces; within a face stickers don't overlap. Covered by a unit test.
- [Redraw cost while spinning] → Buffer is ~ (cols × 2·rows) ≈ tens of thousands of pixels; fill is cheap. Redraw only while the overlay is open and spinning.
- [Net glyph borders may look different across fonts] → Use standard block elements only; verify visually in the target terminal.
- [Scale-1 net loses width with borders] → Fall back to borders without extra face gaps rather than overflowing.
