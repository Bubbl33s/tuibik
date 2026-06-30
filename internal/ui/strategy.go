package ui

import "github.com/user/tuibik/internal/timer"

// InputStrategy abstracts how Space key events translate into timer transitions.
// kittyStrategy uses the Kitty keyboard protocol (press + release).
// fallbackStrategy uses press-press (no key-release events needed).
type InputStrategy interface {
	HandlePress(t *timer.Timer) timer.TransitionResult
	HandleRelease(t *timer.Timer) timer.TransitionResult
	Badge() string
}

// kittyStrategy handles terminals that report key-release events.
// Hold Space → Armed; release Space → Running; press Space again → Stopped.
type kittyStrategy struct{}

func (kittyStrategy) HandlePress(t *timer.Timer) timer.TransitionResult {
	return t.Press()
}

func (kittyStrategy) HandleRelease(t *timer.Timer) timer.TransitionResult {
	return t.Release()
}

func (kittyStrategy) Badge() string { return "⚡ kitty" }

// fallbackStrategy handles terminals that do not report key-release events.
// First Space press: immediately starts the timer (bypasses the 300ms arm hold
// by requiring the caller to set the timer's armHold to 0 via WithArmHold(0)).
// Second Space press: stops the timer.
type fallbackStrategy struct {
	pressed bool // true after the first press that transitioned the timer to Running
}

// HandlePress immediately starts (first call) or stops (second call) the timer.
// It transitions Idle → Running by calling Press() then Release() in sequence.
// The timer must have armHold=0 (set in ui.New) so the immediate release succeeds.
func (f *fallbackStrategy) HandlePress(t *timer.Timer) timer.TransitionResult {
	if !f.pressed {
		f.pressed = true
		t.Press()       // Idle → Armed (with armHold=0)
		return t.Release() // Armed → Running immediately (0 >= armHold=0)
	}
	f.pressed = false
	return t.Press() // Running → Stopped
}

// HandleRelease is a no-op: fallback terminals never fire key-release events.
func (f *fallbackStrategy) HandleRelease(t *timer.Timer) timer.TransitionResult {
	return timer.TransitionResult{}
}

func (fallbackStrategy) Badge() string { return "⌨ standard" }
