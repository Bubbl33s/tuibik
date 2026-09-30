# cube-3d-view Specification

## Purpose

Defines an interactive 3D rendering of the cube in the terminal, letting the user inspect all faces of the current cube state by rotating it.

## Requirements

### Requirement: 3D view shows the current cube
The system SHALL provide a 3D view rendering the cube in its current scrambled state, with each visible sticker colored according to the active color theme and adjacent stickers and cubies visibly separated.

#### Scenario: Scrambled state
- **WHEN** the 3D view is opened after a scramble is generated
- **THEN** the rendered stickers match the state shown in the 2D net

#### Scenario: Only front-facing surfaces
- **WHEN** the cube is rendered at any orientation
- **THEN** only faces turned toward the viewer are visible and nearer surfaces occlude farther ones

### Requirement: Open and close the 3D view
The user SHALL be able to open the 3D view from the idle dashboard with `v` and close it with `Esc` or `v`. The 3D view SHALL NOT be available while a solve is in progress.

#### Scenario: Open and close
- **WHEN** the user presses `v` on the idle dashboard, then `Esc`
- **THEN** the 3D view appears and then the dashboard returns unchanged

#### Scenario: Timer engaged
- **WHEN** the timer is arming, in inspection, or running
- **THEN** `v` does not open the 3D view

### Requirement: Interactive rotation
While the 3D view is open, arrow keys and `h`/`j`/`k`/`l` SHALL rotate the cube about the vertical and horizontal screen axes, `a` SHALL toggle continuous auto-spin, and `0` SHALL reset the default orientation. The pitch SHALL be clamped so the cube cannot flip upside down through the poles.

#### Scenario: Rotate
- **WHEN** the user presses Left in the 3D view
- **THEN** the cube turns about the vertical axis and the rendered image changes

#### Scenario: Auto-spin
- **WHEN** auto-spin is enabled and time passes with no key pressed
- **THEN** the cube continues to rotate; pressing `a` again stops it

#### Scenario: Reset
- **WHEN** the user has rotated the cube and presses `0`
- **THEN** the cube returns to the default orientation showing U, F and R faces

### Requirement: Adapts to terminal size
The 3D view SHALL scale to fit the available area, stay centered, and show a readable message instead of drawing when the area is too small.

#### Scenario: Resize
- **WHEN** the terminal is resized while the 3D view is open
- **THEN** the cube is re-fit to the new area without artifacts

#### Scenario: Tiny terminal
- **WHEN** the available area is smaller than the minimum drawable size
- **THEN** a "terminal too small" message is shown and no panic occurs

### Requirement: Discoverable key
The help overlay and the footer hints SHALL list the 3D view key and, while the view is open, its rotation controls.

#### Scenario: Footer while open
- **WHEN** the 3D view is open
- **THEN** the footer lists the rotate, spin, reset and close keys
