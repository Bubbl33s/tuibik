//! Application state and the input reducer.

use std::cell::RefCell;

use cube::Cube;
use ratatui::widgets::ListState;
use scramble::{Scramble, Scrambler};
use stats::{StatValue, Summary};
use store::{Penalty, Solve, Store};

use crate::cube3d::{self, Axis, Mat3};
use crate::event::{Input, KeyContext, KeyMode};
use crate::sessions::{SessionMenu, SessionMode};
use crate::settings::{Row, Settings, ROWS};
use crate::timer::{Clock, Phase, Timer, TimerConfig};

/// How long a toast stays visible.
pub const TOAST_MS: u64 = 3000;
/// Solves shorter than this (that were not inspection timeouts) are treated as
/// key-autorepeat glitches and discarded.
const MIN_SOLVE_MS: i64 = 200;
/// Rotation of the 3D view per key press (degrees).
pub const VIEW_STEP_DEG: f32 = 15.0;
/// Auto-spin speed of the 3D view (degrees per second).
const SPIN_DEG_PER_S: f32 = 60.0;
/// Duration of an animated 90° axis turn in the 3D view (ms).
const TURN_MS: f32 = 200.0;

/// The modal layer shown over (or instead of parts of) the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Sessions,
    /// Step-by-step scramble preview (drawn inline, not as a popup).
    Preview,
    Help,
    Settings,
    Detail,
    ConfirmDelete,
    /// Interactive 3D view of the current cube.
    Cube3D,
}

/// Orientation, turn animation and spin state of the 3D cube view.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View3d {
    /// Rotation from cube to screen coordinates, as drawn now.
    pub current: Mat3,
    /// Where `current` is heading: fixed axis turns land here exactly.
    pub target: Mat3,
    /// Whether the cube keeps turning on its own.
    pub spin: bool,
    /// Timestamp of the last auto-spin step.
    last_spin_ms: Option<u64>,
    /// The axis turn being animated, if it has started.
    anim: Option<TurnAnim>,
}

/// An eased rotation of `from` by `angle` about the screen-space `axis`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TurnAnim {
    from: Mat3,
    axis: [f32; 3],
    angle: f32,
    start_ms: u64,
    duration_ms: f32,
}

impl Default for View3d {
    fn default() -> Self {
        let m = cube3d::default_orientation();
        View3d {
            current: m,
            target: m,
            spin: false,
            last_spin_ms: None,
            anim: None,
        }
    }
}

/// Smooth ease-in-out over `t` in 0..=1.
fn ease(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

impl View3d {
    /// Turn freely by `dx` degrees about the vertical screen axis and `dy`
    /// degrees about the horizontal one (no limit at the poles). Immediate:
    /// a running axis turn carries on from the rotated position.
    pub fn rotate_free(&mut self, dx: f32, dy: f32) {
        let r = cube3d::mat_mul(
            &cube3d::rotation(Axis::X, dy),
            &cube3d::rotation(Axis::Y, dx),
        );
        let turn = |m: &Mat3| cube3d::orthonormalize(&cube3d::mat_mul(&r, m));
        self.current = turn(&self.current);
        self.target = turn(&self.target);
        if let Some(a) = &mut self.anim {
            a.from = turn(&a.from);
            a.axis = cube3d::rotate_vec(&r, a.axis);
        }
    }

    /// Queue an exact 90° turn about the cube's own `axis` (x toward R, y
    /// toward U, z toward F; see [`cube3d::quarter_turn`]). [`View3d::advance`]
    /// animates toward it; turns pressed meanwhile chain onto the target.
    pub fn turn_axis(&mut self, axis: Axis, reverse: bool) {
        self.target = cube3d::mat_mul(&self.target, &cube3d::quarter_turn(axis, reverse));
        // Restart the easing from where the cube is drawn now.
        self.anim = None;
    }

    /// Whether an axis turn is still on its way.
    pub fn is_animating(&self) -> bool {
        self.current != self.target
    }

    /// Move the animation to `now`, landing exactly on the target when done.
    pub fn advance(&mut self, now: u64) {
        if !self.is_animating() {
            self.anim = None;
            return;
        }
        let a = *self.anim.get_or_insert_with(|| {
            let rel = cube3d::mat_mul(&self.target, &cube3d::transpose(&self.current));
            let (axis, angle) = cube3d::axis_angle(&rel);
            TurnAnim {
                from: self.current,
                axis,
                angle,
                start_ms: now,
                duration_ms: TURN_MS * angle / std::f32::consts::FRAC_PI_2,
            }
        });
        let t = now.saturating_sub(a.start_ms) as f32 / a.duration_ms;
        if a.duration_ms <= 0.0 || t >= 1.0 {
            self.current = self.target;
            self.anim = None;
        } else {
            let r = cube3d::rotation_about(a.axis, a.angle * ease(t));
            self.current = cube3d::mat_mul(&r, &a.from);
        }
    }

    /// Jump to the end of any running turn.
    fn finish_turn(&mut self) {
        self.current = self.target;
        self.anim = None;
    }

    /// Back to the default orientation (spin is left as it is).
    pub fn reset(&mut self) {
        let spin = self.spin;
        let last = self.last_spin_ms;
        *self = View3d::default();
        self.spin = spin;
        self.last_spin_ms = last;
    }

    fn set_spin(&mut self, on: bool, now: u64) {
        self.spin = on;
        self.last_spin_ms = on.then_some(now);
    }

    /// Advance auto-spin and any axis turn to `now`.
    fn tick(&mut self, now: u64) {
        if self.spin {
            if let Some(last) = self.last_spin_ms {
                let dt = now.saturating_sub(last) as f32 / 1000.0;
                self.rotate_free(dt * SPIN_DEG_PER_S, 0.0);
            }
            self.last_spin_ms = Some(now);
        }
        self.advance(now);
    }
}

/// Which screen is shown, derived from the timer phase (never stored).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Only the big timer, full-screen.
    Focus,
    Dashboard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Success,
    Warn,
}

/// A short-lived status message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub text: String,
    pub kind: ToastKind,
    deadline_ms: u64,
}

/// A personal best that a new solve achieved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbKind {
    Single,
    Ao5,
    Ao12,
}

impl PbKind {
    pub fn label(self) -> &'static str {
        match self {
            PbKind::Single => "single",
            PbKind::Ao5 => "ao5",
            PbKind::Ao12 => "ao12",
        }
    }
}

/// Feedback about the most recently recorded solve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastResult {
    pub solve_id: i64,
    /// Effective time in ms (`None` for a DNF).
    pub ms: Option<i64>,
    pub penalty: Penalty,
    /// Effective time minus the previous solve's, when both are finishes.
    pub delta_prev: Option<i64>,
    pub pbs: Vec<PbKind>,
}

/// The top-level application state.
pub struct App {
    /// When true, the event loop should exit.
    pub should_quit: bool,
    /// The scrambler (holds the precomputed Kociemba tables).
    scrambler: Scrambler,
    /// The current scramble being shown.
    pub scramble: Scramble,
    /// The cube state after applying the current scramble.
    pub cube: Cube,
    /// The timer state machine.
    pub timer: Timer,
    /// Monotonic clock base.
    clock: Clock,
    /// Whether the terminal reports key-release events (Kitty protocol).
    pub enhanced: bool,
    /// Persistence layer.
    store: Store,
    /// The active session id.
    pub session_id: i64,
    /// The active session's name (for the header).
    pub session_name: String,
    /// Cached solves for the active session (chronological order).
    pub solves: Vec<Solve>,
    /// Cached statistics for `solves`.
    summary: Summary,
    /// Selected index in the history list (0 = most recent).
    pub history_selected: usize,
    /// Scroll state of the history list (kept between frames).
    pub history_state: RefCell<ListState>,
    /// The open overlay.
    pub overlay: Overlay,
    /// Where cancelling the delete confirmation returns to.
    confirm_return: Overlay,
    /// The session menu overlay state (when `overlay == Sessions`).
    pub session_menu: Option<SessionMenu>,
    /// The step index for scramble preview (0 = solved, N = fully scrambled).
    pub preview_step: usize,
    /// The active theme id.
    pub theme_id: String,
    /// Persisted preferences.
    pub settings: Settings,
    /// The selected row in the settings overlay.
    pub settings_selected: usize,
    /// Feedback for the last solve, cleared when the next one starts.
    pub last_result: Option<LastResult>,
    /// The current toast, if any.
    pub toast: Option<Toast>,
    /// Orientation of the 3D cube view (kept between openings).
    pub view3d: View3d,
}

impl App {
    /// Build the app with a fresh scrambler, a store, and an initial scramble.
    pub fn new(enhanced: bool) -> anyhow::Result<Self> {
        let store = Store::open_default()?;
        Self::with_store(store, enhanced)
    }

    /// Build the app around a provided store (used by tests with in-memory DB).
    pub fn with_store(store: Store, enhanced: bool) -> anyhow::Result<Self> {
        let mut scrambler = Scrambler::new();
        let scramble = scrambler
            .generate()
            .map_err(|e| anyhow::anyhow!("scramble generation failed: {e}"))?;
        Self::assemble(store, scrambler, scramble, enhanced)
    }

    fn assemble(
        store: Store,
        scrambler: Scrambler,
        scramble: Scramble,
        enhanced: bool,
    ) -> anyhow::Result<Self> {
        let default_id = store.ensure_default_session()?;
        // Prefer the persisted active session if it still exists.
        let session_id = match store.active_session_id()? {
            Some(id) if store.get_session(id)?.is_some() => id,
            _ => default_id,
        };
        store.set_active_session(session_id)?;
        let solves = store.list_solves(session_id)?;
        let session_name = store
            .get_session(session_id)?
            .map(|s| s.name)
            .unwrap_or_default();

        // Load the persisted theme, defaulting to the first theme.
        let theme_id = store
            .get_config("theme")?
            .map(|id| crate::theme::by_id(&id).id.to_string())
            .unwrap_or_else(|| crate::theme::all()[0].id.to_string());
        let settings = Settings::load(&store);
        let cube = scramble.cube();
        let summary = stats::summary(&solves);

        let mut app = App {
            should_quit: false,
            scrambler,
            scramble,
            cube,
            timer: Timer::with_config(TimerConfig {
                inspection: settings.inspection,
                fallback: !enhanced,
            }),
            clock: Clock::new(),
            enhanced,
            store,
            session_id,
            session_name,
            solves,
            summary,
            history_selected: 0,
            history_state: RefCell::new(ListState::default()),
            overlay: Overlay::None,
            confirm_return: Overlay::None,
            session_menu: None,
            preview_step: 0,
            theme_id,
            settings,
            settings_selected: 0,
            last_result: None,
            toast: None,
            view3d: View3d::default(),
        };
        if !enhanced {
            app.notify_for(
                "Tap-to-start mode: this terminal has no key-release events",
                ToastKind::Warn,
                6000,
            );
        }
        Ok(app)
    }

    /// Deterministic constructor for tests: seeded scrambler + in-memory store.
    #[cfg(test)]
    pub fn test_app(seed: u64, enhanced: bool) -> Self {
        let store = Store::open_in_memory().unwrap();
        let mut scrambler = Scrambler::with_seed(seed);
        let scramble = scrambler.generate().unwrap();
        Self::assemble(store, scrambler, scramble, enhanced).unwrap()
    }

    /// Test helper: record a solve of exactly `ms` by driving the timer with
    /// explicit timestamps, then persisting via `after_stop`. Bypasses the
    /// real clock so tests are deterministic and fast.
    #[cfg(test)]
    pub fn test_record_solve(&mut self, ms: u64) {
        use crate::timer::{ARM_THRESHOLD_MS, RELEASE_DEBOUNCE_MS};
        self.timer = Timer::new();
        self.timer.press(0);
        self.timer.tick(ARM_THRESHOLD_MS + 1); // -> Ready
        self.timer.release(ARM_THRESHOLD_MS + 1);
        let run_base = ARM_THRESHOLD_MS + 1 + RELEASE_DEBOUNCE_MS + 1;
        self.timer.tick(run_base); // -> Running (run_started = release ts)
        self.timer.stop(ARM_THRESHOLD_MS + 1 + ms);
        self.after_stop();
        self.timer = Timer::with_config(self.timer_config());
    }

    /// Test helper: record a solve and give it a penalty.
    #[cfg(test)]
    pub fn test_record_solve_with(&mut self, ms: u64, penalty: Penalty) {
        self.test_record_solve(ms);
        self.history_selected = 0;
        self.set_selected_penalty(penalty);
    }

    /// The timer configuration implied by settings and the terminal.
    fn timer_config(&self) -> TimerConfig {
        TimerConfig {
            inspection: self.settings.inspection,
            fallback: !self.enhanced,
        }
    }

    /// Generate a fresh scramble and update the cube.
    pub fn new_scramble(&mut self) {
        if let Ok(s) = self.scrambler.generate() {
            self.cube = s.cube();
            self.scramble = s;
        }
    }

    /// Reload the cached solves and statistics for the active session,
    /// keeping the selection inside the list.
    fn reload_solves(&mut self) {
        if let Ok(list) = self.store.list_solves(self.session_id) {
            self.solves = list;
            self.summary = stats::summary(&self.solves);
            self.clamp_selection();
        }
    }

    fn clamp_selection(&mut self) {
        let max = self.solves.len().saturating_sub(1);
        if self.history_selected > max {
            self.history_selected = max;
        }
    }

    /// The (cached) summary statistics for the active session.
    pub fn summary(&self) -> &Summary {
        &self.summary
    }

    /// Solves in display order (most recent first).
    #[allow(dead_code)]
    pub fn solves_recent_first(&self) -> Vec<&Solve> {
        self.solves.iter().rev().collect()
    }

    /// Index into `solves` (chronological) of the selected history row.
    fn selected_index(&self) -> Option<usize> {
        if self.solves.is_empty() {
            None
        } else {
            Some(self.solves.len() - 1 - self.history_selected.min(self.solves.len() - 1))
        }
    }

    /// The selected solve.
    pub fn selected_solve(&self) -> Option<&Solve> {
        self.selected_index().map(|i| &self.solves[i])
    }

    /// The 1-based number of the selected solve within the session.
    pub fn selected_number(&self) -> usize {
        self.selected_index().map_or(0, |i| i + 1)
    }

    /// The active theme.
    pub fn theme(&self) -> &'static crate::theme::Theme {
        crate::theme::by_id(&self.theme_id)
    }

    /// Cycle to the next theme and persist the choice.
    pub fn cycle_theme(&mut self) {
        let next = crate::theme::next(&self.theme_id);
        self.set_theme(next.id);
    }

    fn set_theme(&mut self, id: &str) {
        self.theme_id = crate::theme::by_id(id).id.to_string();
        let _ = self.store.set_config("theme", &self.theme_id);
        self.notify(format!("Theme: {}", self.theme().name), ToastKind::Info);
    }

    /// Current time in ms on the app clock.
    pub fn now(&self) -> u64 {
        self.clock.now_ms()
    }

    /// The overlay that cancelling the delete confirmation returns to.
    pub fn confirm_return(&self) -> Overlay {
        self.confirm_return
    }

    /// Which screen to draw: focus while the timer is engaged.
    pub fn screen(&self) -> Screen {
        if self.timer.is_active() {
            Screen::Focus
        } else {
            Screen::Dashboard
        }
    }

    /// The context the event layer needs to classify keys.
    pub fn key_context(&self) -> KeyContext {
        let mode = if self.timer.is_active() {
            KeyMode::Timing
        } else if self.session_is_editing() {
            KeyMode::TextEntry
        } else {
            KeyMode::Normal
        };
        KeyContext {
            enhanced: self.enhanced,
            mode,
        }
    }

    // ===== Toasts =====

    /// Show a toast for the default duration, replacing any current one.
    pub fn notify(&mut self, text: impl Into<String>, kind: ToastKind) {
        self.notify_for(text, kind, TOAST_MS);
    }

    fn notify_for(&mut self, text: impl Into<String>, kind: ToastKind, ms: u64) {
        self.toast = Some(Toast {
            text: text.into(),
            kind,
            deadline_ms: self.now() + ms,
        });
    }

    // ===== Input =====

    /// Handle a classified input event.
    pub fn handle_input(&mut self, input: Input) {
        let now = self.now();
        self.handle_input_at(input, now);
    }

    /// Like [`App::handle_input`] with an explicit timestamp (deterministic in tests).
    pub fn handle_input_at(&mut self, input: Input, now: u64) {
        if input == Input::ForceQuit {
            self.should_quit = true;
            return;
        }
        // While the timer is engaged it owns the keyboard: shortcuts are
        // suppressed so a stray key can't fire them.
        if self.timer.is_active() {
            self.handle_timing_input(input, now);
            return;
        }
        match self.overlay {
            Overlay::None => self.handle_dashboard_input(input, now),
            Overlay::Sessions => self.handle_session_input(input),
            Overlay::Preview => self.handle_preview_input(input),
            Overlay::Help => {
                if matches!(input, Input::Cancel | Input::Help | Input::Quit) {
                    self.overlay = Overlay::None;
                }
            }
            Overlay::Settings => self.handle_settings_input(input),
            Overlay::Detail => self.handle_detail_input(input),
            Overlay::ConfirmDelete => self.handle_confirm_delete_input(input),
            Overlay::Cube3D => self.handle_cube3d_input(input, now),
        }
    }

    fn handle_timing_input(&mut self, input: Input, now: u64) {
        match input {
            // Space press/repeat = key still held (feeds arming / stops run).
            Input::HoldPress | Input::HoldRepeat => self.on_hold_press(now),
            // Release is debounced (a following press cancels it); a real
            // release is committed by tick().
            Input::HoldRelease => self.timer.release(now),
            Input::None => {}
            // Esc cancels arming/ready/inspection...
            Input::Cancel if self.timer.phase != Phase::Running => self.timer.cancel(),
            // ...and any other key (Esc included) stops a running solve.
            _ => self.on_other_key(now),
        }
    }

    fn handle_dashboard_input(&mut self, input: Input, now: u64) {
        match input {
            Input::Quit => self.should_quit = true,
            Input::NewScramble => self.new_scramble(),
            // First Space press (or repeat) begins arming / inspection / the run.
            Input::HoldPress | Input::HoldRepeat => self.on_hold_press(now),
            Input::Up => self.history_up(),
            Input::Down => self.history_down(),
            Input::JumpTop => self.history_selected = 0,
            Input::JumpBottom => self.history_selected = self.solves.len().saturating_sub(1),
            Input::Confirm => self.open_detail(),
            Input::Penalty(p) => self.set_selected_penalty(p),
            Input::Delete => self.request_delete(),
            Input::OpenSessions => self.open_sessions(),
            Input::TogglePreview => self.open_preview(),
            Input::ToggleTheme => self.cycle_theme(),
            Input::Help => self.overlay = Overlay::Help,
            Input::Settings => self.open_settings(),
            Input::Toggle3d => self.overlay = Overlay::Cube3D,
            _ => {}
        }
    }

    // ===== 3D view =====

    fn handle_cube3d_input(&mut self, input: Input, now: u64) {
        let step = VIEW_STEP_DEG;
        match input {
            Input::Toggle3d | Input::Cancel | Input::Quit => {
                self.view3d.set_spin(false, now);
                self.view3d.finish_turn();
                self.overlay = Overlay::None;
            }
            Input::ViewAxis(axis, reverse) => self.view3d.turn_axis(axis, reverse),
            Input::Yes => self.view3d.turn_axis(Axis::Y, false),
            Input::Left => self.view3d.rotate_free(-step, 0.0),
            Input::Right => self.view3d.rotate_free(step, 0.0),
            Input::Up => self.view3d.rotate_free(0.0, -step),
            Input::Down => self.view3d.rotate_free(0.0, step),
            Input::ToggleSpin => {
                let on = !self.view3d.spin;
                self.view3d.set_spin(on, now);
            }
            Input::ResetView | Input::JumpTop => self.view3d.reset(),
            _ => {}
        }
    }

    /// Whether the screen changes on its own and needs frequent redraws.
    pub fn needs_fast_ticks(&self) -> bool {
        self.timer.is_active()
            || (self.overlay == Overlay::Cube3D && self.view3d.spin)
            || self.is_animating()
    }

    /// Whether a 3D axis turn is animating (redraw at full frame rate).
    pub fn is_animating(&self) -> bool {
        self.overlay == Overlay::Cube3D && self.view3d.is_animating()
    }

    // ===== Scramble preview =====

    /// The number of moves in the current scramble.
    pub fn scramble_len(&self) -> usize {
        self.scramble.moves.len()
    }

    /// Open the step-by-step preview (starts at fully scrambled).
    pub fn open_preview(&mut self) {
        self.preview_step = self.scramble_len();
        self.overlay = Overlay::Preview;
    }

    /// Close the preview.
    pub fn close_preview(&mut self) {
        self.overlay = Overlay::None;
    }

    /// The cube state at the current preview step (prefix of the scramble).
    pub fn preview_cube(&self) -> Cube {
        let k = self.preview_step.min(self.scramble_len());
        Cube::solved().with_sequence(&self.scramble.moves[..k])
    }

    /// The facelet indices affected by the move that leads into the current
    /// step (i.e. move at index `preview_step - 1`), for highlighting. Returns
    /// empty when at step 0.
    pub fn preview_highlight(&self) -> Vec<usize> {
        if self.preview_step == 0 || self.preview_step > self.scramble_len() {
            return Vec::new();
        }
        // Highlight the face of the move just applied: the 9 facelets of that
        // face in the resulting cube.
        let mv = self.scramble.moves[self.preview_step - 1];
        let base = mv.face.index() * 9;
        (base..base + 9).collect()
    }

    fn handle_preview_input(&mut self, input: Input) {
        match input {
            Input::TogglePreview | Input::Cancel | Input::Quit => self.close_preview(),
            Input::Left | Input::Up if self.preview_step > 0 => self.preview_step -= 1,
            Input::Right | Input::Down if self.preview_step < self.scramble_len() => {
                self.preview_step += 1
            }
            _ => {}
        }
    }

    // ===== Settings =====

    pub fn open_settings(&mut self) {
        self.settings_selected = 0;
        self.overlay = Overlay::Settings;
    }

    fn handle_settings_input(&mut self, input: Input) {
        match input {
            Input::Cancel | Input::Settings | Input::Quit => self.overlay = Overlay::None,
            Input::Up => self.settings_selected = self.settings_selected.saturating_sub(1),
            Input::Down => {
                if self.settings_selected + 1 < ROWS.len() {
                    self.settings_selected += 1;
                }
            }
            Input::Confirm | Input::HoldPress | Input::Right => self.change_setting(true),
            Input::Left => self.change_setting(false),
            _ => {}
        }
    }

    /// Change the selected setting (`forward` matters only for the theme) and
    /// apply/persist it immediately.
    fn change_setting(&mut self, forward: bool) {
        match ROWS[self.settings_selected.min(ROWS.len() - 1)] {
            Row::Inspection => {
                self.settings.inspection = !self.settings.inspection;
                self.settings.save(&self.store);
                self.timer.set_config(self.timer_config());
            }
            Row::ShowRunningTime => {
                self.settings.show_running_time = !self.settings.show_running_time;
                self.settings.save(&self.store);
            }
            Row::Theme => {
                let t = if forward {
                    crate::theme::next(&self.theme_id)
                } else {
                    crate::theme::prev(&self.theme_id)
                };
                self.set_theme(t.id);
            }
        }
    }

    // ===== Solve detail, penalties, deletion =====

    fn open_detail(&mut self) {
        if !self.solves.is_empty() {
            self.overlay = Overlay::Detail;
        }
    }

    fn handle_detail_input(&mut self, input: Input) {
        match input {
            Input::Cancel | Input::Confirm | Input::Quit => self.overlay = Overlay::None,
            Input::Penalty(p) => self.set_selected_penalty(p),
            Input::Delete => self.request_delete(),
            Input::Up => self.history_up(),
            Input::Down => self.history_down(),
            _ => {}
        }
    }

    /// Set the penalty of the selected solve and refresh everything derived.
    pub fn set_selected_penalty(&mut self, penalty: Penalty) {
        let Some(solve) = self.selected_solve() else {
            return;
        };
        let id = solve.id;
        let number = self.selected_number();
        let _ = self.store.set_penalty(id, penalty);
        self.reload_solves();
        self.refresh_last_result(id);
        let label = match penalty {
            Penalty::Ok => "OK",
            Penalty::PlusTwo => "+2",
            Penalty::Dnf => "DNF",
        };
        self.notify(format!("Solve #{number}: {label}"), ToastKind::Success);
    }

    /// Ask for confirmation before deleting the selected solve.
    fn request_delete(&mut self) {
        if self.solves.is_empty() {
            return;
        }
        self.confirm_return = self.overlay;
        self.overlay = Overlay::ConfirmDelete;
    }

    fn handle_confirm_delete_input(&mut self, input: Input) {
        match input {
            Input::Yes | Input::Delete => {
                self.delete_selected_solve();
                self.overlay = Overlay::None;
            }
            // `n` (NewScramble) doubles as "no" here.
            Input::Cancel | Input::NewScramble | Input::Quit => {
                self.overlay = self.confirm_return;
            }
            _ => {}
        }
    }

    /// Delete the selected solve (no confirmation; the dialog is the caller's
    /// job). The selection keeps its list position, clamped.
    pub fn delete_selected_solve(&mut self) {
        let Some(solve) = self.selected_solve() else {
            return;
        };
        let id = solve.id;
        let _ = self.store.delete_solve(id);
        self.reload_solves();
        if self.last_result.as_ref().is_some_and(|l| l.solve_id == id) {
            self.last_result = None;
        }
        self.notify("Solve deleted", ToastKind::Info);
    }

    /// Move the history selection toward newer solves.
    pub fn history_up(&mut self) {
        if self.history_selected > 0 {
            self.history_selected -= 1;
        }
    }

    /// Move the history selection toward older solves.
    pub fn history_down(&mut self) {
        let max = self.solves.len().saturating_sub(1);
        if self.history_selected < max {
            self.history_selected += 1;
        }
    }

    // ===== Session management =====

    /// Open the session overlay.
    pub fn open_sessions(&mut self) {
        if let Ok(sessions) = self.store.list_sessions() {
            self.session_menu = Some(SessionMenu::new(sessions, self.session_id));
            self.overlay = Overlay::Sessions;
        }
    }

    /// Close the session overlay.
    pub fn close_sessions(&mut self) {
        self.overlay = Overlay::None;
        self.session_menu = None;
    }

    /// Switch the active session, reloading solves and persisting the choice.
    fn switch_session(&mut self, id: i64) {
        self.session_id = id;
        let _ = self.store.set_active_session(id);
        self.history_selected = 0;
        self.last_result = None;
        self.reload_solves();
        self.refresh_session_name();
        self.notify(format!("Session: {}", self.session_name), ToastKind::Info);
    }

    fn refresh_session_name(&mut self) {
        if let Ok(Some(s)) = self.store.get_session(self.session_id) {
            self.session_name = s.name;
        }
    }

    /// Handle input while the session overlay is open.
    fn handle_session_input(&mut self, input: Input) {
        // Determine the sub-mode to route text vs. navigation.
        let sub = self
            .session_menu
            .as_ref()
            .map(|m| m.mode.clone())
            .unwrap_or(SessionMode::Browse);

        match sub {
            SessionMode::Creating { .. } => self.session_text_input(input, true),
            SessionMode::Renaming { .. } => self.session_text_input(input, false),
            SessionMode::ConfirmDelete => self.session_confirm_delete_input(input),
            SessionMode::Browse => self.session_browse_input(input),
        }
    }

    fn session_browse_input(&mut self, input: Input) {
        let Some(menu) = self.session_menu.as_mut() else {
            return;
        };
        match input {
            Input::Up => menu.up(),
            Input::Down => menu.down(),
            Input::OpenSessions | Input::Quit | Input::Cancel => self.close_sessions(),
            // Enter / Space switches to the selected session.
            Input::Confirm | Input::HoldPress => {
                if let Some(sel) = menu.selected_session() {
                    let id = sel.id;
                    self.switch_session(id);
                    self.close_sessions();
                }
            }
            Input::NewScramble => menu.begin_create(), // 'n' = new session
            Input::Rename => menu.begin_rename(),      // 'r' = rename
            Input::Delete => menu.begin_confirm_delete(), // 'd' = delete
            _ => {}
        }
    }

    fn session_confirm_delete_input(&mut self, input: Input) {
        match input {
            Input::Cancel | Input::Quit | Input::NewScramble => {
                if let Some(m) = self.session_menu.as_mut() {
                    m.cancel_edit();
                }
            }
            Input::Delete | Input::Yes => self.perform_delete_session(),
            _ => {}
        }
    }

    fn perform_delete_session(&mut self) {
        let (del_id, remaining_first) = {
            let Some(menu) = self.session_menu.as_ref() else {
                return;
            };
            let Some(sel) = menu.selected_session() else {
                return;
            };
            let del_id = sel.id;
            let first_other = menu.sessions.iter().find(|s| s.id != del_id).map(|s| s.id);
            (del_id, first_other)
        };
        let _ = self.store.delete_session(del_id);
        // If we deleted the active session, switch to another.
        if self.session_id == del_id {
            if let Some(other) = remaining_first {
                self.switch_session(other);
            }
        }
        // Refresh the menu list.
        if let Ok(sessions) = self.store.list_sessions() {
            self.session_menu = Some(SessionMenu::new(sessions, self.session_id));
        }
    }

    fn session_text_input(&mut self, input: Input, creating: bool) {
        match input {
            Input::Cancel => {
                if let Some(m) = self.session_menu.as_mut() {
                    m.cancel_edit();
                }
            }
            Input::Confirm => self.commit_session_text(creating),
            Input::Char(c) => self.push_session_char(c),
            Input::Backspace => self.session_backspace(),
            _ => {}
        }
    }

    /// Push a printable char into the session text editor.
    pub fn push_session_char(&mut self, c: char) {
        if let Some(m) = self.session_menu.as_mut() {
            m.push_char(c);
        }
    }

    /// Backspace in the session text editor.
    pub fn session_backspace(&mut self) {
        if let Some(m) = self.session_menu.as_mut() {
            m.backspace();
        }
    }

    /// Confirm the current session text (create or rename).
    pub fn commit_session_text(&mut self, creating: bool) {
        let name = self
            .session_menu
            .as_ref()
            .and_then(|m| m.current_input())
            .map(|s| s.trim().to_string());
        let Some(name) = name else { return };
        if name.is_empty() {
            if let Some(m) = self.session_menu.as_mut() {
                m.cancel_edit();
            }
            return;
        }
        if creating {
            if let Ok(id) = self.store.create_session(&name) {
                self.switch_session(id);
            }
        } else if let Some(sel_id) = self
            .session_menu
            .as_ref()
            .and_then(|m| m.selected_session())
            .map(|s| s.id)
        {
            let _ = self.store.rename_session(sel_id, &name);
            self.refresh_session_name();
        }
        // Refresh menu.
        if let Ok(sessions) = self.store.list_sessions() {
            self.session_menu = Some(SessionMenu::new(sessions, self.session_id));
        }
    }

    /// Whether the session overlay is currently in a text-editing sub-mode.
    pub fn session_is_editing(&self) -> bool {
        self.overlay == Overlay::Sessions
            && matches!(
                self.session_menu.as_ref().map(|m| &m.mode),
                Some(SessionMode::Creating { .. }) | Some(SessionMode::Renaming { .. })
            )
    }

    // ===== Timing =====

    /// A hold-key press (or autorepeat). Feeds the timer's press logic. If it
    /// stops a running solve, persist it.
    fn on_hold_press(&mut self, now: u64) {
        let was_running = self.timer.phase == Phase::Running;
        self.timer.press(now);
        if was_running {
            if self.timer.has_result() {
                self.after_stop();
            }
        } else if matches!(self.timer.phase, Phase::Running | Phase::Inspecting) {
            self.last_result = None;
        }
    }

    fn on_other_key(&mut self, now: u64) {
        if self.timer.phase == Phase::Running {
            self.timer.stop(now);
            self.after_stop();
        }
    }

    /// After the timer stops: persist the solve, reload history/stats, work
    /// out the post-solve feedback, and generate a new scramble.
    fn after_stop(&mut self) {
        if !self.timer.has_result() {
            return;
        }
        let time_ms = self.timer.result_ms() as i64;
        let penalty = self.timer.result_penalty();
        self.timer.consume_result();
        // Guard against spurious near-zero solves caused by keyboard
        // autorepeat (rapid press/release pairs). A real solve is never this
        // short; inspection-timeout DNFs (raw 0) are exempt.
        if penalty != Penalty::Dnf && time_ms < MIN_SOLVE_MS {
            return;
        }
        let prev = self.summary.clone();
        let scramble_text = self.scramble.text.clone();
        // Persist; ignore errors so a DB hiccup doesn't crash the timer.
        let id = self
            .store
            .add_solve(self.session_id, time_ms, &scramble_text, penalty)
            .ok();
        self.history_selected = 0;
        self.reload_solves();
        if let Some(id) = id {
            let pbs = detect_pbs(&prev, &self.summary);
            let mut result = self.build_last_result(id);
            result.pbs = pbs;
            if penalty == Penalty::Dnf && time_ms == 0 {
                self.notify("Inspection over 17 s: DNF", ToastKind::Warn);
            } else if let Some(pb) = result.pbs.first() {
                let text = format!("New PB {}!", pb.label());
                self.notify(text, ToastKind::Success);
            }
            self.last_result = Some(result);
        }
        self.new_scramble();
    }

    /// Effective time and delta-vs-previous for the solve with `id`.
    fn build_last_result(&self, id: i64) -> LastResult {
        let idx = self.solves.iter().position(|s| s.id == id);
        let solve = idx.map(|i| &self.solves[i]);
        let ms = solve.and_then(stats::effective_ms);
        let delta_prev = match (idx, ms) {
            (Some(i), Some(now)) if i > 0 => {
                stats::effective_ms(&self.solves[i - 1]).map(|prev| now - prev)
            }
            _ => None,
        };
        LastResult {
            solve_id: id,
            ms,
            penalty: solve.map_or(Penalty::Ok, |s| s.penalty),
            delta_prev,
            pbs: Vec::new(),
        }
    }

    /// Recompute the last-result feedback after its solve was edited.
    fn refresh_last_result(&mut self, id: i64) {
        if self.last_result.as_ref().is_some_and(|l| l.solve_id == id) {
            self.last_result = Some(self.build_last_result(id));
        }
    }

    /// Advance time-based state using the real clock.
    pub fn tick(&mut self) {
        let now = self.now();
        self.tick_at(now);
    }

    /// Advance time-based state at an explicit timestamp.
    pub fn tick_at(&mut self, now: u64) {
        self.timer.tick(now);
        // An inspection timeout stops the timer with a DNF result.
        if self.timer.has_result() {
            self.after_stop();
        }
        if matches!(self.timer.phase, Phase::Running | Phase::Inspecting) {
            self.last_result = None;
        }
        if self.toast.as_ref().is_some_and(|t| now >= t.deadline_ms) {
            self.toast = None;
        }
        if self.overlay == Overlay::Cube3D {
            self.view3d.tick(now);
        }
    }
}

/// Which records a new solve beat. The first value never counts as a PB.
fn detect_pbs(prev: &Summary, new: &Summary) -> Vec<PbKind> {
    fn better(prev: StatValue, new: StatValue) -> bool {
        matches!((prev, new), (StatValue::Time(p), StatValue::Time(n)) if n < p)
    }
    let mut pbs = Vec::new();
    if matches!((prev.best, new.best), (Some(p), Some(n)) if n < p) {
        pbs.push(PbKind::Single);
    }
    if better(prev.best_ao5, new.best_ao5) {
        pbs.push(PbKind::Ao5);
    }
    if better(prev.best_ao12, new.best_ao12) {
        pbs.push(PbKind::Ao12);
    }
    pbs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timer::{ARM_THRESHOLD_MS, RELEASE_DEBOUNCE_MS};

    #[test]
    fn quit_sets_flag_when_idle() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn initial_scramble_present_no_solves() {
        let app = App::test_app(1, true);
        assert!(!app.scramble.moves.is_empty());
        assert!(!app.cube.is_solved());
        assert_eq!(app.solves.len(), 0);
        assert_eq!(app.summary().count, 0);
    }

    #[test]
    fn completing_a_solve_persists_and_updates_stats() {
        let mut app = App::test_app(1, false);
        app.test_record_solve(3000); // a real 3s solve
        assert_eq!(app.solves.len(), 1);
        assert_eq!(app.summary().count, 1);
        assert!(app.summary().best.is_some());
    }

    #[test]
    fn solves_persist_across_reopen_same_store_path() {
        // Use a temp file-backed store to verify persistence across App builds.
        let dir = std::env::temp_dir().join(format!("tuibik-app-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("db.sqlite");

        {
            let store = Store::open(&path).unwrap();
            let mut app = App::with_store(store, false).unwrap();
            app.test_record_solve(3000);
            assert_eq!(app.solves.len(), 1);
        }
        {
            let store = Store::open(&path).unwrap();
            let app = App::with_store(store, false).unwrap();
            assert_eq!(app.solves.len(), 1, "solve should persist across reopen");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn history_navigation_clamps() {
        let mut app = App::test_app(1, false);
        app.test_record_solve(2000);
        app.test_record_solve(3000);
        assert_eq!(app.solves.len(), 2);
        app.history_selected = 0;
        app.history_up();
        assert_eq!(app.history_selected, 0, "clamp at top");
        app.history_down();
        assert_eq!(app.history_selected, 1);
        app.history_down();
        assert_eq!(app.history_selected, 1, "clamp at bottom");
    }

    #[test]
    fn jump_keys_go_to_newest_and_oldest() {
        let mut app = App::test_app(1, true);
        for i in 0..5 {
            app.test_record_solve(2000 + i * 100);
        }
        app.handle_input(Input::JumpBottom);
        assert_eq!(app.history_selected, 4);
        app.handle_input(Input::JumpTop);
        assert_eq!(app.history_selected, 0);
    }

    #[test]
    fn new_solve_selects_newest() {
        let mut app = App::test_app(1, true);
        app.test_record_solve(2000);
        app.test_record_solve(3000);
        app.history_selected = 1;
        app.test_record_solve(4000);
        assert_eq!(app.history_selected, 0);
    }

    #[test]
    fn open_and_close_session_overlay() {
        let mut app = App::test_app(1, false);
        assert_eq!(app.overlay, Overlay::None);
        app.handle_input(Input::OpenSessions);
        assert_eq!(app.overlay, Overlay::Sessions);
        assert!(app.session_menu.is_some());
        // Pressing 's' again closes.
        app.handle_input(Input::OpenSessions);
        assert_eq!(app.overlay, Overlay::None);
    }

    #[test]
    fn create_session_switches_and_isolates_solves() {
        let mut app = App::test_app(1, false);
        app.test_record_solve(2800);
        assert_eq!(app.solves.len(), 1);
        let default_id = app.session_id;

        // Open sessions, create a new one via the text-entry path.
        app.handle_input(Input::OpenSessions);
        app.handle_input(Input::NewScramble); // begin_create
        assert!(app.session_is_editing());
        assert_eq!(app.key_context().mode, KeyMode::TextEntry);
        app.handle_input(Input::Char('O'));
        app.handle_input(Input::Char('H'));
        app.handle_input(Input::Confirm);

        assert_ne!(app.session_id, default_id);
        assert_eq!(app.session_name, "OH");
        app.close_sessions();
        assert_eq!(app.solves.len(), 0, "new session starts empty");

        app.switch_session(default_id);
        assert_eq!(app.solves.len(), 1);
    }

    #[test]
    fn session_text_entry_accepts_letters_that_are_shortcuts() {
        let mut app = App::test_app(1, false);
        app.handle_input(Input::OpenSessions);
        app.handle_input(Input::NewScramble);
        for c in "q d".chars() {
            app.handle_input(Input::Char(c));
        }
        assert_eq!(
            app.session_menu.as_ref().unwrap().current_input(),
            Some("q d")
        );
        assert!(!app.should_quit);
        app.handle_input(Input::Backspace);
        assert_eq!(
            app.session_menu.as_ref().unwrap().current_input(),
            Some("q ")
        );
        app.handle_input(Input::Cancel);
        assert!(!app.session_is_editing());
        assert_eq!(app.overlay, Overlay::Sessions);
    }

    #[test]
    fn rename_session_updates_name() {
        let mut app = App::test_app(1, false);
        app.handle_input(Input::OpenSessions);
        app.handle_input(Input::Rename); // begin_rename (prefills)
        for _ in 0..40 {
            app.handle_input(Input::Backspace);
        }
        for c in "Renamed".chars() {
            app.handle_input(Input::Char(c));
        }
        app.handle_input(Input::Confirm);
        let sessions = app.session_menu.as_ref().unwrap().sessions.clone();
        assert!(sessions.iter().any(|s| s.name == "Renamed"));
        assert_eq!(app.session_name, "Renamed", "header follows the rename");
    }

    #[test]
    fn delete_session_removes_it_and_switches() {
        let mut app = App::test_app(1, false);
        app.handle_input(Input::OpenSessions);
        app.handle_input(Input::NewScramble);
        for c in "Temp".chars() {
            app.handle_input(Input::Char(c));
        }
        app.handle_input(Input::Confirm);
        let temp_id = app.session_id;
        app.handle_input(Input::Delete); // begin_confirm_delete
        app.handle_input(Input::Delete); // confirm
        assert_ne!(app.session_id, temp_id);
        let sessions = app.session_menu.as_ref().unwrap().sessions.clone();
        assert!(!sessions.iter().any(|s| s.id == temp_id));
    }

    #[test]
    fn preview_opens_at_full_scramble() {
        let mut app = App::test_app(1, false);
        let n = app.scramble_len();
        app.handle_input(Input::TogglePreview);
        assert_eq!(app.overlay, Overlay::Preview);
        assert_eq!(app.preview_step, n);
        assert_eq!(app.preview_cube(), app.cube);
    }

    #[test]
    fn preview_navigation_and_prefixes() {
        let mut app = App::test_app(1, false);
        app.handle_input(Input::TogglePreview);
        for _ in 0..app.scramble_len() {
            app.handle_input(Input::Left);
        }
        assert_eq!(app.preview_step, 0);
        assert!(app.preview_cube().is_solved());
        assert!(app.preview_highlight().is_empty());

        app.handle_input(Input::Right);
        assert_eq!(app.preview_step, 1);
        let expected = cube::Cube::solved().with_sequence(&app.scramble.moves[..1]);
        assert_eq!(app.preview_cube(), expected);
        assert!(!app.preview_highlight().is_empty());
    }

    #[test]
    fn preview_navigation_clamps() {
        let mut app = App::test_app(1, false);
        app.handle_input(Input::TogglePreview);
        let n = app.scramble_len();
        for _ in 0..(n + 5) {
            app.handle_input(Input::Right);
        }
        assert_eq!(app.preview_step, n);
        for _ in 0..(n + 5) {
            app.handle_input(Input::Left);
        }
        assert_eq!(app.preview_step, 0);
    }

    #[test]
    fn preview_toggles_closed() {
        let mut app = App::test_app(1, false);
        app.handle_input(Input::TogglePreview);
        assert_eq!(app.overlay, Overlay::Preview);
        app.handle_input(Input::TogglePreview);
        assert_eq!(app.overlay, Overlay::None);
    }

    #[test]
    fn cycle_theme_changes_palette_and_toasts() {
        let mut app = App::test_app(1, true);
        let first = app.theme_id.clone();
        app.handle_input(Input::ToggleTheme);
        assert_ne!(app.theme_id, first, "theme should change after cycling");
        assert_eq!(app.theme().id, app.theme_id);
        assert!(app.toast.as_ref().unwrap().text.contains(app.theme().name));
    }

    #[test]
    fn theme_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("tuibik-theme-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("db.sqlite");

        let chosen;
        {
            let store = Store::open(&path).unwrap();
            let mut app = App::with_store(store, false).unwrap();
            app.cycle_theme();
            chosen = app.theme_id.clone();
        }
        {
            let store = Store::open(&path).unwrap();
            let app = App::with_store(store, false).unwrap();
            assert_eq!(app.theme_id, chosen, "theme should persist across reopen");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ----- timing -----

    #[test]
    fn hold_press_arms_not_starts() {
        let mut app = App::test_app(1, true);
        assert_eq!(app.timer.phase, Phase::Idle);
        app.handle_input(Input::HoldPress);
        assert_eq!(app.timer.phase, Phase::Arming, "first press arms, not runs");
        assert_eq!(app.screen(), Screen::Focus);
    }

    #[test]
    fn autorepeat_keeps_arming_and_does_not_change_history() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::HoldPress); // arm
        for _ in 0..30 {
            app.handle_input(Input::HoldRepeat);
            assert_eq!(app.solves.len(), 0);
            assert!(matches!(app.timer.phase, Phase::Arming | Phase::Ready));
        }
    }

    #[test]
    fn press_while_running_stops_and_records() {
        let mut app = App::test_app(1, true);
        app.timer.press(0);
        app.timer.tick(ARM_THRESHOLD_MS + 1); // Ready
        app.timer.release(ARM_THRESHOLD_MS + 1);
        app.timer
            .tick(ARM_THRESHOLD_MS + 1 + RELEASE_DEBOUNCE_MS + 1); // Running
        assert_eq!(app.timer.phase, Phase::Running);
        app.timer.press(ARM_THRESHOLD_MS + 5000);
        assert!(app.timer.has_result());
        app.after_stop();
        assert_eq!(app.solves.len(), 1);
    }

    #[test]
    fn spurious_zero_solve_is_discarded() {
        let mut app = App::test_app(1, true);
        app.timer.press(0);
        app.timer.tick(ARM_THRESHOLD_MS + 1);
        app.timer.release(ARM_THRESHOLD_MS + 1);
        let run_base = ARM_THRESHOLD_MS + 1 + RELEASE_DEBOUNCE_MS + 1;
        app.timer.tick(run_base);
        assert_eq!(app.timer.phase, Phase::Running);
        app.timer.stop(run_base + 5); // ~5ms solve -> discarded
        app.after_stop();
        assert_eq!(app.solves.len(), 0, "near-zero solves must be discarded");
    }

    #[test]
    fn fallback_mode_no_longer_sticks_in_ready_and_records_a_solve() {
        // Regression: without key-release events the timer used to sit in
        // Ready forever (and swallow `q`).
        let mut app = App::test_app(1, false);
        app.handle_input_at(Input::HoldPress, 1_000);
        assert_eq!(app.timer.phase, Phase::Running);
        app.tick_at(6_000);
        assert_eq!(app.timer.phase, Phase::Running);
        app.handle_input_at(Input::HoldPress, 11_000);
        assert_eq!(app.solves.len(), 1);
        assert_eq!(app.solves[0].time_ms, 10_000);
        assert_eq!(app.timer.phase, Phase::Idle);
        // And the app can be quit again.
        app.handle_input(Input::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn fallback_startup_shows_toast() {
        let app = App::test_app(1, false);
        assert!(app.toast.as_ref().unwrap().text.contains("Tap-to-start"));
        let app = App::test_app(1, true);
        assert!(app.toast.is_none());
    }

    #[test]
    fn esc_cancels_arming_ready_and_inspection_without_recording() {
        let mut app = App::test_app(1, true);
        app.handle_input_at(Input::HoldPress, 0);
        assert_eq!(app.timer.phase, Phase::Arming);
        app.handle_input_at(Input::Cancel, 10);
        assert_eq!(app.timer.phase, Phase::Idle);

        app.handle_input_at(Input::HoldPress, 100);
        app.tick_at(100 + ARM_THRESHOLD_MS + 1);
        assert_eq!(app.timer.phase, Phase::Ready);
        app.handle_input_at(Input::Cancel, 500);
        assert_eq!(app.timer.phase, Phase::Idle);
        assert_eq!(app.screen(), Screen::Dashboard);
        assert!(!app.should_quit);
        assert_eq!(app.solves.len(), 0);

        app.settings.inspection = true;
        app.timer.set_config(app.timer_config());
        app.handle_input_at(Input::HoldPress, 1_000);
        assert_eq!(app.timer.phase, Phase::Inspecting);
        app.handle_input_at(Input::Cancel, 2_000);
        assert_eq!(app.timer.phase, Phase::Idle);
        assert_eq!(app.solves.len(), 0);
    }

    #[test]
    fn esc_stops_a_running_solve_like_any_key() {
        let mut app = App::test_app(1, false);
        app.handle_input_at(Input::HoldPress, 0);
        app.handle_input_at(Input::Cancel, 5_000);
        assert_eq!(app.solves.len(), 1);
        assert!(!app.should_quit);
    }

    #[test]
    fn shortcuts_are_suppressed_while_timing() {
        let mut app = App::test_app(1, true);
        let scramble = app.scramble.text.clone();
        app.handle_input_at(Input::HoldPress, 0);
        app.tick_at(ARM_THRESHOLD_MS + 1);
        assert_eq!(app.timer.phase, Phase::Ready);
        for input in [
            Input::NewScramble,
            Input::Delete,
            Input::OpenSessions,
            Input::ToggleTheme,
            Input::TogglePreview,
            Input::Settings,
            Input::Help,
            Input::Toggle3d,
            Input::Up,
            Input::Penalty(Penalty::Dnf),
            Input::Quit,
        ] {
            app.handle_input_at(input, ARM_THRESHOLD_MS + 5);
            assert_eq!(app.overlay, Overlay::None);
            assert_eq!(app.timer.phase, Phase::Ready);
        }
        assert_eq!(
            app.scramble.text, scramble,
            "n while ready keeps the scramble"
        );
        assert!(!app.should_quit);
    }

    #[test]
    fn ctrl_c_quits_from_any_state() {
        let mut app = App::test_app(1, true);
        app.handle_input_at(Input::HoldPress, 0);
        app.handle_input(Input::ForceQuit);
        assert!(app.should_quit);
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Help);
        app.handle_input(Input::ForceQuit);
        assert!(app.should_quit);
    }

    // ----- overlays -----

    fn open_each_overlay(app: &mut App, which: Overlay) {
        match which {
            Overlay::Sessions => app.handle_input(Input::OpenSessions),
            Overlay::Preview => app.handle_input(Input::TogglePreview),
            Overlay::Help => app.handle_input(Input::Help),
            Overlay::Settings => app.handle_input(Input::Settings),
            Overlay::Detail => app.handle_input(Input::Confirm),
            Overlay::ConfirmDelete => app.handle_input(Input::Delete),
            Overlay::Cube3D => app.handle_input(Input::Toggle3d),
            Overlay::None => {}
        }
        assert_eq!(app.overlay, which);
    }

    const ALL_OVERLAYS: [Overlay; 7] = [
        Overlay::Sessions,
        Overlay::Preview,
        Overlay::Help,
        Overlay::Settings,
        Overlay::Detail,
        Overlay::ConfirmDelete,
        Overlay::Cube3D,
    ];

    #[test]
    fn esc_closes_every_overlay_and_never_quits() {
        for which in ALL_OVERLAYS {
            let mut app = App::test_app(1, true);
            app.test_record_solve(3000);
            open_each_overlay(&mut app, which);
            app.handle_input(Input::Cancel);
            assert_eq!(app.overlay, Overlay::None, "{which:?}");
            assert!(!app.should_quit, "{which:?}");
        }
    }

    #[test]
    fn q_in_an_overlay_closes_it_without_quitting() {
        for which in ALL_OVERLAYS {
            let mut app = App::test_app(1, true);
            app.test_record_solve(3000);
            open_each_overlay(&mut app, which);
            app.handle_input(Input::Quit);
            assert!(!app.should_quit, "{which:?}");
            assert!(
                app.overlay != which || which == Overlay::None,
                "{which:?} should have closed"
            );
        }
    }

    #[test]
    fn esc_on_dashboard_does_not_quit() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Cancel);
        assert!(!app.should_quit);
    }

    #[test]
    fn help_and_settings_toggle_with_their_keys() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Help);
        assert_eq!(app.overlay, Overlay::Help);
        app.handle_input(Input::Help);
        assert_eq!(app.overlay, Overlay::None);
        app.handle_input(Input::Settings);
        assert_eq!(app.overlay, Overlay::Settings);
        app.handle_input(Input::Settings);
        assert_eq!(app.overlay, Overlay::None);
    }

    #[test]
    fn detail_needs_a_solve() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Confirm);
        assert_eq!(
            app.overlay,
            Overlay::None,
            "nothing to open in an empty session"
        );
    }

    // ----- 3D view -----

    #[test]
    fn cube_view_opens_and_closes_with_v_and_esc() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Toggle3d);
        assert_eq!(app.overlay, Overlay::Cube3D);
        app.handle_input(Input::Toggle3d);
        assert_eq!(app.overlay, Overlay::None);
        app.handle_input(Input::Toggle3d);
        app.handle_input(Input::Cancel);
        assert_eq!(app.overlay, Overlay::None);
        assert!(!app.should_quit);
    }

    #[test]
    fn cube_view_not_openable_while_timer_active() {
        let mut app = App::test_app(1, true);
        app.handle_input_at(Input::HoldPress, 0);
        assert_eq!(app.timer.phase, Phase::Arming);
        app.handle_input_at(Input::Toggle3d, 10);
        assert_eq!(app.overlay, Overlay::None);
        // Fallback mode: Space starts the run straight away.
        let mut app = App::test_app(1, false);
        app.handle_input_at(Input::HoldPress, 0);
        assert_eq!(app.timer.phase, Phase::Running);
        app.handle_input_at(Input::Toggle3d, 1_000);
        assert_ne!(app.overlay, Overlay::Cube3D);
    }

    /// Largest entry-wise difference between two matrices.
    fn mat_diff(a: &Mat3, b: &Mat3) -> f32 {
        (0..9)
            .map(|k| (a[k / 3][k % 3] - b[k / 3][k % 3]).abs())
            .fold(0.0, f32::max)
    }

    /// Run any pending turn animation to its end.
    fn settle(v: &mut View3d) {
        v.advance(0);
        v.advance(60_000);
    }

    #[test]
    fn view3d_axis_turns_are_exact() {
        for axis in [Axis::X, Axis::Y, Axis::Z] {
            let mut v = View3d::default();
            for _ in 0..4 {
                v.turn_axis(axis, false);
                settle(&mut v);
            }
            assert_eq!(v, View3d::default(), "{axis:?} four times");
            v.turn_axis(axis, false);
            settle(&mut v);
            assert_ne!(v, View3d::default());
            v.turn_axis(axis, true);
            settle(&mut v);
            assert_eq!(v, View3d::default(), "{axis:?} then reverse");
            // Queued without animating in between, too.
            for _ in 0..4 {
                v.turn_axis(axis, true);
            }
            assert_eq!(v.target, View3d::default().target);
        }
    }

    #[test]
    fn view3d_axis_turn_follows_the_cube_after_free_rotation() {
        let mut v = View3d::default();
        v.rotate_free(40.0, -70.0);
        let before = v.current;
        v.turn_axis(Axis::Y, false);
        settle(&mut v);
        // The cube's own U–D axis (second column) stays put on screen...
        let col = |m: &Mat3, j: usize| [m[0][j], m[1][j], m[2][j]];
        let (u0, u1) = (col(&before, 1), col(&v.current, 1));
        assert!(
            (0..3).all(|i| (u0[i] - u1[i]).abs() < 1e-5),
            "{u0:?} {u1:?}"
        );
        // ...while F moves to where L was, like the notation's y.
        let (r0, f1) = (col(&before, 0), col(&v.current, 2));
        assert!(
            (0..3).all(|i| (r0[i] + f1[i]).abs() < 1e-5),
            "{r0:?} {f1:?}"
        );
        // Not the screen's vertical axis: that turn would give another result.
        let screen_y = cube3d::mat_mul(&cube3d::quarter_turn(Axis::Y, false), &before);
        assert!(mat_diff(&v.current, &screen_y) > 0.1);
    }

    #[test]
    fn view3d_turn_animates_with_easing_and_lands_exactly() {
        let mut v = View3d::default();
        let start = v.current;
        v.turn_axis(Axis::X, false);
        assert!(v.is_animating());
        assert_eq!(v.current, start, "no jump on press");
        v.advance(1_000);
        assert_eq!(v.current, start);
        let mut frames = vec![start];
        for t in (1_020..1_200).step_by(20) {
            v.advance(t);
            assert!(v.is_animating(), "{t}");
            frames.push(v.current);
        }
        let steps: Vec<f32> = frames.windows(2).map(|w| mat_diff(&w[0], &w[1])).collect();
        assert!(
            steps.iter().all(|d| *d > 1e-4),
            "every frame differs: {steps:?}"
        );
        // Eased: slow at both ends, faster in the middle.
        let mid = steps[steps.len() / 2];
        assert!(steps[0] < mid && *steps.last().unwrap() < mid, "{steps:?}");
        v.advance(1_000 + TURN_MS as u64);
        assert!(!v.is_animating());
        assert_eq!(v.current, v.target);
        assert_eq!(
            v.target,
            cube3d::mat_mul(&start, &cube3d::quarter_turn(Axis::X, false))
        );
    }

    #[test]
    fn view3d_chained_turns_end_at_180_without_snapping() {
        let mut v = View3d::default();
        let start = v.current;
        v.turn_axis(Axis::Z, false);
        v.advance(0);
        let mut prev = v.current;
        let mut t = 0;
        let mut max_step: f32 = 0.0;
        while v.is_animating() {
            t += 16;
            if t == 96 {
                // Second press mid-turn: the cube stays where it is drawn.
                v.turn_axis(Axis::Z, false);
                assert_eq!(v.current, prev);
            }
            v.advance(t);
            max_step = max_step.max(mat_diff(&v.current, &prev));
            prev = v.current;
            assert!(t < 2_000);
        }
        assert!(max_step < 0.3, "no snap between turns: {max_step}");
        let half = cube3d::mat_mul(&start, &cube3d::rotation(Axis::Z, 180.0));
        assert!(mat_diff(&v.current, &half) < 1e-6);
        // Two presses in one batch (exactly 180° apart) also animate there.
        let mut v = View3d::default();
        v.turn_axis(Axis::Z, false);
        v.turn_axis(Axis::Z, false);
        v.advance(0);
        v.advance(100);
        assert!(mat_diff(&v.current, &start) > 1e-3, "moving");
        settle(&mut v);
        assert!(mat_diff(&v.current, &half) < 1e-6);
    }

    #[test]
    fn view3d_stays_orthonormal_after_many_free_rotations() {
        let mut v = View3d::default();
        for i in 0..1000 {
            v.rotate_free(VIEW_STEP_DEG * 0.37, if i % 3 == 0 { -7.1 } else { 4.3 });
        }
        let m = v.current;
        let id = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert!(mat_diff(&cube3d::mat_mul(&m, &cube3d::transpose(&m)), &id) < 1e-5);
        v.reset();
        assert_eq!(v, View3d::default());
    }

    #[test]
    fn cube_view_fixed_axis_keys_animate_90_degree_turns() {
        let mut app = App::test_app(1, true);
        app.handle_input_at(Input::Toggle3d, 0);
        let start = app.view3d;
        assert!(!app.is_animating());
        app.handle_input_at(Input::ViewAxis(Axis::X, false), 100);
        assert!(app.is_animating() && app.needs_fast_ticks());
        app.tick_at(100);
        app.tick_at(200);
        assert_ne!(app.view3d.current, start.current, "part way");
        assert_ne!(app.view3d.current, app.view3d.target);
        app.tick_at(400);
        assert!(!app.is_animating());
        let expected = cube3d::mat_mul(&start.current, &cube3d::quarter_turn(Axis::X, false));
        assert_eq!(app.view3d.current, expected);
        app.handle_input_at(Input::ViewAxis(Axis::X, true), 500);
        app.tick_at(500);
        app.tick_at(800);
        assert_eq!(app.view3d, start);
        // `y` arrives as Yes and turns about the cube's U–D axis.
        for _ in 0..4 {
            app.handle_input_at(Input::Yes, 900);
        }
        app.tick_at(900);
        assert!(!app.is_animating(), "four turns: already home");
        assert_eq!(app.view3d, start);
        app.handle_input_at(Input::Yes, 1_000);
        app.handle_input_at(Input::ViewAxis(Axis::Y, true), 1_000);
        app.tick_at(1_000);
        assert_eq!(app.view3d, start);
        for _ in 0..4 {
            app.handle_input_at(Input::ViewAxis(Axis::Z, false), 1_100);
        }
        assert_eq!(app.view3d.target, start.target);
        assert_eq!(app.overlay, Overlay::Cube3D, "y does not close the view");
        // Closing mid-turn lands on the target.
        app.handle_input_at(Input::ViewAxis(Axis::Z, false), 1_200);
        app.handle_input_at(Input::Cancel, 1_210);
        assert_eq!(app.view3d.current, app.view3d.target);
        assert!(!app.needs_fast_ticks());
    }

    #[test]
    fn cube_view_free_rotation_passes_through_the_poles() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Toggle3d);
        let start = app.view3d.current;
        app.handle_input(Input::Left);
        assert_ne!(app.view3d.current, start);
        assert!(!app.is_animating(), "free steps are immediate");
        app.handle_input(Input::Right);
        assert!(mat_diff(&app.view3d.current, &start) < 1e-5);
        // A full circle about the horizontal axis: every step moves the cube.
        let steps = (360.0 / VIEW_STEP_DEG) as usize;
        let mut prev = app.view3d.current;
        for _ in 0..steps {
            app.handle_input(Input::Down);
            assert!(mat_diff(&app.view3d.current, &prev) > 0.1);
            prev = app.view3d.current;
        }
        assert!(mat_diff(&app.view3d.current, &start) < 1e-4);
        // The dashboard selection is untouched while the view is open.
        assert_eq!(app.history_selected, 0);
    }

    #[test]
    fn cube_view_reset_restores_default_orientation() {
        let mut app = App::test_app(1, true);
        app.handle_input_at(Input::Toggle3d, 0);
        app.handle_input_at(Input::Left, 0);
        app.handle_input_at(Input::Up, 0);
        app.handle_input_at(Input::ViewAxis(Axis::Z, true), 0);
        app.tick_at(0);
        app.tick_at(50);
        app.handle_input_at(Input::Yes, 60);
        // Reset mid-animation.
        app.handle_input_at(Input::ResetView, 70);
        assert_eq!(app.view3d, View3d::default());
        // Home resets too.
        app.handle_input_at(Input::Down, 100);
        app.handle_input_at(Input::ViewAxis(Axis::X, false), 100);
        app.tick_at(1_000);
        app.handle_input_at(Input::JumpTop, 1_100);
        assert_eq!(app.view3d, View3d::default());
    }

    #[test]
    fn cube_view_auto_spin_advances_with_time_and_stops() {
        let mut app = App::test_app(1, true);
        app.handle_input_at(Input::Toggle3d, 0);
        assert!(!app.needs_fast_ticks());
        app.handle_input_at(Input::ToggleSpin, 1_000);
        assert!(app.needs_fast_ticks());
        let m0 = app.view3d.current;
        app.tick_at(1_500);
        let m1 = app.view3d.current;
        // Half a second of spin about the vertical screen axis.
        let expected = cube3d::mat_mul(&cube3d::rotation(Axis::Y, SPIN_DEG_PER_S / 2.0), &m0);
        assert!(mat_diff(&m1, &expected) < 1e-5, "{m0:?} -> {m1:?}");
        app.handle_input_at(Input::ToggleSpin, 1_600);
        app.tick_at(3_000);
        assert_eq!(app.view3d.current, m1, "stopped");
        // Closing stops the spin too.
        app.handle_input_at(Input::ToggleSpin, 3_000);
        app.handle_input_at(Input::Cancel, 3_100);
        assert!(!app.view3d.spin);
        assert!(!app.needs_fast_ticks());
    }

    #[test]
    fn fast_ticks_only_while_timing_or_spinning() {
        let mut app = App::test_app(1, true);
        assert!(!app.needs_fast_ticks());
        app.handle_input_at(Input::HoldPress, 0);
        assert!(app.needs_fast_ticks());
    }

    // ----- post-solve feedback -----

    #[test]
    fn delta_versus_previous_solve() {
        let mut app = App::test_app(1, true);
        app.test_record_solve(13_000);
        app.test_record_solve(12_480);
        let r = app.last_result.clone().unwrap();
        assert_eq!(r.delta_prev, Some(-520));
        assert_eq!(r.ms, Some(12_480));
    }

    #[test]
    fn first_solve_has_no_delta_and_no_pb() {
        let mut app = App::test_app(1, true);
        app.test_record_solve(13_000);
        let r = app.last_result.clone().unwrap();
        assert_eq!(r.delta_prev, None);
        assert!(r.pbs.is_empty(), "first value only establishes the record");
    }

    #[test]
    fn new_best_single_is_a_pb() {
        let mut app = App::test_app(1, true);
        app.test_record_solve(11_200);
        app.test_record_solve(10_950);
        let r = app.last_result.clone().unwrap();
        assert_eq!(r.pbs, vec![PbKind::Single]);
        assert!(app.toast.as_ref().unwrap().text.contains("PB single"));
        app.test_record_solve(12_000);
        assert!(app.last_result.as_ref().unwrap().pbs.is_empty());
    }

    #[test]
    fn first_ao5_is_not_a_pb_but_a_better_one_is() {
        let mut app = App::test_app(1, true);
        for t in [12_000, 12_100, 12_200, 12_300] {
            app.test_record_solve(t);
        }
        app.test_record_solve(12_400); // 5th solve -> first ao5
        assert!(!app.last_result.as_ref().unwrap().pbs.contains(&PbKind::Ao5));
        app.test_record_solve(9_000);
        app.test_record_solve(9_000);
        let pbs = app.last_result.clone().unwrap().pbs;
        assert!(pbs.contains(&PbKind::Ao5), "{pbs:?}");
    }

    #[test]
    fn last_result_clears_when_next_timing_starts() {
        let mut app = App::test_app(1, false);
        app.test_record_solve(3_000);
        assert!(app.last_result.is_some());
        app.handle_input_at(Input::HoldPress, 0); // fallback: starts running
        assert!(app.last_result.is_none());
    }

    #[test]
    fn inspection_timeout_records_a_dnf_and_new_scramble() {
        let mut app = App::test_app(1, true);
        app.settings.inspection = true;
        app.timer.set_config(app.timer_config());
        let before = app.scramble.text.clone();
        app.handle_input_at(Input::HoldPress, 0);
        app.tick_at(17_000);
        assert_eq!(app.solves.len(), 1);
        assert_eq!(app.solves[0].penalty, Penalty::Dnf);
        assert_eq!(app.solves[0].time_ms, 0);
        assert_eq!(app.solves[0].scramble, before);
        assert_ne!(app.scramble.text, before);
        assert_eq!(app.timer.phase, Phase::Idle);
    }

    #[test]
    fn late_inspection_start_records_plus_two() {
        let mut app = App::test_app(1, false); // fallback + inspection
        app.settings.inspection = true;
        app.timer.set_config(app.timer_config());
        app.handle_input_at(Input::HoldPress, 0);
        app.tick_at(16_000);
        app.handle_input_at(Input::HoldPress, 16_000);
        app.handle_input_at(Input::HoldPress, 21_000);
        assert_eq!(app.solves[0].penalty, Penalty::PlusTwo);
        assert_eq!(app.solves[0].time_ms, 5_000);
    }

    // ----- toasts -----

    #[test]
    fn toast_expires_after_its_deadline() {
        let mut app = App::test_app(1, true);
        app.notify("hello", ToastKind::Info);
        let now = app.now();
        app.tick_at(now + TOAST_MS - 100);
        assert!(app.toast.is_some(), "still visible before the deadline");
        app.tick_at(now + TOAST_MS + 100);
        assert!(app.toast.is_none());
    }

    #[test]
    fn newer_toast_replaces_older() {
        let mut app = App::test_app(1, true);
        app.notify("one", ToastKind::Info);
        app.notify("two", ToastKind::Warn);
        assert_eq!(app.toast.as_ref().unwrap().text, "two");
    }

    #[test]
    fn toasts_for_delete_penalty_and_session_switch() {
        let mut app = App::test_app(1, true);
        app.test_record_solve(3_000);
        app.handle_input(Input::Penalty(Penalty::Dnf));
        assert!(app.toast.as_ref().unwrap().text.contains("DNF"));
        app.handle_input(Input::Delete);
        app.handle_input(Input::Yes);
        assert_eq!(app.toast.as_ref().unwrap().text, "Solve deleted");
        app.handle_input(Input::OpenSessions);
        app.handle_input(Input::NewScramble);
        app.handle_input(Input::Char('X'));
        app.handle_input(Input::Confirm);
        assert!(app.toast.as_ref().unwrap().text.contains('X'));
    }

    // ----- penalties & delete -----

    #[test]
    fn two_right_after_a_solve_marks_it_plus_two() {
        let mut app = App::test_app(1, true);
        app.test_record_solve(12_340);
        app.handle_input(Input::Penalty(Penalty::PlusTwo));
        assert_eq!(app.solves[0].penalty, Penalty::PlusTwo);
        assert_eq!(app.solves[0].time_ms, 12_340, "raw time unchanged");
        assert!(app.toast.is_some());
        // The post-solve feedback follows the edit.
        assert_eq!(app.last_result.as_ref().unwrap().ms, Some(14_340));
        // Summary updated immediately.
        assert_eq!(app.summary().best, Some(14_340));
    }

    #[test]
    fn penalty_can_be_toggled_back_to_ok() {
        let mut app = App::test_app(1, true);
        app.test_record_solve_with(9_000, Penalty::Dnf);
        assert_eq!(app.summary().best, None);
        app.handle_input(Input::Penalty(Penalty::Ok));
        assert_eq!(app.solves[0].penalty, Penalty::Ok);
        assert_eq!(app.summary().best, Some(9_000));
    }

    #[test]
    fn penalty_applies_to_the_selected_solve() {
        let mut app = App::test_app(1, true);
        app.test_record_solve(1_000);
        app.test_record_solve(2_000);
        app.test_record_solve(3_000);
        app.history_selected = 2; // oldest
        app.handle_input(Input::Penalty(Penalty::Dnf));
        assert_eq!(app.solves[0].penalty, Penalty::Dnf);
        assert_eq!(app.solves[2].penalty, Penalty::Ok);
    }

    #[test]
    fn penalty_persists_across_reopen() {
        let dir = std::env::temp_dir().join(format!("tuibik-pen-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("db.sqlite");
        {
            let mut app = App::with_store(Store::open(&path).unwrap(), true).unwrap();
            app.test_record_solve(5_000);
            app.handle_input(Input::Penalty(Penalty::Dnf));
        }
        let app = App::with_store(Store::open(&path).unwrap(), true).unwrap();
        assert_eq!(app.solves[0].penalty, Penalty::Dnf);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancel_delete_keeps_the_solve() {
        let mut app = App::test_app(1, true);
        app.test_record_solve(2_500);
        app.handle_input(Input::Delete);
        assert_eq!(app.overlay, Overlay::ConfirmDelete);
        app.handle_input(Input::Cancel);
        assert_eq!(app.overlay, Overlay::None);
        assert_eq!(app.solves.len(), 1);
        app.handle_input(Input::Delete);
        app.handle_input(Input::NewScramble); // 'n' = no
        assert_eq!(app.solves.len(), 1);
    }

    #[test]
    fn confirm_delete_keeps_list_position() {
        let mut app = App::test_app(1, true);
        for i in 1..=10 {
            app.test_record_solve(i * 1_000);
        }
        // Solve #5 of 10 is at history index 5 (10 - 5).
        app.history_selected = 5;
        assert_eq!(app.selected_number(), 5);
        app.handle_input(Input::Delete);
        app.handle_input(Input::Yes);
        assert_eq!(app.solves.len(), 9);
        assert!(!app.solves.iter().any(|s| s.time_ms == 5_000));
        assert_eq!(
            app.history_selected, 5,
            "selection stays at the same position"
        );
        assert_eq!(app.overlay, Overlay::None);
    }

    #[test]
    fn confirm_delete_with_d_and_clamps_at_the_end() {
        let mut app = App::test_app(1, true);
        app.test_record_solve(1_000);
        app.test_record_solve(2_000);
        app.history_selected = 1; // oldest
        app.handle_input(Input::Delete);
        app.handle_input(Input::Delete); // `d` again confirms
        assert_eq!(app.solves.len(), 1);
        assert_eq!(app.history_selected, 0, "clamped into the shorter list");
    }

    #[test]
    fn delete_from_detail_and_cancel_returns_to_detail() {
        let mut app = App::test_app(1, true);
        app.test_record_solve(2_500);
        app.handle_input(Input::Confirm);
        assert_eq!(app.overlay, Overlay::Detail);
        app.handle_input(Input::Delete);
        assert_eq!(app.overlay, Overlay::ConfirmDelete);
        app.handle_input(Input::Cancel);
        assert_eq!(app.overlay, Overlay::Detail);
        app.handle_input(Input::Delete);
        app.handle_input(Input::Yes);
        assert_eq!(app.overlay, Overlay::None);
        assert!(app.solves.is_empty());
    }

    #[test]
    fn delete_on_empty_history_does_nothing() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Delete);
        assert_eq!(app.overlay, Overlay::None);
    }

    // ----- settings -----

    #[test]
    fn toggling_inspection_updates_timer_config_and_persists() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Settings);
        assert_eq!(app.settings_selected, 0);
        app.handle_input(Input::Confirm);
        assert!(app.settings.inspection);
        assert!(app.timer.config().inspection);
        assert_eq!(
            app.store.get_config("inspection").unwrap().as_deref(),
            Some("true")
        );
        app.handle_input(Input::Confirm);
        assert!(!app.settings.inspection);
        assert!(!app.timer.config().inspection);
    }

    #[test]
    fn toggling_show_running_time() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Settings);
        app.handle_input(Input::Down);
        app.handle_input(Input::HoldPress);
        assert!(!app.settings.show_running_time);
    }

    #[test]
    fn theme_changes_immediately_from_settings() {
        let mut app = App::test_app(1, true);
        let first = app.theme_id.clone();
        app.handle_input(Input::Settings);
        app.handle_input(Input::Down);
        app.handle_input(Input::Down);
        app.handle_input(Input::Right);
        assert_ne!(app.theme_id, first);
        assert_eq!(app.overlay, Overlay::Settings, "overlay stays open");
        app.handle_input(Input::Left);
        assert_eq!(app.theme_id, first);
    }

    #[test]
    fn settings_navigation_clamps() {
        let mut app = App::test_app(1, true);
        app.handle_input(Input::Settings);
        app.handle_input(Input::Up);
        assert_eq!(app.settings_selected, 0);
        for _ in 0..10 {
            app.handle_input(Input::Down);
        }
        assert_eq!(app.settings_selected, ROWS.len() - 1);
    }

    #[test]
    fn settings_persist_across_reopen() {
        let dir = std::env::temp_dir().join(format!("tuibik-set-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("db.sqlite");
        {
            let mut app = App::with_store(Store::open(&path).unwrap(), true).unwrap();
            app.handle_input(Input::Settings);
            app.handle_input(Input::Confirm); // inspection on
            app.handle_input(Input::Down);
            app.handle_input(Input::Confirm); // running time off
        }
        let app = App::with_store(Store::open(&path).unwrap(), true).unwrap();
        assert!(app.settings.inspection);
        assert!(!app.settings.show_running_time);
        assert!(app.timer.config().inspection);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dashboard_summary_is_cached_and_tracks_changes() {
        let mut app = App::test_app(1, true);
        assert_eq!(app.summary().count, 0);
        app.test_record_solve(1_000);
        assert_eq!(app.summary().count, 1);
        app.handle_input(Input::Delete);
        app.handle_input(Input::Yes);
        assert_eq!(app.summary().count, 0);
    }
}
