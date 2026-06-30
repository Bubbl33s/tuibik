package timer

import (
	"testing"
	"time"
)

// fakeClock is a manually-advanced clock for deterministic tests.
type fakeClock struct {
	now time.Time
}

func newFakeClock() *fakeClock {
	return &fakeClock{now: time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC)}
}

func (c *fakeClock) Now() time.Time { return c.now }

func (c *fakeClock) advance(d time.Duration) { c.now = c.now.Add(d) }

// newTestTimer creates a timer with a fake clock and the given armHold.
func newTestTimer(fc *fakeClock, armHold time.Duration) *Timer {
	t := New(fc)
	t.armHold = armHold
	return t
}

// ── Press tests ────────────────────────────────────────────────────────────────

func TestPress_IdleToArmed(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 300*time.Millisecond)

	res := tmr.Press()

	if !res.Changed {
		t.Error("expected Changed=true on Idle→Armed")
	}
	if res.NewState != Armed {
		t.Errorf("expected NewState=Armed, got %v", res.NewState)
	}
	if tmr.state != Armed {
		t.Errorf("expected internal state=Armed, got %v", tmr.state)
	}
	if tmr.armedAt.IsZero() {
		t.Error("expected armedAt to be recorded")
	}
}

func TestPress_ArmedIsNoOp(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 300*time.Millisecond)
	tmr.Press() // Idle → Armed

	res := tmr.Press() // Armed → no-op

	if res.Changed {
		t.Error("expected Changed=false on Armed→Press (no-op)")
	}
	if res.NewState != Armed {
		t.Errorf("expected NewState=Armed, got %v", res.NewState)
	}
}

func TestPress_RunningToStopped(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 1*time.Millisecond)

	// Arm
	tmr.Press()
	fc.advance(5 * time.Millisecond) // exceed armHold

	// Start
	tmr.Release()
	fc.advance(3 * time.Second) // simulate a 3s solve

	// Stop
	res := tmr.Press()

	if !res.Changed {
		t.Error("expected Changed=true on Running→Stopped")
	}
	if res.NewState != Stopped {
		t.Errorf("expected NewState=Stopped, got %v", res.NewState)
	}
	if !res.StopTick {
		t.Error("expected StopTick=true")
	}
	if res.Elapsed != 3*time.Second {
		t.Errorf("expected Elapsed=3s, got %v", res.Elapsed)
	}
}

func TestPress_StoppedToIdle(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 1*time.Millisecond)

	tmr.Press()                       // Idle → Armed
	fc.advance(5 * time.Millisecond)  // exceed armHold
	tmr.Release()                     // Armed → Running
	fc.advance(2 * time.Second)       // solve duration
	tmr.Press()                       // Running → Stopped

	res := tmr.Press() // Stopped → Idle

	if !res.Changed {
		t.Error("expected Changed=true on Stopped→Idle")
	}
	if res.NewState != Idle {
		t.Errorf("expected NewState=Idle, got %v", res.NewState)
	}
	if tmr.state != Idle {
		t.Errorf("expected internal state=Idle, got %v", tmr.state)
	}
}

// ── Release tests ──────────────────────────────────────────────────────────────

func TestRelease_ArmCancelWhenHoldTooShort(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 300*time.Millisecond)

	tmr.Press() // Idle → Armed; armedAt recorded
	fc.advance(299 * time.Millisecond)

	res := tmr.Release() // hold < 300ms → cancel

	if !res.Changed {
		t.Error("expected Changed=true on arm cancel")
	}
	if res.NewState != Idle {
		t.Errorf("expected NewState=Idle after cancel, got %v", res.NewState)
	}
	if tmr.state != Idle {
		t.Errorf("expected internal state=Idle, got %v", tmr.state)
	}
	if !tmr.armedAt.IsZero() {
		t.Error("expected armedAt to be cleared after cancel")
	}
}

func TestRelease_ArmCommitWhenHoldSufficient(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 300*time.Millisecond)

	tmr.Press() // Idle → Armed
	fc.advance(300 * time.Millisecond)

	res := tmr.Release() // hold == 300ms → commit

	if !res.Changed {
		t.Error("expected Changed=true on Armed→Running")
	}
	if res.NewState != Running {
		t.Errorf("expected NewState=Running, got %v", res.NewState)
	}
	if !res.StartTick {
		t.Error("expected StartTick=true")
	}
	if tmr.state != Running {
		t.Errorf("expected internal state=Running, got %v", tmr.state)
	}
}

func TestRelease_IdleIsNoOp(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 300*time.Millisecond)

	res := tmr.Release() // Idle → no-op

	if res.Changed {
		t.Error("expected Changed=false on Release from Idle")
	}
	if res.NewState != Idle {
		t.Errorf("expected NewState=Idle, got %v", res.NewState)
	}
}

func TestRelease_RunningIsNoOp(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 1*time.Millisecond)

	tmr.Press()
	fc.advance(5 * time.Millisecond)
	tmr.Release() // → Running

	res := tmr.Release() // Running → no-op

	if res.Changed {
		t.Error("expected Changed=false on Release from Running")
	}
	if res.NewState != Running {
		t.Errorf("expected NewState=Running, got %v", res.NewState)
	}
}

// ── Elapsed tests ─────────────────────────────────────────────────────────────

func TestElapsed_DuringRunning_LiveFromClock(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 1*time.Millisecond)

	tmr.Press()
	fc.advance(5 * time.Millisecond)
	tmr.Release() // → Running; startedAt = now

	fc.advance(1500 * time.Millisecond)
	got := tmr.Elapsed()

	if got != 1500*time.Millisecond {
		t.Errorf("expected Elapsed=1.5s during Running, got %v", got)
	}
}

func TestElapsed_AfterStopped_FrozenNotRecomputed(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 1*time.Millisecond)

	tmr.Press()
	fc.advance(5 * time.Millisecond)
	tmr.Release() // → Running
	fc.advance(2 * time.Second)
	tmr.Press() // → Stopped; elapsed frozen at 2s

	// Advance clock further; frozen value must not change.
	fc.advance(999 * time.Second)
	got := tmr.Elapsed()

	if got != 2*time.Second {
		t.Errorf("expected frozen Elapsed=2s after Stopped, got %v", got)
	}
}

func TestElapsed_IdleReturnsZero(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 300*time.Millisecond)

	if got := tmr.Elapsed(); got != 0 {
		t.Errorf("expected Elapsed=0 in Idle, got %v", got)
	}
}

func TestElapsed_ArmedReturnsZero(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 300*time.Millisecond)
	tmr.Press() // Idle → Armed
	fc.advance(100 * time.Millisecond)

	if got := tmr.Elapsed(); got != 0 {
		t.Errorf("expected Elapsed=0 in Armed, got %v", got)
	}
}

// ── armHold precision tests ───────────────────────────────────────────────────

func TestArmHold_BelowThresholdCancels(t *testing.T) {
	fc := newFakeClock()
	// Use 1ms so we can test boundary precisely without sleeping.
	tmr := newTestTimer(fc, 1*time.Millisecond)

	tmr.Press()
	fc.advance(999 * time.Microsecond) // < 1ms

	res := tmr.Release()

	if res.NewState != Idle {
		t.Errorf("expected Idle (cancel) at 999µs hold, got %v", res.NewState)
	}
}

func TestArmHold_AtThresholdCommits(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 1*time.Millisecond)

	tmr.Press()
	fc.advance(1 * time.Millisecond) // == armHold

	res := tmr.Release()

	if res.NewState != Running {
		t.Errorf("expected Running at exact 1ms hold, got %v", res.NewState)
	}
}

func TestArmHold_AboveThresholdCommits(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 1*time.Millisecond)

	tmr.Press()
	fc.advance(2 * time.Millisecond) // > armHold

	res := tmr.Release()

	if res.NewState != Running {
		t.Errorf("expected Running when hold > armHold, got %v", res.NewState)
	}
}

// ── Reset tests ───────────────────────────────────────────────────────────────

func TestReset_FromStopped(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 1*time.Millisecond)

	tmr.Press()
	fc.advance(5 * time.Millisecond)
	tmr.Release()
	fc.advance(1 * time.Second)
	tmr.Press() // → Stopped

	tmr.Reset()

	if tmr.state != Idle {
		t.Errorf("expected Idle after Reset, got %v", tmr.state)
	}
	if tmr.Elapsed() != 0 {
		t.Errorf("expected Elapsed=0 after Reset, got %v", tmr.Elapsed())
	}
}

func TestReset_FromOtherStatesIsNoOp(t *testing.T) {
	fc := newFakeClock()
	tmr := newTestTimer(fc, 300*time.Millisecond)

	// From Idle
	tmr.Reset()
	if tmr.state != Idle {
		t.Error("Reset from Idle should be no-op")
	}

	// From Armed
	tmr.Press()
	tmr.Reset()
	if tmr.state != Armed {
		t.Error("Reset from Armed should be no-op")
	}
}

// ── Table-driven full state-machine walkthrough ───────────────────────────────

func TestStateMachine_TableDriven(t *testing.T) {
	type step struct {
		name        string
		action      func(tmr *Timer, fc *fakeClock)
		wantState   State
		wantChanged bool
		wantStart   bool
		wantStop    bool
	}

	fc := newFakeClock()
	tmr := newTestTimer(fc, 1*time.Millisecond)

	steps := []step{
		{
			name: "Idle+Press → Armed",
			action: func(tmr *Timer, fc *fakeClock) {
				res := tmr.Press()
				if !res.Changed || res.NewState != Armed {
					t.Errorf("Idle+Press: got changed=%v state=%v", res.Changed, res.NewState)
				}
			},
			wantState: Armed,
		},
		{
			name: "Armed+Release before hold → Idle (cancel)",
			action: func(tmr *Timer, fc *fakeClock) {
				fc.advance(500 * time.Microsecond) // < 1ms armHold
				res := tmr.Release()
				if !res.Changed || res.NewState != Idle {
					t.Errorf("cancel: got changed=%v state=%v", res.Changed, res.NewState)
				}
			},
			wantState: Idle,
		},
		{
			name: "Idle+Press again → Armed",
			action: func(tmr *Timer, fc *fakeClock) {
				res := tmr.Press()
				if !res.Changed || res.NewState != Armed {
					t.Errorf("re-arm: got changed=%v state=%v", res.Changed, res.NewState)
				}
			},
			wantState: Armed,
		},
		{
			name: "Armed+Release after hold → Running",
			action: func(tmr *Timer, fc *fakeClock) {
				fc.advance(2 * time.Millisecond) // >= 1ms armHold
				res := tmr.Release()
				if !res.Changed || res.NewState != Running || !res.StartTick {
					t.Errorf("commit: got changed=%v state=%v startTick=%v", res.Changed, res.NewState, res.StartTick)
				}
			},
			wantState: Running,
		},
		{
			name: "Running+Release → no-op",
			action: func(tmr *Timer, fc *fakeClock) {
				res := tmr.Release()
				if res.Changed {
					t.Error("Release from Running should be no-op")
				}
			},
			wantState: Running,
		},
		{
			name: "Running+Press → Stopped",
			action: func(tmr *Timer, fc *fakeClock) {
				fc.advance(5 * time.Second)
				res := tmr.Press()
				if !res.Changed || res.NewState != Stopped || !res.StopTick || res.Elapsed != 5*time.Second {
					t.Errorf("stop: got changed=%v state=%v stopTick=%v elapsed=%v",
						res.Changed, res.NewState, res.StopTick, res.Elapsed)
				}
			},
			wantState: Stopped,
		},
		{
			name: "Stopped+Press → Idle (reset path)",
			action: func(tmr *Timer, fc *fakeClock) {
				res := tmr.Press()
				if !res.Changed || res.NewState != Idle {
					t.Errorf("stopped→idle: got changed=%v state=%v", res.Changed, res.NewState)
				}
			},
			wantState: Idle,
		},
	}

	for _, s := range steps {
		t.Run(s.name, func(t *testing.T) {
			s.action(tmr, fc)
			if tmr.state != s.wantState {
				t.Errorf("after %q: want state %v, got %v", s.name, s.wantState, tmr.state)
			}
		})
	}
}
