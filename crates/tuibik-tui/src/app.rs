//! Application state and the input reducer.

use std::cell::RefCell;

use cube::Cube;
use ratatui::widgets::ListState;
use scramble::{Scramble, Scrambler};
use stats::{StatValue, Summary};
use store::{Penalty, Solve, Store};

use crate::event::{Input, KeyContext, KeyMode};
use crate::sessions::{SessionMenu, SessionMode};
use crate::settings::{Row, Settings, ROWS};
use crate::timer::{Clock, Phase, Timer, TimerConfig};

/// How long a toast stays visible.
pub const TOAST_MS: u64 = 3000;
/// Solves shorter than this (that were not inspection timeouts) are treated as
/// key-autorepeat glitches and discarded.
const MIN_SOLVE_MS: i64 = 200;

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
            _ => {}
        }
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
            Overlay::None => {}
        }
        assert_eq!(app.overlay, which);
    }

    const ALL_OVERLAYS: [Overlay; 6] = [
        Overlay::Sessions,
        Overlay::Preview,
        Overlay::Help,
        Overlay::Settings,
        Overlay::Detail,
        Overlay::ConfirmDelete,
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
