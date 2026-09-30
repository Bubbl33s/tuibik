## MODIFIED Requirements

### Requirement: Interactive rotation
While the 3D view is open, the user SHALL have two kinds of rotation controls. **Fixed axis controls**: `x`/`X`, `y`/`Y` and `z`/`Z` SHALL turn the cube exactly 90° about the cube's own R–L, U–D and F–B axes respectively (whatever its current orientation), lowercase in one direction and uppercase in the opposite direction, so four presses return the cube to its starting orientation. Each turn SHALL be animated smoothly with easing over a short time (well under half a second) and SHALL end exactly on the 90° orientation; presses made during an animation SHALL chain without jumps. **Free controls**: the arrow keys and `h`/`j`/`k`/`l` SHALL rotate the cube immediately in small steps about the vertical and horizontal screen axes, with no limit that stops the cube from turning past the poles. `a` SHALL toggle continuous auto-spin, and `0` or `Home` SHALL reset the default orientation showing the U, F and R faces.

#### Scenario: Fixed axis turn
- **WHEN** the user presses `x` in the 3D view
- **THEN** the cube animates a 90° turn about its own R–L axis, and pressing `X` turns it back

#### Scenario: Axis follows the cube
- **WHEN** the user has freely rotated the cube and then presses `y`
- **THEN** the cube turns about its own U–D axis, not about the vertical axis of the screen

#### Scenario: Smooth animation
- **WHEN** a fixed axis turn is triggered
- **THEN** intermediate orientations are drawn and the motion eases in and out, ending exactly at the target

#### Scenario: Chained turns
- **WHEN** the user presses `z` twice quickly
- **THEN** the cube ends exactly 180° from its start, with no snap between the turns

#### Scenario: Four turns are identity
- **WHEN** the user presses `z` four times
- **THEN** the cube is back in its original orientation

#### Scenario: Rotate
- **WHEN** the user presses Left in the 3D view
- **THEN** the cube turns by a small angle about the vertical axis and the rendered image changes

#### Scenario: Through the poles
- **WHEN** the user holds Up until the cube has turned more than 90° about the horizontal axis
- **THEN** the cube keeps rotating smoothly and never stops or jumps

#### Scenario: Auto-spin
- **WHEN** auto-spin is enabled and time passes with no key pressed
- **THEN** the cube continues to rotate; pressing `a` again stops it

#### Scenario: Reset
- **WHEN** the user has rotated the cube by any combination of controls and presses `0`
- **THEN** the cube returns to the default orientation showing U, F and R faces

### Requirement: Discoverable key
The help overlay and the footer hints SHALL list the 3D view key and, while the view is open, its fixed axis controls, free rotation controls, spin, reset and close keys.

#### Scenario: Footer while open
- **WHEN** the 3D view is open
- **THEN** the footer lists the axis, free rotate, spin, reset and close keys

## ADDED Requirements

### Requirement: Responsive input
The 3D view SHALL stay responsive while keys are held or pressed rapidly: pending input SHALL be applied together before the next redraw so the displayed orientation never lags behind the keys pressed, and no key press SHALL be dropped or stuck.

#### Scenario: Key held down
- **WHEN** the user holds an arrow key for several seconds
- **THEN** the cube rotates continuously and stops promptly once the key is released, with no backlog of queued rotation

#### Scenario: Burst of keys
- **WHEN** the user presses many rotation keys in quick succession
- **THEN** the final orientation equals the combined effect of all presses

### Requirement: High-definition rendering
The 3D view SHALL draw the cube with finer than half-cell resolution (at least 2×2 sub-pixels per terminal cell), keep sticker shapes undistorted (square pixels), and keep the cube fully inside the drawing area at every orientation.

#### Scenario: Finer detail
- **WHEN** the cube is shown at a given terminal size
- **THEN** it is drawn with at least twice the horizontal pixel density of a half-block rendering

#### Scenario: Any orientation
- **WHEN** the cube is at any orientation, including after fixed and free rotations combined
- **THEN** no part of it is clipped by the edge of the drawing area

## REMOVED Requirements

### Requirement: Pitch clamp
**Reason**: Fixed axis controls and unconstrained free rotation make the clamp unnecessary and it prevented reaching the bottom face.
**Migration**: Use `x`/`X` for exact vertical flips; free rotation now continues through the poles.
