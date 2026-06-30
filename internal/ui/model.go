// Package ui provides the Bubbletea model and Lipgloss layout for tuibik.
package ui

import (
	"fmt"
	"math/rand/v2"
	"time"

	tea "charm.land/bubbletea/v2"
	"github.com/user/tuibik/internal/history"
	"github.com/user/tuibik/internal/scramble"
	"github.com/user/tuibik/internal/timer"
)

// tickMsg is dispatched by the display-refresh tick command.
type tickMsg time.Time

// tickCmd fires a tickMsg after 10ms to drive the live timer display.
func tickCmd() tea.Cmd {
	return tea.Tick(10*time.Millisecond, func(t time.Time) tea.Msg {
		return tickMsg(t)
	})
}

// Model is the Bubbletea model for tuibik.
// It is a value type; all mutations return a new Model.
type Model struct {
	tmr      *timer.Timer
	session  *history.Session
	strategy InputStrategy
	scramble string
	rng      *rand.Rand
	width    int
	height   int
	styles   styleSet
}

// New returns an initialized Model ready to run.
//
//   - kitty=true  → kittyStrategy (hold-release Space protocol)
//   - kitty=false → fallbackStrategy (press-press protocol) with auto-upgrade:
//     if the terminal later reports key-release support via KeyboardEnhancementsMsg,
//     Update() upgrades the strategy to kittyStrategy automatically.
func New(kitty bool) Model {
	tmr := timer.NewDefault()
	var strat InputStrategy
	if kitty {
		strat = kittyStrategy{}
	} else {
		strat = &fallbackStrategy{}
		// Allow fallbackStrategy.HandlePress to transition Armed→Running
		// immediately by setting armHold=0 (Press+Release succeeds in 0ms).
		tmr.WithArmHold(0)
	}
	seed := uint64(time.Now().UnixNano())
	rng := rand.New(rand.NewPCG(seed, seed^0xdeadbeef))
	return Model{
		tmr:      tmr,
		session:  history.New(),
		strategy: strat,
		scramble: scramble.Format(scramble.Generate(20, rng)),
		rng:      rng,
		styles:   newStyles(),
	}
}

// Init satisfies tea.Model. Keyboard enhancement requests are issued through
// View() instead of as an Init command (bubbletea v2 API).
func (m Model) Init() tea.Cmd {
	return nil
}

// Update processes an incoming message and returns the updated model plus an
// optional follow-up command.
func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd) {
	switch msg := msg.(type) {

	case tea.KeyboardEnhancementsMsg:
		// Upgrade from fallback to kitty when the terminal confirms key-release
		// support. Only upgrades, never downgrades.
		if msg.SupportsEventTypes() {
			if _, isFallback := m.strategy.(*fallbackStrategy); isFallback {
				m.strategy = kittyStrategy{}
				// Restore the 300ms arm-hold threshold now that releases are available.
				m.tmr.WithArmHold(300 * time.Millisecond)
			}
		}
		return m, nil

	case tea.KeyPressMsg:
		switch msg.String() {
		case "space":
			res := m.strategy.HandlePress(m.tmr)
			return m.handleTransition(res)
		case "n":
			if st := m.tmr.State(); st == timer.Idle || st == timer.Stopped {
				m.scramble = scramble.Format(scramble.Generate(20, m.rng))
			}
			return m, nil
		case "q", "ctrl+c":
			if st := m.tmr.State(); st == timer.Idle || st == timer.Stopped {
				return m, tea.Quit
			}
			return m, nil
		}
		return m, nil

	case tea.KeyReleaseMsg:
		if msg.String() == "space" {
			res := m.strategy.HandleRelease(m.tmr)
			return m.handleTransition(res)
		}
		return m, nil

	case tea.WindowSizeMsg:
		m.width = msg.Width
		m.height = msg.Height
		return m, nil

	case tickMsg:
		if m.tmr.State() == timer.Running {
			return m, tickCmd()
		}
		// Chain broken: timer stopped between tick dispatches.
		return m, nil
	}

	return m, nil
}

// handleTransition maps a timer.TransitionResult to the model update and command.
func (m Model) handleTransition(res timer.TransitionResult) (tea.Model, tea.Cmd) {
	if res.StartTick {
		return m, tickCmd()
	}
	if res.Elapsed > 0 {
		// A solve just finished: record, regenerate scramble, reset timer.
		m.session.Add(history.Solve{
			Index:    m.session.Count() + 1,
			Duration: res.Elapsed,
			Scramble: m.scramble,
			At:       time.Now(),
		})
		m.scramble = scramble.Format(scramble.Generate(20, m.rng))
		m.tmr.Reset()
	}
	return m, nil
}

// View satisfies tea.Model and returns the rendered view with terminal flags.
func (m Model) View() tea.View {
	v := tea.NewView(m.render())
	v.AltScreen = true
	// Request key-release events every frame; the terminal responds once with
	// KeyboardEnhancementsMsg if supported. Setting this always is safe.
	v.KeyboardEnhancements.ReportEventTypes = true
	return v
}

// ModeBadge returns the label for the active input mode.
// Returns "⚡ kitty" or "⌨ standard".
func (m Model) ModeBadge() string {
	return m.strategy.Badge()
}

// formatDuration formats a duration to centisecond precision.
//   - d < 60s  → "S.ss"    (e.g. "23.45")
//   - d >= 60s → "M:SS.ss" (e.g. "1:03.45")
func formatDuration(d time.Duration) string {
	cs := (d.Milliseconds() / 10) % 100
	totalSec := int(d.Seconds())
	if totalSec < 60 {
		return fmt.Sprintf("%d.%02d", totalSec, cs)
	}
	m := totalSec / 60
	s := totalSec % 60
	return fmt.Sprintf("%d:%02d.%02d", m, s, cs)
}

// timerDisplay returns the string and armed-state flag for the current timer state.
func (m Model) timerDisplay() (text string, armed bool) {
	switch m.tmr.State() {
	case timer.Armed:
		return "READY", true
	case timer.Running:
		return formatDuration(m.tmr.Elapsed()), false
	case timer.Stopped:
		return formatDuration(m.tmr.Elapsed()), false
	default: // Idle
		// Show the most recent (non-DNF) solve time, or "0.00" if no solves yet.
		if solves := m.session.Solves(); len(solves) > 0 && !solves[0].DNF {
			return formatDuration(solves[0].Duration), false
		}
		return "0.00", false
	}
}

// formatStat formats a stat value for display; returns "-" when not available.
func formatStat(d time.Duration, ok bool) string {
	if !ok {
		return "-"
	}
	return formatDuration(d)
}

// formatAoStat formats an AoResult; returns "-" when not available and "DNF" for DNF averages.
func formatAoStat(r history.AoResult, ok bool) string {
	if !ok {
		return "-"
	}
	if r.DNF {
		return "DNF"
	}
	return formatDuration(r.Duration)
}
