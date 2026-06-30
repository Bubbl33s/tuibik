package history

import (
	"sort"
	"time"
)

// Solve represents a single completed solve attempt.
// Duration is 0 when DNF is true.
type Solve struct {
	Index    int
	Duration time.Duration
	Scramble string
	At       time.Time
	DNF      bool
}

// AoResult holds a trimmed average result.
// Duration is meaningful only when DNF is false.
type AoResult struct {
	Duration time.Duration
	DNF      bool
}

// Session stores solve records for the current session.
// Solves are appended in insertion order (oldest first).
// Session is not goroutine-safe; callers must serialize access.
type Session struct {
	solves []Solve
}

// New returns an empty Session.
func New() *Session {
	return &Session{}
}

// Add appends sv to the session. The internal slice grows without bound.
func (s *Session) Add(sv Solve) {
	s.solves = append(s.solves, sv)
}

// Count returns the total number of recorded solves.
func (s *Session) Count() int {
	return len(s.solves)
}

// Solves returns a copy of all recorded solves in newest-first order.
// The original internal slice is never modified.
func (s *Session) Solves() []Solve {
	n := len(s.solves)
	out := make([]Solve, n)
	for i, sv := range s.solves {
		out[n-1-i] = sv
	}
	return out
}

// Best returns the minimum Duration among non-DNF solves.
// Returns false when no non-DNF solves exist.
func (s *Session) Best() (time.Duration, bool) {
	var best time.Duration
	found := false
	for _, sv := range s.solves {
		if sv.DNF {
			continue
		}
		if !found || sv.Duration < best {
			best = sv.Duration
			found = true
		}
	}
	return best, found
}

// Mean returns the arithmetic mean of all non-DNF Duration values.
// Returns false when no non-DNF solves exist.
func (s *Session) Mean() (time.Duration, bool) {
	var total time.Duration
	count := 0
	for _, sv := range s.solves {
		if sv.DNF {
			continue
		}
		total += sv.Duration
		count++
	}
	if count == 0 {
		return 0, false
	}
	return total / time.Duration(count), true
}

// Average computes the WCA-style trimmed mean of the most recent `window` solves.
// Returns false when Count() < window.
//
// Algorithm:
//   - Take the last `window` solves.
//   - Sort a copy: non-DNF ascending by Duration, then all DNF at the end.
//   - Trim 1 best + 1 worst for any window; 5 best + 5 worst for window == 100.
//   - If any DNF survives in the trimmed body, the result is DNF.
//   - Otherwise the result is the arithmetic mean of the body durations.
func (s *Session) Average(window int) (AoResult, bool) {
	if s.Count() < window {
		return AoResult{}, false
	}

	last := s.solves[len(s.solves)-window:]
	cp := make([]Solve, window)
	copy(cp, last)

	// Sort: non-DNF ascending by Duration, DNF at the end.
	sort.Slice(cp, func(i, j int) bool {
		switch {
		case !cp[i].DNF && !cp[j].DNF:
			return cp[i].Duration < cp[j].Duration
		case cp[i].DNF && cp[j].DNF:
			return false
		default:
			return !cp[i].DNF // non-DNF before DNF
		}
	})

	trim := 1
	if window == 100 {
		trim = 5
	}
	body := cp[trim : window-trim]

	// Any DNF remaining in the body makes the result a DNF average.
	for _, sv := range body {
		if sv.DNF {
			return AoResult{DNF: true}, true
		}
	}

	var total time.Duration
	for _, sv := range body {
		total += sv.Duration
	}
	return AoResult{Duration: total / time.Duration(len(body))}, true
}
