# scramble-generation Specification

## Purpose

Defines the observable behaviour of scramble generation so the underlying solver library can be replaced without changing what users and the TUI receive.

## Requirements

### Requirement: Random-state scrambles
The system SHALL generate scrambles that, applied to a solved cube, produce a random solvable cube state, expressed as a move sequence in WCA notation.

#### Scenario: Scramble is valid and non-trivial
- **WHEN** a scramble is generated
- **THEN** it contains at least one move and applying it to a solved cube does not yield a solved cube

#### Scenario: Text and moves agree
- **WHEN** a scramble is generated
- **THEN** parsing its text yields exactly its move list

### Requirement: Scramble length bounds
Each generated scramble SHALL contain between 16 and 21 moves inclusive.

#### Scenario: Length over many scrambles
- **WHEN** 20 or more scrambles are generated from one scrambler
- **THEN** every scramble has 16 to 21 moves

### Requirement: Seeded determinism
A scrambler created with a given seed SHALL produce the same sequence of scrambles every time. Consecutive scrambles from one scrambler SHALL differ.

#### Scenario: Same seed, same output
- **WHEN** two scramblers are created with the same seed and each generates N scrambles
- **THEN** the two sequences are identical

#### Scenario: Consecutive scrambles differ
- **WHEN** one scrambler generates two scrambles
- **THEN** their texts differ

### Requirement: Facelet convention compatibility
The cube state obtained by applying a scramble with the project's cube model SHALL match the state the solver library derives from the same move sequence.

#### Scenario: Facelet strings match
- **WHEN** a scramble is applied by the project's cube model and by the solver library
- **THEN** both produce the same 54-character facelet string

### Requirement: Reusable tables
The system SHALL build solver tables once per scrambler and reuse them for all scrambles it generates.

#### Scenario: Repeated generation
- **WHEN** several scrambles are generated from one scrambler
- **THEN** table construction does not occur again per scramble
