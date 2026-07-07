# Apply Progress: rubik-tui — chained PRs

topic_key: sdd/rubik-tui/apply-progress
project: tuibik
type: architecture
capture_prompt: false

---

## Tasks Completed

- [x] Task 4.1 — internal/history/history.go (Session, Solve, AoResult types; New/Add/Count/Solves/Best/Mean/Average)
- [x] Task 4.2 — internal/history/history_test.go (table-driven; ao5/ao12/ao100 numeric, DNF semantics, Best/Mean exclusions, Solves copy)
- [x] Task 1.1 — go.mod + go.sum (charm.land v2 module paths, Go 1.24.2)
- [x] Task 1.2 — .gitignore, LICENSE (MIT 2024), README.md skeleton
- [x] Task 1.3 — Makefile (build/test/vet/clean + stubbed dist/release)
- [x] Task 2.1 — internal/scramble/scramble.go (type Move string, Generate(n int, rng *rand.Rand) []Move, Format, Face())
- [x] Task 2.2 — internal/scramble/scramble_test.go (1000-iteration constraint checks, all pass)
- [x] Task 3.1 — internal/timer/timer.go (Clock interface, State machine, TransitionResult, Press/Release/Reset/Elapsed)
- [x] Task 3.2 — internal/timer/timer_test.go (18 table-driven cases, fake clock, zero time.Sleep, armHold precision)
- [x] Task 5.1 — internal/ui/strategy.go (InputStrategy interface + kittyStrategy + fallbackStrategy)
- [x] Task 5.2 — internal/ui/styles.go (Lipgloss styleSet with amber armed indicator + muted scramble)
- [x] Task 5.3 — internal/ui/model.go (Model struct, New, Init, Update, View, formatDuration, handleTransition)
- [x] Task 5.4 — internal/ui/layout.go (width-band render functions: full/wide/medium/minimal)
- [x] Task 5.5 — internal/ui/model_test.go (15 Update transition tests + 5 golden View snapshots)
- [x] Task 5.6 — internal/ui/testdata/*.golden (5 golden view files for widths 100/80/60/40 + solves)
- [x] Task 6.1 — cmd/tuibik/main.go (entrypoint with --kitty/--no-kitty/--version flags)

## Tasks Remaining

- [x] PR 3: internal/history — ao5/ao12/ao100 trimmed averages + DNF semantics
- [x] PR 4: internal/ui + cmd/tuibik — Bubbletea model + lipgloss layout + main entrypoint
- [ ] PR 5: Makefile dist/release + CI pipeline

## PRs

- PR 1 URL: https://github.com/Bubbl33s/tuibik/pull/1 — bootstrap + scramble (feat/rubik-tui-pr1 → main)
- PR 2 URL: https://github.com/Bubbl33s/tuibik/pull/2 — timer state machine (feat/rubik-tui-pr2 → main)
- PR 3 URL: https://github.com/Bubbl33s/tuibik/pull/3 — session history ao5/ao12/ao100 + DNF semantics (feat/rubik-tui-pr3 → main)
- PR 4 URL: https://github.com/Bubbl33s/tuibik/pull/4 — TUI model + Lipgloss layout + CLI entrypoint (feat/rubik-tui-pr4 → main)

## Test Results

- PR 1: `go test -count=1 -race ./...` → ok scramble 1.022s
- PR 2: `go test -count=1 -race ./...` → ok scramble 1.054s, ok timer 1.013s (18 tests)
- PR 3: `go test -count=1 -race ./...` → ok history 1.013s, ok scramble 1.024s, ok timer 1.013s (all pass)
- PR 4: `go test -count=1 -race ./...` → ok history, scramble, timer, ui (20 ui tests: 15 Update transitions + 5 golden View snapshots)

## Key Discoveries

- Charm v2 uses `charm.land/` module prefix, NOT `github.com/charmbracelet/*/v2`
  - bubbletea: `charm.land/bubbletea/v2 v2.0.0`
  - lipgloss: `charm.land/lipgloss/v2 v2.0.0`
  - bubbles: `charm.land/bubbles/v2 v2.0.0`
- Go upgraded go.mod to 1.24.2 automatically during `go get`
- A global git URL rewrite rule overrides SSH to HTTPS+token for a different account;
  used `gh auth token` to get the correct Bubbl33s token for push (same workaround applies to all PRs)
- `internal/scramble` uses `math/rand/v2` (Go 1.22+), `rand.NewPCG(seed, seed)` for deterministic tests
- Timer design uses injectable Clock interface (not spec's `time.Time` param style) — more testable;
  armHold field is exported-via-WithArmHold for test overrides, avoiding time.Sleep entirely
- feat/rubik-tui-pr2 was branched from feat/rubik-tui-pr1 (not main) because PR1 hasn't merged yet
- feat/rubik-tui-pr3 was branched from feat/rubik-tui-pr2 (stacked to main strategy)
- history.Session uses (duration, bool) returns (not pointer style from spec); Average(window) is generic for ao5/ao12/ao100
- ao100 trim=5 (5 best + 5 worst); ao5/ao12 trim=1 — follows WCA rules exactly
- DNF sort: non-DNF ascending by Duration, then DNF at the end (not by Duration field since DNF Duration=0)
- bubbletea v2 Model interface: `Init() Cmd` (not `(Model,Cmd)`), `View() tea.View` (not string)
- bubbletea v2: no WithAltScreen() / WithKeyboardEnhancements() ProgramOptions; set via tea.View fields instead
- bubbletea v2 / ultraviolet: space key String() == "space" (not " ") because ultraviolet explicitly skips space in text short-circuit
- bubbletea v2 KeyboardEnhancements: request via v.KeyboardEnhancements.ReportEventTypes=true in View(); response via KeyboardEnhancementsMsg.SupportsEventTypes()
- KittyReportEventTypes flag value == 2 (from charmbracelet/x/ansi)
- fallbackStrategy bypasses 300ms arm hold by setting timer.WithArmHold(0); restored to 300ms on kitty upgrade via KeyboardEnhancementsMsg
- .gitignore had `tuibik` (bare name) which shadowed cmd/tuibik/ directory; fixed to `/tuibik` (root-anchored)

## Interface as Implemented

### internal/scramble (PR 1)

```go
type Move string
func (m Move) Face() byte
func Generate(n int, rng *rand.Rand) []Move
func Format(moves []Move) string
var axis = map[byte]int{'R':0,'L':0,'U':1,'D':1,'F':2,'B':2}
```

### internal/history (PR 3)

```go
type Solve struct { Index int; Duration time.Duration; Scramble string; At time.Time; DNF bool }
type AoResult struct { Duration time.Duration; DNF bool }
type Session struct { solves []Solve }

func New() *Session
func (s *Session) Add(sv Solve)
func (s *Session) Count() int
func (s *Session) Solves() []Solve          // newest-first copy
func (s *Session) Best() (time.Duration, bool)
func (s *Session) Mean() (time.Duration, bool)
func (s *Session) Average(window int) (AoResult, bool)
// window 100 → trim=5; all others → trim=1
```

### internal/ui (PR 4)

```go
type InputStrategy interface {
    HandlePress(t *timer.Timer) timer.TransitionResult
    HandleRelease(t *timer.Timer) timer.TransitionResult
    Badge() string
}
type kittyStrategy struct{}     // Badge() == "⚡ kitty"
type fallbackStrategy struct{ pressed bool }  // Badge() == "⌨ standard"

type Model struct { tmr, session, strategy, scramble, rng, width, height, styles }
func New(kitty bool) Model
func (m Model) Init() tea.Cmd
func (m Model) Update(msg tea.Msg) (tea.Model, tea.Cmd)
func (m Model) View() tea.View          // v2: returns tea.View not string
func (m Model) ModeBadge() string
func formatDuration(d time.Duration) string  // "S.ss" or "M:SS.ss"
```

### internal/timer (PR 2)

```go
type State int  // Idle, Armed, Running, Stopped
type Clock interface { Now() time.Time }
type TransitionResult struct { Changed bool; NewState State; StartTick, StopTick bool; Elapsed time.Duration }
type Timer struct { /* unexported */ armHold time.Duration /* default 300ms */ }

func New(clock Clock) *Timer
func NewDefault() *Timer                   // realClock, 300ms
func (t *Timer) WithArmHold(d time.Duration) *Timer
func (t *Timer) State() State
func (t *Timer) Elapsed() time.Duration    // live during Running, frozen after Stopped, 0 otherwise
func (t *Timer) Press() TransitionResult
func (t *Timer) Release() TransitionResult
func (t *Timer) Reset()
```
