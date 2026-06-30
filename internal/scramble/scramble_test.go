package scramble_test

import (
	"math/rand/v2"
	"strings"
	"testing"

	"github.com/user/tuibik/internal/scramble"
)

// newRng returns a deterministic RNG seeded from the given seed value.
func newRng(seed uint64) *rand.Rand {
	return rand.New(rand.NewPCG(seed, seed))
}

// validFaces is the set of legal face bytes.
var validFaces = map[byte]struct{}{
	'R': {}, 'L': {}, 'U': {}, 'D': {}, 'F': {}, 'B': {},
}

// validSuffixes is the set of legal move suffix strings.
var validSuffixes = map[string]struct{}{
	"":  {},
	"'": {},
	"2": {},
}

// axisOf maps a face to its axis index for constraint checking.
var axisOf = map[byte]int{
	'R': 0, 'L': 0,
	'U': 1, 'D': 1,
	'F': 2, 'B': 2,
}

// assertScramble verifies all invariants for a single scramble result.
func assertScramble(t *testing.T, moves []scramble.Move, label string) {
	t.Helper()

	if len(moves) != 20 {
		t.Errorf("%s: expected 20 moves, got %d", label, len(moves))
		return
	}

	for i, m := range moves {
		face := m.Face()
		if _, ok := validFaces[face]; !ok {
			t.Errorf("%s: move[%d] has invalid face byte %q", label, i, face)
		}

		s := string(m)
		if len(s) == 0 {
			t.Errorf("%s: move[%d] is empty", label, i)
			continue
		}

		suffix := s[1:]
		if _, ok := validSuffixes[suffix]; !ok {
			t.Errorf("%s: move[%d] has invalid suffix %q (full move: %q)", label, i, suffix, s)
		}
	}

	for i := 1; i < len(moves); i++ {
		prev := moves[i-1].Face()
		curr := moves[i].Face()
		if axisOf[curr] == axisOf[prev] {
			t.Errorf("%s: consecutive same-axis moves at positions %d and %d: %q %q",
				label, i-1, i, moves[i-1], moves[i])
		}
	}
}

// TestGenerate_Length verifies exactly 20 moves are returned.
func TestGenerate_Length(t *testing.T) {
	rng := newRng(42)
	moves := scramble.Generate(20, rng)
	if len(moves) != 20 {
		t.Errorf("expected 20 moves, got %d", len(moves))
	}
}

// TestGenerate_ValidFaces verifies every move has a legal face byte.
func TestGenerate_ValidFaces(t *testing.T) {
	rng := newRng(100)
	moves := scramble.Generate(20, rng)
	for i, m := range moves {
		if _, ok := validFaces[m.Face()]; !ok {
			t.Errorf("move[%d]: invalid face byte %q", i, m.Face())
		}
	}
}

// TestGenerate_ValidModifiers verifies every move has a legal suffix.
func TestGenerate_ValidModifiers(t *testing.T) {
	rng := newRng(200)
	moves := scramble.Generate(20, rng)
	for i, m := range moves {
		s := string(m)
		if len(s) == 0 {
			t.Errorf("move[%d]: empty move", i)
			continue
		}
		suffix := s[1:]
		if _, ok := validSuffixes[suffix]; !ok {
			t.Errorf("move[%d]: invalid suffix %q in move %q", i, suffix, s)
		}
	}
}

// TestGenerate_NoConsecutiveSameAxis verifies the consecutive-axis constraint.
func TestGenerate_NoConsecutiveSameAxis(t *testing.T) {
	rng := newRng(300)
	moves := scramble.Generate(20, rng)
	for i := 1; i < len(moves); i++ {
		prev := moves[i-1].Face()
		curr := moves[i].Face()
		if axisOf[curr] == axisOf[prev] {
			t.Errorf("consecutive same-axis moves at [%d,%d]: %q %q",
				i-1, i, moves[i-1], moves[i])
		}
	}
}

// TestGenerate_Format verifies Format produces a space-joined string of all moves.
func TestGenerate_Format(t *testing.T) {
	rng := newRng(400)
	moves := scramble.Generate(20, rng)
	formatted := scramble.Format(moves)

	parts := strings.Split(formatted, " ")
	if len(parts) != 20 {
		t.Errorf("Format: expected 20 space-separated tokens, got %d", len(parts))
	}

	for i, part := range parts {
		if part != string(moves[i]) {
			t.Errorf("Format: token[%d] = %q, want %q", i, part, string(moves[i]))
		}
	}
}

// TestGenerate_Move_Face verifies the Face() method returns the first byte.
func TestGenerate_Move_Face(t *testing.T) {
	cases := []struct {
		move scramble.Move
		want byte
	}{
		{"R", 'R'},
		{"U'", 'U'},
		{"F2", 'F'},
		{"L'", 'L'},
		{"D2", 'D'},
		{"B", 'B'},
	}

	for _, tc := range cases {
		if got := tc.move.Face(); got != tc.want {
			t.Errorf("Move(%q).Face() = %q, want %q", tc.move, got, tc.want)
		}
	}
}

// TestGenerate_Deterministic verifies that the same seed produces the same output.
func TestGenerate_Deterministic(t *testing.T) {
	seed := uint64(12345)
	a := scramble.Generate(20, newRng(seed))
	b := scramble.Generate(20, newRng(seed))

	if len(a) != len(b) {
		t.Fatalf("same seed produced different lengths: %d vs %d", len(a), len(b))
	}
	for i := range a {
		if a[i] != b[i] {
			t.Errorf("same seed diverged at move[%d]: %q vs %q", i, a[i], b[i])
		}
	}
}

// TestGenerate_AllConstraints runs Generate 1000 times and asserts all
// invariants hold for every result.
func TestGenerate_AllConstraints(t *testing.T) {
	for seed := uint64(0); seed < 1000; seed++ {
		rng := newRng(seed)
		moves := scramble.Generate(20, rng)
		assertScramble(t, moves, "seed="+string(rune('0'+seed%10)))
	}
}
