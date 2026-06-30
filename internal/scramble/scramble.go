// Package scramble generates valid 20-move practice scrambles for 3x3 Rubik's cube.
//
// A generated scramble satisfies the consecutive-face constraint: no two
// adjacent moves may share the same axis (same face or opposite face).
package scramble

import (
	"math/rand/v2"
	"strings"
)

// Move is a single Rubik's cube move notation string, e.g. "R", "U'", "F2".
type Move string

// Face returns the face byte of the move: one of 'R', 'L', 'U', 'D', 'F', 'B'.
func (m Move) Face() byte {
	if len(m) == 0 {
		return 0
	}
	return m[0]
}

var faces = []byte{'R', 'L', 'U', 'D', 'F', 'B'}

// axis maps each face to its rotation axis.
// Faces on the same axis (same or opposite) are blocked from consecutive placement.
var axis = map[byte]int{
	'R': 0, 'L': 0,
	'U': 1, 'D': 1,
	'F': 2, 'B': 2,
}

var suffixes = []string{"", "'", "2"}

// Generate returns a slice of n moves forming a valid practice scramble.
// It uses rng for randomness so callers can inject a deterministic source
// for testing or a seeded source for production.
//
// The algorithm guarantees forward progress: at each position it resamples
// only the face until a legal (different-axis) face is found. With 6 faces
// and a maximum of 2 excluded faces per axis, at least 4 legal choices
// always remain, so the inner loop terminates quickly.
func Generate(n int, rng *rand.Rand) []Move {
	moves := make([]Move, 0, n)
	lastAxis := -1

	for len(moves) < n {
		f := faces[rng.IntN(6)]
		if lastAxis >= 0 && axis[f] == lastAxis {
			continue
		}
		suf := suffixes[rng.IntN(3)]
		moves = append(moves, Move(string(f)+suf))
		lastAxis = axis[f]
	}

	return moves
}

// Format returns the moves as a single space-joined string, e.g. "R U' F2 ...".
func Format(moves []Move) string {
	parts := make([]string, len(moves))
	for i, m := range moves {
		parts[i] = string(m)
	}
	return strings.Join(parts, " ")
}
