package timer

import "time"

// State represents the current phase of the solve timer.
type State int

const (
	// Idle is the resting state before any interaction.
	Idle State = iota
	// Armed means the space bar is held long enough to commit to a start.
	Armed
	// Running means the solve is in progress.
	Running
	// Stopped means the solve has ended and the final time is frozen.
	Stopped
)

// Clock is injected for deterministic tests.
type Clock interface {
	Now() time.Time
}

type realClock struct{}

func (realClock) Now() time.Time { return time.Now() }

// TransitionResult carries the outcome of a Press or Release call.
type TransitionResult struct {
	// Changed is true when the state actually changed.
	Changed bool
	// NewState is the state after the event.
	NewState State
	// StartTick tells the UI to begin the display-refresh Cmd.
	StartTick bool
	// StopTick tells the UI to stop the display-refresh chain.
	StopTick bool
	// Elapsed is the solve duration, non-zero only when a solve just finished (Stopped).
	Elapsed time.Duration
}

// Timer implements the Idle→Armed→Running→Stopped→Idle state machine.
// It is not goroutine-safe; callers must serialize access if needed.
type Timer struct {
	state     State
	armedAt   time.Time
	startedAt time.Time
	clock     Clock
	armHold   time.Duration // minimum hold duration to commit; default 300ms
	elapsed   time.Duration // frozen final value after Stopped
}

// New returns a Timer with the given Clock and 300ms arm-hold threshold.
func New(clock Clock) *Timer {
	return &Timer{
		clock:   clock,
		armHold: 300 * time.Millisecond,
	}
}

// NewDefault returns a Timer using the real wall clock and 300ms arm-hold threshold.
func NewDefault() *Timer {
	return New(realClock{})
}

// WithArmHold overrides the arm-hold threshold. Useful for tests.
func (t *Timer) WithArmHold(d time.Duration) *Timer {
	t.armHold = d
	return t
}

// State returns the current timer state.
func (t *Timer) State() State {
	return t.state
}

// Elapsed returns the live duration during Running, the frozen value after Stopped, and 0 otherwise.
func (t *Timer) Elapsed() time.Duration {
	switch t.state {
	case Running:
		return t.clock.Now().Sub(t.startedAt)
	case Stopped:
		return t.elapsed
	default:
		return 0
	}
}

// Press signals a space-key-down event.
//
// State machine:
//
//	Idle    → Armed   (records armedAt)
//	Running → Stopped (records elapsed, signals StopTick)
//	Stopped → Idle    (same as Reset)
//	Armed   → no-op
func (t *Timer) Press() TransitionResult {
	now := t.clock.Now()

	switch t.state {
	case Idle:
		t.armedAt = now
		t.state = Armed
		return TransitionResult{Changed: true, NewState: Armed}

	case Running:
		dur := now.Sub(t.startedAt)
		t.elapsed = dur
		t.state = Stopped
		return TransitionResult{
			Changed:  true,
			NewState: Stopped,
			StopTick: true,
			Elapsed:  dur,
		}

	case Stopped:
		t.state = Idle
		t.armedAt = time.Time{}
		t.startedAt = time.Time{}
		t.elapsed = 0
		return TransitionResult{Changed: true, NewState: Idle}

	default:
		// Armed → no-op
		return TransitionResult{Changed: false, NewState: t.state}
	}
}

// Release signals a space-key-up event.
//
// State machine:
//
//	Armed, hold < armHold → Idle   (cancelled; no solve)
//	Armed, hold >= armHold → Running (records startedAt, signals StartTick)
//	any other state       → no-op
func (t *Timer) Release() TransitionResult {
	now := t.clock.Now()

	if t.state != Armed {
		return TransitionResult{Changed: false, NewState: t.state}
	}

	held := now.Sub(t.armedAt)
	if held < t.armHold {
		// Cancelled: hold was too short.
		t.state = Idle
		t.armedAt = time.Time{}
		return TransitionResult{Changed: true, NewState: Idle}
	}

	// Committed: transition to Running.
	t.startedAt = now
	t.state = Running
	return TransitionResult{Changed: true, NewState: Running, StartTick: true}
}

// Reset transitions Stopped → Idle and clears all stored instants.
// Called from any other state it is a no-op.
func (t *Timer) Reset() {
	if t.state != Stopped {
		return
	}
	t.state = Idle
	t.armedAt = time.Time{}
	t.startedAt = time.Time{}
	t.elapsed = 0
}
