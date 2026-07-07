package ui

import (
	"flag"
	"math/rand/v2"
	"os"
	"path/filepath"
	"testing"
	"time"

	tea "charm.land/bubbletea/v2"
	"charm.land/lipgloss/v2"
	"github.com/user/tuibik/internal/history"
	"github.com/user/tuibik/internal/timer"
)

var update = flag.Bool("update", false, "update golden files")

// ── test helpers ───────────────────────────────────────────────────────────────

// testClock is a deterministic timer.Clock.
type testClock struct {
	now time.Time
}

func (c *testClock) Now() time.Time         { return c.now }
func (c *testClock) advance(d time.Duration) { c.now = c.now.Add(d) }

func newTestClock() *testClock {
	return &testClock{now: time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC)}
}

// newTestStyles returns a plain (no ANSI color) styleSet for golden comparisons.
func newTestStyles() styleSet {
	plain := lipgloss.NewStyle()
	return styleSet{
		timerNormal: plain,
		timerArmed:  plain,
		scramble:    plain,
		badge:       plain,
		statValue:   plain,
		statLabel:   plain,
		historyItem: plain,
	}
}

// newFallbackTestModel builds a fallback-strategy Model with a fake clock.
func newFallbackTestModel(fc *testClock) Model {
	tmr := timer.New(fc).WithArmHold(0)
	return Model{
		tmr:      tmr,
		session:  history.New(),
		strategy: &fallbackStrategy{},
		scramble: "R U' F2 B L' D R'",
		rng:      rand.New(rand.NewPCG(42, 43)),
		styles:   newTestStyles(),
	}
}

// newKittyTestModel builds a kittyStrategy Model with a fake clock (300ms arm hold).
func newKittyTestModel(fc *testClock) Model {
	tmr := timer.New(fc)
	return Model{
		tmr:      tmr,
		session:  history.New(),
		strategy: kittyStrategy{},
		scramble: "R U' F2 B L' D R'",
		rng:      rand.New(rand.NewPCG(42, 43)),
		styles:   newTestStyles(),
	}
}

// spacePress returns a Space key-press message.
// bubbletea v2 / ultraviolet: space String() == "space" (not " ").
func spacePress() tea.KeyPressMsg { return tea.KeyPressMsg{Code: tea.KeySpace} }

// spaceRelease returns a Space key-release message.
func spaceRelease() tea.KeyReleaseMsg { return tea.KeyReleaseMsg{Code: tea.KeySpace} }

// mustModel asserts the returned tea.Model is a Model.
func mustModel(tm tea.Model) Model { return tm.(Model) }

// ── Update() state-transition tests ───────────────────────────────────────────

func TestUpdate_Fallback_SpaceStartsTimer(t *testing.T) {
	fc := newTestClock()
	m := newFallbackTestModel(fc)

	m2, cmd := m.Update(spacePress())
	m = mustModel(m2)

	if m.tmr.State() != timer.Running {
		t.Errorf("expected Running after first space press, got %v", m.tmr.State())
	}
	if cmd == nil {
		t.Error("expected tickCmd to be returned")
	}
}

func TestUpdate_Fallback_SpaceStopsTimer_AddsSolve(t *testing.T) {
	fc := newTestClock()
	m := newFallbackTestModel(fc)

	// First press: starts timer.
	m2, _ := m.Update(spacePress())
	m = mustModel(m2)

	// Advance fake clock to simulate a 5-second solve.
	fc.advance(5 * time.Second)

	// Second press: stops timer, records solve, resets timer to Idle.
	m2, _ = m.Update(spacePress())
	m = mustModel(m2)

	if m.tmr.State() != timer.Idle {
		t.Errorf("expected Idle after solve, got %v", m.tmr.State())
	}
	if m.session.Count() != 1 {
		t.Errorf("expected 1 solve, got %d", m.session.Count())
	}
	solves := m.session.Solves()
	if len(solves) == 0 {
		t.Fatal("expected at least 1 solve in session")
	}
	if solves[0].Duration != 5*time.Second {
		t.Errorf("expected solve duration 5s, got %v", solves[0].Duration)
	}
}

func TestUpdate_Kitty_SpacePressArmsTimer(t *testing.T) {
	fc := newTestClock()
	m := newKittyTestModel(fc)

	m2, _ := m.Update(spacePress())
	m = mustModel(m2)

	if m.tmr.State() != timer.Armed {
		t.Errorf("expected Armed after space press, got %v", m.tmr.State())
	}
}

func TestUpdate_Kitty_SpaceReleaseAfterHold_StartsTimer(t *testing.T) {
	fc := newTestClock()
	m := newKittyTestModel(fc)

	// Press: Idle → Armed.
	m2, _ := m.Update(spacePress())
	m = mustModel(m2)

	// Advance clock past arm hold.
	fc.advance(300 * time.Millisecond)

	// Release: Armed → Running; should return tickCmd.
	m2, cmd := m.Update(spaceRelease())
	m = mustModel(m2)

	if m.tmr.State() != timer.Running {
		t.Errorf("expected Running after release, got %v", m.tmr.State())
	}
	if cmd == nil {
		t.Error("expected tickCmd after Armed → Running")
	}
}

func TestUpdate_Kitty_SpacePressWhileRunning_StopsAndRecords(t *testing.T) {
	fc := newTestClock()
	m := newKittyTestModel(fc)

	// Arm and start.
	m2, _ := m.Update(spacePress())
	m = mustModel(m2)
	fc.advance(300 * time.Millisecond)
	m2, _ = m.Update(spaceRelease())
	m = mustModel(m2)

	// Solve: 7 seconds.
	fc.advance(7 * time.Second)

	// Stop.
	m2, _ = m.Update(spacePress())
	m = mustModel(m2)

	if m.tmr.State() != timer.Idle {
		t.Errorf("expected Idle after solve, got %v", m.tmr.State())
	}
	if m.session.Count() != 1 {
		t.Errorf("expected 1 solve, got %d", m.session.Count())
	}
	solves := m.session.Solves()
	if solves[0].Duration != 7*time.Second {
		t.Errorf("expected 7s solve, got %v", solves[0].Duration)
	}
}

func TestUpdate_NKey_RegeneratesScramble(t *testing.T) {
	fc := newTestClock()
	m := newKittyTestModel(fc)
	original := m.scramble

	m2, _ := m.Update(tea.KeyPressMsg{Code: 'n', Text: "n"})
	m = mustModel(m2)

	if m.scramble == original {
		t.Error("expected scramble to change after pressing n")
	}
}

func TestUpdate_NKey_IgnoredWhenRunning(t *testing.T) {
	fc := newTestClock()
	m := newFallbackTestModel(fc)

	// Start timer.
	m2, _ := m.Update(spacePress())
	m = mustModel(m2)
	original := m.scramble

	// 'n' should be ignored while running.
	m2, _ = m.Update(tea.KeyPressMsg{Code: 'n', Text: "n"})
	m = mustModel(m2)

	if m.scramble != original {
		t.Error("scramble should not change while timer is running")
	}
}

func TestUpdate_QKey_QuitsWhenIdle(t *testing.T) {
	fc := newTestClock()
	m := newKittyTestModel(fc)

	_, cmd := m.Update(tea.KeyPressMsg{Code: 'q', Text: "q"})

	if cmd == nil {
		t.Error("expected tea.Quit cmd on 'q' in Idle")
	}
	// Verify it produces a QuitMsg.
	msg := cmd()
	if _, ok := msg.(tea.QuitMsg); !ok {
		t.Errorf("expected QuitMsg, got %T", msg)
	}
}

func TestUpdate_QKey_IgnoredWhenRunning(t *testing.T) {
	fc := newTestClock()
	m := newFallbackTestModel(fc)

	// Start timer.
	m2, _ := m.Update(spacePress())
	m = mustModel(m2)

	// 'q' while running should be ignored.
	_, cmd := m.Update(tea.KeyPressMsg{Code: 'q', Text: "q"})

	if cmd != nil {
		t.Error("expected no cmd on 'q' while timer is running")
	}
	if m.tmr.State() != timer.Running {
		t.Errorf("expected timer still Running, got %v", m.tmr.State())
	}
}

func TestUpdate_CtrlC_QuitsWhenIdle(t *testing.T) {
	fc := newTestClock()
	m := newKittyTestModel(fc)

	_, cmd := m.Update(tea.KeyPressMsg{Code: 'c', Mod: tea.ModCtrl})

	if cmd == nil {
		t.Error("expected tea.Quit cmd on ctrl+c in Idle")
	}
	msg := cmd()
	if _, ok := msg.(tea.QuitMsg); !ok {
		t.Errorf("expected QuitMsg, got %T", msg)
	}
}

func TestUpdate_WindowSizeMsg_StoresSize(t *testing.T) {
	fc := newTestClock()
	m := newKittyTestModel(fc)

	m2, _ := m.Update(tea.WindowSizeMsg{Width: 120, Height: 40})
	m = mustModel(m2)

	if m.width != 120 {
		t.Errorf("expected width 120, got %d", m.width)
	}
	if m.height != 40 {
		t.Errorf("expected height 40, got %d", m.height)
	}
}

func TestUpdate_TickMsg_ContinuesWhenRunning(t *testing.T) {
	fc := newTestClock()
	m := newFallbackTestModel(fc)

	// Start timer.
	m2, _ := m.Update(spacePress())
	m = mustModel(m2)

	// Tick while running should return another tickCmd.
	_, cmd := m.Update(tickMsg(time.Now()))
	if cmd == nil {
		t.Error("expected tickCmd while timer is running")
	}
}

func TestUpdate_TickMsg_StopsWhenNotRunning(t *testing.T) {
	fc := newTestClock()
	m := newKittyTestModel(fc) // Idle state

	// Tick while idle should return nil (chain breaks).
	_, cmd := m.Update(tickMsg(time.Now()))
	if cmd != nil {
		t.Error("expected nil cmd from tick when timer is not running")
	}
}

func TestUpdate_KeyboardEnhancementsMsg_UpgradesFallbackToKitty(t *testing.T) {
	fc := newTestClock()
	m := newFallbackTestModel(fc)

	if _, ok := m.strategy.(*fallbackStrategy); !ok {
		t.Fatal("precondition: expected fallbackStrategy")
	}

	// Simulate terminal granting event types (key releases).
	// SupportsEventTypes checks Flags & ansi.KittyReportEventTypes.
	// The flag value from charmbracelet/x/ansi is 2.
	m2, _ := m.Update(tea.KeyboardEnhancementsMsg{Flags: 2})
	m = mustModel(m2)

	if _, ok := m.strategy.(kittyStrategy); !ok {
		t.Errorf("expected strategy to upgrade to kittyStrategy, got %T", m.strategy)
	}
}

func TestUpdate_KeyboardEnhancementsMsg_NoUpgradeWhenAlreadyKitty(t *testing.T) {
	fc := newTestClock()
	m := newKittyTestModel(fc)

	m2, _ := m.Update(tea.KeyboardEnhancementsMsg{Flags: 2})
	m = mustModel(m2)

	if _, ok := m.strategy.(kittyStrategy); !ok {
		t.Errorf("expected kittyStrategy to remain, got %T", m.strategy)
	}
}

// ── formatDuration tests ───────────────────────────────────────────────────────

func TestFormatDuration(t *testing.T) {
	cases := []struct {
		d    time.Duration
		want string
	}{
		{0, "0.00"},
		{450 * time.Millisecond, "0.45"},
		{23*time.Second + 450*time.Millisecond, "23.45"},
		{59*time.Second + 990*time.Millisecond, "59.99"},
		{63*time.Second + 450*time.Millisecond, "1:03.45"},
		{2*time.Minute + 0*time.Second, "2:00.00"},
		{10*time.Minute + 5*time.Second + 120*time.Millisecond, "10:05.12"},
	}
	for _, tc := range cases {
		got := formatDuration(tc.d)
		if got != tc.want {
			t.Errorf("formatDuration(%v) = %q, want %q", tc.d, got, tc.want)
		}
	}
}

// ── golden View() snapshot tests ──────────────────────────────────────────────

// goldenPath returns the path for a named golden file.
func goldenPath(name string) string {
	return filepath.Join("testdata", name+".golden")
}

// assertGolden compares got against the named golden file, or writes it when -update.
func assertGolden(t *testing.T, name, got string) {
	t.Helper()
	path := goldenPath(name)
	if *update {
		if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
			t.Fatalf("mkdir %s: %v", filepath.Dir(path), err)
		}
		if err := os.WriteFile(path, []byte(got), 0644); err != nil {
			t.Fatalf("writing golden %s: %v", path, err)
		}
		t.Logf("updated golden: %s", path)
		return
	}
	want, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("reading golden %s: %v (run `go test -update` to create)", path, err)
	}
	if string(want) != got {
		t.Errorf("golden mismatch %s:\ngot:\n%s\nwant:\n%s", name, got, string(want))
	}
}

// goldenModel builds a deterministic Model for View snapshot tests.
// It uses a fake clock with timer in Idle state and a fixed scramble.
// Optionally seeds the session with solves for stats display.
func goldenModel(width int, solves []history.Solve) Model {
	fc := newTestClock()
	tmr := timer.New(fc)
	sess := history.New()
	for _, sv := range solves {
		sess.Add(sv)
	}
	return Model{
		tmr:      tmr,
		session:  sess,
		strategy: kittyStrategy{},
		scramble: "R U' F2 B L' D R' U F B' R2 U2 F' B2 L D2 R",
		rng:      rand.New(rand.NewPCG(42, 43)),
		styles:   newTestStyles(),
		width:    width,
	}
}

func TestView_Width100_EmptySession(t *testing.T) {
	m := goldenModel(100, nil)
	assertGolden(t, "view_w100_empty", m.render())
}

func TestView_Width80_EmptySession(t *testing.T) {
	m := goldenModel(80, nil)
	assertGolden(t, "view_w80_empty", m.render())
}

func TestView_Width60_EmptySession(t *testing.T) {
	m := goldenModel(60, nil)
	assertGolden(t, "view_w60_empty", m.render())
}

func TestView_Width40_EmptySession(t *testing.T) {
	m := goldenModel(40, nil)
	assertGolden(t, "view_w40_empty", m.render())
}

func TestView_Width100_WithSolves(t *testing.T) {
	solves := []history.Solve{
		{Index: 1, Duration: 23*time.Second + 450*time.Millisecond, Scramble: "R U' F2", At: time.Date(2024, 1, 1, 0, 0, 0, 0, time.UTC)},
		{Index: 2, Duration: 19*time.Second + 120*time.Millisecond, Scramble: "U F B'", At: time.Date(2024, 1, 1, 0, 0, 1, 0, time.UTC)},
		{Index: 3, Duration: 21*time.Second + 880*time.Millisecond, Scramble: "L D2 R", At: time.Date(2024, 1, 1, 0, 0, 2, 0, time.UTC)},
	}
	m := goldenModel(100, solves)
	assertGolden(t, "view_w100_solves", m.render())
}
