package history

import (
	"testing"
	"time"
)

// mkSolve is a helper that creates a non-DNF Solve with the given duration.
func mkSolve(d time.Duration) Solve {
	return Solve{Duration: d}
}

// mkDNF is a helper that creates a DNF Solve.
func mkDNF() Solve {
	return Solve{DNF: true}
}

// buildSession creates a Session pre-loaded with the given solves (in slice order, oldest first).
func buildSession(solves []Solve) *Session {
	s := New()
	for _, sv := range solves {
		s.Add(sv)
	}
	return s
}

// ── Average ───────────────────────────────────────────────────────────────────

func TestAverage_InsufficientCount(t *testing.T) {
	tests := []struct {
		name   string
		solves []Solve
		window int
	}{
		{
			name:   "ao5 with 4 solves returns false",
			window: 5,
			solves: []Solve{mkSolve(1 * time.Second), mkSolve(2 * time.Second), mkSolve(3 * time.Second), mkSolve(4 * time.Second)},
		},
		{
			name:   "ao100 with 99 solves returns false",
			window: 100,
			solves: func() []Solve {
				sv := make([]Solve, 99)
				for i := range sv {
					sv[i] = mkSolve(time.Duration(i+1) * time.Second)
				}
				return sv
			}(),
		},
		{
			name:   "ao12 with 11 solves returns false",
			window: 12,
			solves: func() []Solve {
				sv := make([]Solve, 11)
				for i := range sv {
					sv[i] = mkSolve(time.Duration(i+1) * time.Second)
				}
				return sv
			}(),
		},
		{
			name:   "zero solves ao5 returns false",
			window: 5,
			solves: nil,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			s := buildSession(tt.solves)
			_, ok := s.Average(tt.window)
			if ok {
				t.Errorf("expected false (insufficient solves), got true")
			}
		})
	}
}

func TestAverage_Ao5_Numeric(t *testing.T) {
	// SCEN-06: [10s, 9s, 8s, 7s, 6s] — no DNF.
	// Sorted:  [6s, 7s, 8s, 9s, 10s]
	// Trim 1 best (6s) + 1 worst (10s) → body [7s, 8s, 9s]
	// Mean = 24s / 3 = 8s
	solves := []Solve{
		mkSolve(10 * time.Second),
		mkSolve(9 * time.Second),
		mkSolve(8 * time.Second),
		mkSolve(7 * time.Second),
		mkSolve(6 * time.Second),
	}
	s := buildSession(solves)
	got, ok := s.Average(5)
	if !ok {
		t.Fatal("expected ok=true, got false")
	}
	if got.DNF {
		t.Error("expected DNF=false")
	}
	want := 8 * time.Second
	if got.Duration != want {
		t.Errorf("ao5 duration: got %v, want %v", got.Duration, want)
	}
}

func TestAverage_Ao12_Numeric(t *testing.T) {
	// 12 solves: 1s..12s (oldest-first).
	// Sorted:  [1s, 2s, ..., 12s]
	// Trim 1 best (1s) + 1 worst (12s) → body [2s..11s] (10 values)
	// Sum = 2+3+…+11 = 65; Mean = 65s / 10 = 6.5s
	solves := make([]Solve, 12)
	for i := range solves {
		solves[i] = mkSolve(time.Duration(i+1) * time.Second)
	}
	s := buildSession(solves)
	got, ok := s.Average(12)
	if !ok {
		t.Fatal("expected ok=true, got false")
	}
	if got.DNF {
		t.Error("expected DNF=false")
	}
	want := 65 * time.Second / 10 // 6.5s
	if got.Duration != want {
		t.Errorf("ao12 duration: got %v, want %v", got.Duration, want)
	}
}

func TestAverage_Ao100_Trim5(t *testing.T) {
	// 100 solves: 1s..100s (oldest-first).
	// Sorted:  [1s, 2s, ..., 100s]
	// Trim 5 best (1–5s) + 5 worst (96–100s) → body [6s..95s] (90 values)
	// Sum = 6+7+…+95 = 4545s; Mean = 4545s / 90 = 50.5s
	solves := make([]Solve, 100)
	for i := range solves {
		solves[i] = mkSolve(time.Duration(i+1) * time.Second)
	}
	s := buildSession(solves)
	got, ok := s.Average(100)
	if !ok {
		t.Fatal("expected ok=true, got false")
	}
	if got.DNF {
		t.Error("expected DNF=false")
	}
	want := 4545 * time.Second / 90 // 50.5s
	if got.Duration != want {
		t.Errorf("ao100 duration: got %v, want %v", got.Duration, want)
	}
}

func TestAverage_SingleDNF_DiscardedAsWorst(t *testing.T) {
	// SCEN-08: [10s, 9s, 8s, 7s, DNF]
	// Sorted:  [7s, 8s, 9s, 10s, DNF]
	// Trim 1 best (7s) + 1 worst (DNF) → body [8s, 9s, 10s]
	// No DNF in body → DNF=false; Mean = 27s / 3 = 9s
	solves := []Solve{
		mkSolve(10 * time.Second),
		mkSolve(9 * time.Second),
		mkSolve(8 * time.Second),
		mkSolve(7 * time.Second),
		mkDNF(),
	}
	s := buildSession(solves)
	got, ok := s.Average(5)
	if !ok {
		t.Fatal("expected ok=true, got false")
	}
	if got.DNF {
		t.Error("expected DNF=false: single DNF must be discarded as worst")
	}
	want := 9 * time.Second
	if got.Duration != want {
		t.Errorf("ao5 duration: got %v, want %v", got.Duration, want)
	}
}

func TestAverage_DNFInBodyAfterTrim(t *testing.T) {
	// SCEN-09: [10s, 9s, DNF, DNF, DNF]
	// Sorted:  [9s, 10s, DNF, DNF, DNF]
	// Trim 1 best (9s) + 1 worst (one DNF) → body [10s, DNF, DNF]
	// DNF in body → AoResult.DNF = true
	solves := []Solve{
		mkSolve(10 * time.Second),
		mkSolve(9 * time.Second),
		mkDNF(),
		mkDNF(),
		mkDNF(),
	}
	s := buildSession(solves)
	got, ok := s.Average(5)
	if !ok {
		t.Fatal("expected ok=true, got false")
	}
	if !got.DNF {
		t.Error("expected DNF=true when DNF survives the trim")
	}
}

func TestAverage_MultipleDNFs_ProduceDNFResult(t *testing.T) {
	tests := []struct {
		name   string
		solves []Solve
		window int
	}{
		{
			name: "ao5 with 4 DNFs",
			solves: []Solve{
				mkSolve(5 * time.Second),
				mkDNF(), mkDNF(), mkDNF(), mkDNF(),
			},
			window: 5,
		},
		{
			name: "ao5 all DNF",
			solves: []Solve{
				mkDNF(), mkDNF(), mkDNF(), mkDNF(), mkDNF(),
			},
			window: 5,
		},
		{
			name: "ao12 with 3 DNFs — 2 survive trim",
			solves: func() []Solve {
				sv := make([]Solve, 12)
				for i := 0; i < 9; i++ {
					sv[i] = mkSolve(time.Duration(i+1) * time.Second)
				}
				sv[9] = mkDNF()
				sv[10] = mkDNF()
				sv[11] = mkDNF()
				return sv
			}(),
			window: 12,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			s := buildSession(tt.solves)
			got, ok := s.Average(tt.window)
			if !ok {
				t.Fatal("expected ok=true, got false")
			}
			if !got.DNF {
				t.Error("expected DNF=true with multiple DNFs in window")
			}
		})
	}
}

// ── Best ──────────────────────────────────────────────────────────────────────

func TestBest(t *testing.T) {
	tests := []struct {
		name      string
		solves    []Solve
		wantDur   time.Duration
		wantFound bool
	}{
		{
			name:      "zero solves returns false",
			solves:    nil,
			wantFound: false,
		},
		{
			name:      "all DNF returns false",
			solves:    []Solve{mkDNF(), mkDNF()},
			wantFound: false,
		},
		{
			name: "single non-DNF returns its duration",
			solves: []Solve{
				mkSolve(10 * time.Second),
			},
			wantDur:   10 * time.Second,
			wantFound: true,
		},
		{
			name: "excludes DNF — returns min of non-DNF",
			// SCEN-10 subset: [15s, 12s, 10s, DNF, 11s]
			solves: []Solve{
				mkSolve(15 * time.Second),
				mkSolve(12 * time.Second),
				mkSolve(10 * time.Second),
				mkDNF(),
				mkSolve(11 * time.Second),
			},
			wantDur:   10 * time.Second,
			wantFound: true,
		},
		{
			name: "DNF interspersed — correct minimum",
			solves: []Solve{
				mkDNF(),
				mkSolve(8 * time.Second),
				mkDNF(),
				mkSolve(5 * time.Second),
				mkDNF(),
			},
			wantDur:   5 * time.Second,
			wantFound: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			s := buildSession(tt.solves)
			got, ok := s.Best()
			if ok != tt.wantFound {
				t.Fatalf("Best() found=%v, want %v", ok, tt.wantFound)
			}
			if tt.wantFound && got != tt.wantDur {
				t.Errorf("Best() = %v, want %v", got, tt.wantDur)
			}
		})
	}
}

// ── Mean ──────────────────────────────────────────────────────────────────────

func TestMean(t *testing.T) {
	tests := []struct {
		name      string
		solves    []Solve
		wantDur   time.Duration
		wantFound bool
	}{
		{
			name:      "zero solves returns false",
			solves:    nil,
			wantFound: false,
		},
		{
			name:      "all DNF returns false",
			solves:    []Solve{mkDNF(), mkDNF(), mkDNF()},
			wantFound: false,
		},
		{
			name: "excludes DNF from mean",
			// SCEN-10: [15s, 12s, 10s, DNF, 11s] → mean of [15, 12, 10, 11] = 48/4 = 12s
			solves: []Solve{
				mkSolve(15 * time.Second),
				mkSolve(12 * time.Second),
				mkSolve(10 * time.Second),
				mkDNF(),
				mkSolve(11 * time.Second),
			},
			wantDur:   12 * time.Second,
			wantFound: true,
		},
		{
			name: "single non-DNF solve",
			solves: []Solve{
				mkDNF(),
				mkSolve(7 * time.Second),
				mkDNF(),
			},
			wantDur:   7 * time.Second,
			wantFound: true,
		},
		{
			name: "all non-DNF mean",
			solves: []Solve{
				mkSolve(4 * time.Second),
				mkSolve(6 * time.Second),
				mkSolve(8 * time.Second),
			},
			// (4+6+8)/3 = 6s
			wantDur:   6 * time.Second,
			wantFound: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			s := buildSession(tt.solves)
			got, ok := s.Mean()
			if ok != tt.wantFound {
				t.Fatalf("Mean() found=%v, want %v", ok, tt.wantFound)
			}
			if tt.wantFound && got != tt.wantDur {
				t.Errorf("Mean() = %v, want %v", got, tt.wantDur)
			}
		})
	}
}

// ── Solves ────────────────────────────────────────────────────────────────────

func TestSolves_NewestFirst(t *testing.T) {
	a := Solve{Index: 1, Duration: 1 * time.Second}
	b := Solve{Index: 2, Duration: 2 * time.Second}
	c := Solve{Index: 3, Duration: 3 * time.Second}

	s := New()
	s.Add(a)
	s.Add(b)
	s.Add(c)

	got := s.Solves()
	if len(got) != 3 {
		t.Fatalf("Solves() len=%d, want 3", len(got))
	}
	// Newest first: c, b, a
	if got[0].Index != c.Index || got[1].Index != b.Index || got[2].Index != a.Index {
		t.Errorf("Solves() order = [%d, %d, %d], want [%d, %d, %d]",
			got[0].Index, got[1].Index, got[2].Index,
			c.Index, b.Index, a.Index)
	}
}

func TestSolves_ReturnsCopy_OriginalUnmodified(t *testing.T) {
	s := buildSession([]Solve{
		mkSolve(1 * time.Second),
		mkSolve(2 * time.Second),
		mkSolve(3 * time.Second),
	})

	copy1 := s.Solves()
	// Mutate the returned copy.
	copy1[0].Duration = 999 * time.Second

	copy2 := s.Solves()
	// The original order and values must be intact.
	if copy2[0].Duration == 999*time.Second {
		t.Error("Solves() returned a reference to internal state; mutation leaked back")
	}
}

func TestSolves_Empty(t *testing.T) {
	s := New()
	got := s.Solves()
	if len(got) != 0 {
		t.Errorf("Solves() on empty session len=%d, want 0", len(got))
	}
}

// ── Zero-solve boundary ───────────────────────────────────────────────────────

func TestZeroSolves_AllReturnFalse(t *testing.T) {
	s := New()

	if _, ok := s.Best(); ok {
		t.Error("Best() on zero solves: expected false")
	}
	if _, ok := s.Mean(); ok {
		t.Error("Mean() on zero solves: expected false")
	}
	if _, ok := s.Average(5); ok {
		t.Error("Average(5) on zero solves: expected false")
	}
	if _, ok := s.Average(12); ok {
		t.Error("Average(12) on zero solves: expected false")
	}
	if _, ok := s.Average(100); ok {
		t.Error("Average(100) on zero solves: expected false")
	}
}

// ── Add / Count ───────────────────────────────────────────────────────────────

func TestAdd_Count(t *testing.T) {
	s := New()
	if s.Count() != 0 {
		t.Fatalf("initial Count()=%d, want 0", s.Count())
	}
	s.Add(mkSolve(5 * time.Second))
	if s.Count() != 1 {
		t.Errorf("Count() after 1 add = %d, want 1", s.Count())
	}
	s.Add(mkDNF())
	if s.Count() != 2 {
		t.Errorf("Count() after 2 adds = %d, want 2", s.Count())
	}
}
