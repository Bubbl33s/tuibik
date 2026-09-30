# Spec Delta

## Purpose

Defines how the unfolded 2D cube net is drawn so that individual pieces, faces and orientation can be read at a glance in the terminal.

## ADDED Requirements

### Requirement: Stickers are visually separated
The net SHALL render a visible border or gap between every pair of adjacent stickers, so that neighboring stickers of the same color remain distinguishable.

#### Scenario: Solved cube
- **WHEN** the net of a solved cube is rendered
- **THEN** the nine stickers of each face are individually distinguishable rather than appearing as one solid block

#### Scenario: Compact size
- **WHEN** the panel is too small for the large net layout
- **THEN** the compact layout still separates stickers from each other

### Requirement: Faces are visually grouped
The net SHALL separate faces from one another more strongly than stickers within a face, so each 3×3 face reads as a unit.

#### Scenario: Face boundaries
- **WHEN** the net is rendered at any supported size
- **THEN** the separation between two faces is visibly larger than the separation between two stickers of one face

### Requirement: Orientation cues
The net SHALL mark each face's center sticker with its face letter (U, L, F, R, B, D) at sizes where a sticker is large enough to hold it.

#### Scenario: Large layout
- **WHEN** the net is rendered in the large layout
- **THEN** each center sticker shows its face letter, readable against the sticker color

### Requirement: Highlight and adaptive scale preserved
The net SHALL keep highlighting the stickers affected by the current scramble-preview step, and SHALL choose the largest layout that fits its panel, falling back to the compact layout otherwise.

#### Scenario: Preview highlight
- **WHEN** a scramble preview step is shown
- **THEN** the stickers moved by that step are visibly highlighted

#### Scenario: Fit
- **WHEN** the panel is resized
- **THEN** the net never overflows the panel; it uses the compact layout when the large one does not fit
