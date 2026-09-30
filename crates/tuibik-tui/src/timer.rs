//! csTimer-style hold-to-arm timer, robust against OS key autorepeat.
//!
//! On many terminals, holding a key produces a *burst* of `Press`/`Release`
//! event pairs (autorepeat) rather than one sustained press. Worse, the first
//! autorepeat only arrives after an initial delay (often 300–500 ms), so a
//! naive "no press for N ms = released" heuristic misfires on the very first
//! hold.
//!
//! To be robust we use the terminal's real `Release` events but **debounce**
//! them: a release is only "real" if it is *not* immediately followed by a
//! press (autorepeat). A press cancels any pending release. This makes the
//! logic immune to both the initial autorepeat delay and the repeat bursts.
//!
//! Phases (enhanced mode, no inspection):
//! ```text
//! Idle    --press--> Arming            (key down)
//! Arming  --held >= ARM_THRESHOLD_MS--> Ready   (green)
//! Arming  --real release--> Idle       (cancel)
//! Ready   --real release--> Running    (clock starts)
//! Running --press/any key--> Stopped   (record time)
//! Stopped --consumed by app--> Idle
//! ```
//!
//! With WCA inspection enabled the first press goes to `Inspecting` instead,
//! and the arming cycle is nested inside it (a real release while arming
//! returns to `Inspecting`). In *fallback* mode (terminal without key-release
//! events) there is no arming: a press starts the solve (or inspection) and a
//! press while running stops it, and releases are ignored, so the timer can
//! never get stuck in `Ready`.
//!
//! The inspection penalty is computed when the solve starts: up to 15 s of
//! inspection is free, up to 17 s is `+2`, and reaching 17 s without starting
//! stops the timer with a DNF (raw time 0).

use std::time::{Duration, Instant};

use store::Penalty;

/// How long the key must be held before the timer is armed ("ready"/green).
pub const ARM_THRESHOLD_MS: u64 = 300;

/// A release is committed as "real" only if no press cancels it within this
/// window. Autorepeat release→press gaps are far smaller than this.
pub const RELEASE_DEBOUNCE_MS: u64 = 60;

/// Length of the free inspection period.
pub const INSPECTION_MS: u64 = 15_000;
/// Inspection time after which the attempt is a DNF.
pub const INSPECTION_DNF_MS: u64 = 17_000;
/// Inspection cue times.
pub const INSPECTION_CUE_1_MS: u64 = 8_000;
pub const INSPECTION_CUE_2_MS: u64 = 12_000;

/// The phase of the timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Arming,
    Ready,
    Inspecting,
    Running,
    Stopped,
}

/// Timer behaviour switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimerConfig {
    /// WCA inspection before the solve.
    pub inspection: bool,
    /// Tap-to-start mode for terminals without key-release events.
    pub fallback: bool,
}

/// The timer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timer {
    pub phase: Phase,
    config: TimerConfig,
    /// When arming started (ms).
    arm_started_ms: u64,
    /// A pending release timestamp (ms), if a release was seen and not yet
    /// cancelled by a press or committed by a tick.
    pending_release_ms: Option<u64>,
    /// When inspection started (ms); retained while arming/ready inside it.
    inspection_started_ms: Option<u64>,
    /// Inspection time elapsed as of the last tick.
    inspection_live_ms: u64,
    /// When the run started (ms).
    run_started_ms: u64,
    /// The final elapsed time in ms once Stopped.
    final_ms: u64,
    /// The live elapsed time while Running.
    live_ms: u64,
    /// Penalty carried by the run / result (from inspection overtime).
    penalty: Penalty,
}

impl Default for Timer {
    fn default() -> Self {
        Timer::with_config(TimerConfig::default())
    }
}

impl Timer {
    #[cfg(test)]
    pub fn new() -> Self {
        Timer::default()
    }

    pub fn with_config(config: TimerConfig) -> Self {
        Timer {
            phase: Phase::Idle,
            config,
            arm_started_ms: 0,
            pending_release_ms: None,
            inspection_started_ms: None,
            inspection_live_ms: 0,
            run_started_ms: 0,
            final_ms: 0,
            live_ms: 0,
            penalty: Penalty::Ok,
        }
    }

    #[cfg(test)]
    pub fn config(&self) -> TimerConfig {
        self.config
    }

    /// Update the configuration (takes effect on the next start).
    pub fn set_config(&mut self, config: TimerConfig) {
        self.config = config;
    }

    /// True while the timer owns the keyboard: arming, ready, inspecting or
    /// running.
    pub fn is_active(&self) -> bool {
        matches!(
            self.phase,
            Phase::Arming | Phase::Ready | Phase::Inspecting | Phase::Running
        )
    }

    pub fn display_ms(&self) -> u64 {
        match self.phase {
            Phase::Running => self.live_ms,
            _ => self.final_ms,
        }
    }

    pub fn has_result(&self) -> bool {
        self.phase == Phase::Stopped
    }

    pub fn result_ms(&self) -> u64 {
        self.final_ms
    }

    /// The penalty attached to the current run / result.
    pub fn result_penalty(&self) -> Penalty {
        self.penalty
    }

    pub fn consume_result(&mut self) {
        if self.phase == Phase::Stopped {
            self.phase = Phase::Idle;
        }
    }

    /// Whether this timer is inside an inspection (inspecting, or arming/ready
    /// nested in it).
    pub fn in_inspection(&self) -> bool {
        self.inspection_started_ms.is_some()
            && matches!(self.phase, Phase::Inspecting | Phase::Arming | Phase::Ready)
    }

    /// Inspection time elapsed as of the last tick.
    #[cfg(test)]
    pub fn inspection_ms(&self) -> u64 {
        self.inspection_live_ms
    }

    /// Whole seconds left in inspection (15 → 0), or `None` once in overtime.
    pub fn inspection_seconds_left(&self) -> Option<u64> {
        let e = self.inspection_live_ms;
        if e > INSPECTION_MS {
            None
        } else {
            Some((INSPECTION_MS - e).div_ceil(1000))
        }
    }

    /// The 8 s cue is active (8 s or more elapsed).
    pub fn cue_8s(&self) -> bool {
        self.in_inspection() && self.inspection_live_ms >= INSPECTION_CUE_1_MS
    }

    /// The 12 s cue is active (12 s or more elapsed).
    pub fn cue_12s(&self) -> bool {
        self.in_inspection() && self.inspection_live_ms >= INSPECTION_CUE_2_MS
    }

    /// Inspection is past 15 s (a `+2` is pending).
    pub fn inspection_overtime(&self) -> bool {
        self.in_inspection() && self.inspection_live_ms > INSPECTION_MS
    }

    /// Cancel arming / ready / inspection, returning to idle without a result.
    pub fn cancel(&mut self) {
        if matches!(self.phase, Phase::Arming | Phase::Ready | Phase::Inspecting) {
            self.phase = Phase::Idle;
            self.pending_release_ms = None;
            self.inspection_started_ms = None;
            self.inspection_live_ms = 0;
            self.penalty = Penalty::Ok;
        }
    }

    fn begin_solve_or_inspection(&mut self, now_ms: u64) {
        self.penalty = Penalty::Ok;
        self.final_ms = 0;
        if self.config.inspection {
            self.phase = Phase::Inspecting;
            self.inspection_started_ms = Some(now_ms);
            self.inspection_live_ms = 0;
        } else if self.config.fallback {
            self.start_run(now_ms);
        } else {
            self.phase = Phase::Arming;
            self.arm_started_ms = now_ms;
        }
    }

    /// Start the clock at `at_ms`, applying any inspection penalty.
    fn start_run(&mut self, at_ms: u64) {
        if let Some(insp) = self.inspection_started_ms.take() {
            let elapsed = at_ms.saturating_sub(insp);
            self.penalty = if elapsed > INSPECTION_MS {
                Penalty::PlusTwo
            } else {
                Penalty::Ok
            };
        }
        self.inspection_live_ms = 0;
        self.phase = Phase::Running;
        self.run_started_ms = at_ms;
        self.live_ms = 0;
    }

    /// A key-down for the hold key at `now_ms`. Autorepeat presses call this
    /// repeatedly while held; each one cancels a pending release.
    pub fn press(&mut self, now_ms: u64) {
        // Any press cancels a pending release (it was autorepeat, not a real up).
        self.pending_release_ms = None;
        match self.phase {
            Phase::Idle | Phase::Stopped => self.begin_solve_or_inspection(now_ms),
            Phase::Inspecting => {
                if self.config.fallback {
                    self.start_run(now_ms);
                } else {
                    self.phase = Phase::Arming;
                    self.arm_started_ms = now_ms;
                }
            }
            Phase::Arming => {
                if now_ms.saturating_sub(self.arm_started_ms) >= ARM_THRESHOLD_MS {
                    self.phase = Phase::Ready;
                }
            }
            Phase::Ready => {}
            Phase::Running => {
                // A genuine press while running stops the solve.
                self.stop(now_ms);
            }
        }
    }

    /// A key-up for the hold key at `now_ms`. Recorded as *pending*; it is only
    /// acted upon by `tick` if no press cancels it within `RELEASE_DEBOUNCE_MS`.
    /// Ignored in fallback mode.
    pub fn release(&mut self, now_ms: u64) {
        if self.config.fallback {
            return;
        }
        if matches!(self.phase, Phase::Arming | Phase::Ready) {
            self.pending_release_ms = Some(now_ms);
        }
    }

    /// Advance time: promote Arming->Ready, commit a debounced real release
    /// (Ready->Running or Arming->Idle/Inspecting), expire inspection, and
    /// update the live clocks.
    pub fn tick(&mut self, now_ms: u64) {
        match self.phase {
            Phase::Arming => {
                if now_ms.saturating_sub(self.arm_started_ms) >= ARM_THRESHOLD_MS {
                    self.phase = Phase::Ready;
                }
                if let Some(rel) = self.pending_release_ms {
                    if now_ms.saturating_sub(rel) >= RELEASE_DEBOUNCE_MS {
                        // Real release before ready -> cancel (back to
                        // inspection if we are inside one).
                        self.pending_release_ms = None;
                        self.phase = if self.inspection_started_ms.is_some() {
                            Phase::Inspecting
                        } else {
                            Phase::Idle
                        };
                    }
                }
            }
            Phase::Ready => {
                if let Some(rel) = self.pending_release_ms {
                    if now_ms.saturating_sub(rel) >= RELEASE_DEBOUNCE_MS {
                        // Real release while ready -> start the run at the moment
                        // of release for accurate timing.
                        self.pending_release_ms = None;
                        let within_limit = self
                            .inspection_started_ms
                            .is_none_or(|i| rel.saturating_sub(i) <= INSPECTION_DNF_MS);
                        if within_limit {
                            self.start_run(rel);
                            self.live_ms = now_ms.saturating_sub(self.run_started_ms);
                        }
                    }
                }
            }
            Phase::Running => {
                self.live_ms = now_ms.saturating_sub(self.run_started_ms);
            }
            _ => {}
        }

        // Inspection clock and timeout (applies while inspecting or nested).
        if self.in_inspection() {
            let insp = self.inspection_started_ms.unwrap_or(now_ms);
            let elapsed = now_ms.saturating_sub(insp);
            self.inspection_live_ms = elapsed;
            if elapsed >= INSPECTION_DNF_MS {
                self.inspection_started_ms = None;
                self.pending_release_ms = None;
                self.inspection_live_ms = 0;
                self.penalty = Penalty::Dnf;
                self.final_ms = 0;
                self.phase = Phase::Stopped;
            }
        }
    }

    /// Stop the run at `now_ms`.
    pub fn stop(&mut self, now_ms: u64) {
        if self.phase == Phase::Running {
            self.final_ms = now_ms.saturating_sub(self.run_started_ms);
            self.phase = Phase::Stopped;
        }
    }
}

/// Format a time in ms as `M:SS.cc` or `S.cc` (csTimer style, centiseconds).
pub fn format_ms(ms: u64) -> String {
    let total_cs = ms / 10;
    let cs = total_cs % 100;
    let total_s = total_cs / 100;
    let s = total_s % 60;
    let m = total_s / 60;
    if m > 0 {
        format!("{}:{:02}.{:02}", m, s, cs)
    } else {
        format!("{}.{:02}", s, cs)
    }
}

pub fn now_ms(base: Instant) -> u64 {
    base.elapsed().as_millis() as u64
}

pub struct Clock {
    base: Instant,
}

impl Clock {
    pub fn new() -> Self {
        Clock {
            base: Instant::now(),
        }
    }
    pub fn now_ms(&self) -> u64 {
        now_ms(self.base)
    }
    #[allow(dead_code)]
    pub fn elapsed(&self) -> Duration {
        self.base.elapsed()
    }
}

impl Default for Clock {
    fn default() -> Self {
        Clock::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENH_INSP: TimerConfig = TimerConfig {
        inspection: true,
        fallback: false,
    };
    const FB: TimerConfig = TimerConfig {
        inspection: false,
        fallback: true,
    };
    const FB_INSP: TimerConfig = TimerConfig {
        inspection: true,
        fallback: true,
    };

    /// Drive an enhanced hold-and-release starting at `t`; returns the release time.
    fn hold_and_release(tm: &mut Timer, t: u64) -> u64 {
        tm.press(t);
        tm.tick(t + ARM_THRESHOLD_MS + 10);
        assert_eq!(tm.phase, Phase::Ready);
        let rel = t + ARM_THRESHOLD_MS + 20;
        tm.release(rel);
        tm.tick(rel + RELEASE_DEBOUNCE_MS + 1);
        rel
    }

    #[test]
    fn hold_arm_release_run_flow() {
        let mut t = Timer::new();
        t.press(0);
        assert_eq!(t.phase, Phase::Arming);
        // Held past threshold via a tick.
        t.tick(ARM_THRESHOLD_MS + 10);
        assert_eq!(t.phase, Phase::Ready);
        // Real release: pending, then committed by a tick after debounce.
        t.release(ARM_THRESHOLD_MS + 20);
        t.tick(ARM_THRESHOLD_MS + 20 + RELEASE_DEBOUNCE_MS + 1);
        assert_eq!(t.phase, Phase::Running);
        // Run then stop.
        t.tick(ARM_THRESHOLD_MS + 5000);
        assert!(t.display_ms() >= 4000);
        t.stop(ARM_THRESHOLD_MS + 5000);
        assert_eq!(t.phase, Phase::Stopped);
        assert_eq!(t.result_penalty(), Penalty::Ok);
        t.consume_result();
        assert_eq!(t.phase, Phase::Idle);
    }

    #[test]
    fn autorepeat_release_press_does_not_cancel() {
        // Initial press, then long delay before first autorepeat (initial
        // autorepeat delay) — during which a spurious release/press pair may
        // occur. The press must cancel the pending release so arming survives.
        let mut t = Timer::new();
        t.press(0);
        // A release arrives (autorepeat gap) ...
        t.release(30);
        // ... but a press arrives quickly after, cancelling it.
        t.press(45);
        // Ticks during the hold must not cancel arming.
        t.tick(50);
        t.tick(100);
        assert_ne!(
            t.phase,
            Phase::Idle,
            "autorepeat release/press must not cancel"
        );
    }

    #[test]
    fn real_release_while_arming_cancels() {
        let mut t = Timer::new();
        t.press(0);
        t.release(50); // released before reaching Ready
        t.tick(50 + RELEASE_DEBOUNCE_MS + 1);
        assert_eq!(t.phase, Phase::Idle);
        assert!(!t.has_result());
    }

    #[test]
    fn press_while_running_stops() {
        let mut t = Timer::new();
        t.press(0);
        t.tick(ARM_THRESHOLD_MS + 1); // Ready
        t.release(ARM_THRESHOLD_MS + 1);
        t.tick(ARM_THRESHOLD_MS + 1 + RELEASE_DEBOUNCE_MS + 1); // Running
        assert_eq!(t.phase, Phase::Running);
        t.press(ARM_THRESHOLD_MS + 5000);
        assert_eq!(t.phase, Phase::Stopped);
    }

    #[test]
    fn cancel_returns_to_idle_from_arming_ready_inspecting() {
        let mut t = Timer::new();
        t.press(0);
        t.cancel();
        assert_eq!(t.phase, Phase::Idle);
        t.press(100);
        t.tick(100 + ARM_THRESHOLD_MS + 1);
        assert_eq!(t.phase, Phase::Ready);
        t.cancel();
        assert_eq!(t.phase, Phase::Idle);
        // A stale release must not start anything.
        t.tick(10_000);
        assert_eq!(t.phase, Phase::Idle);

        let mut t = Timer::with_config(ENH_INSP);
        t.press(0);
        assert_eq!(t.phase, Phase::Inspecting);
        t.cancel();
        assert_eq!(t.phase, Phase::Idle);
        assert!(!t.has_result());
    }

    #[test]
    fn cancel_does_not_stop_running() {
        let mut t = Timer::with_config(FB);
        t.press(0);
        t.cancel();
        assert_eq!(t.phase, Phase::Running);
    }

    #[test]
    fn fallback_never_stays_ready_and_records_ten_seconds() {
        let mut t = Timer::with_config(FB);
        t.press(0);
        assert_eq!(t.phase, Phase::Running, "press starts immediately");
        // A release must be ignored and ticks must never enter Ready.
        t.release(50);
        t.tick(5_000);
        assert_eq!(t.phase, Phase::Running);
        t.press(10_000);
        assert_eq!(t.phase, Phase::Stopped);
        assert_eq!(t.result_ms(), 10_000);
        assert_eq!(t.result_penalty(), Penalty::Ok);
        t.consume_result();
        assert_eq!(t.phase, Phase::Idle);
    }

    #[test]
    fn inspect_then_solve_enhanced_has_no_penalty() {
        let mut t = Timer::with_config(ENH_INSP);
        t.press(0); // tap Space -> inspection begins
        assert_eq!(t.phase, Phase::Inspecting);
        t.tick(6_000);
        assert_eq!(t.phase, Phase::Inspecting);
        assert_eq!(t.inspection_seconds_left(), Some(9));
        let rel = hold_and_release(&mut t, 6_000);
        assert_eq!(t.phase, Phase::Running);
        t.tick(rel + 4_000);
        t.press(rel + 4_000);
        assert!(t.has_result());
        assert_eq!(t.result_ms(), 4_000);
        assert_eq!(t.result_penalty(), Penalty::Ok);
    }

    #[test]
    fn early_release_while_nested_arming_returns_to_inspecting() {
        let mut t = Timer::with_config(ENH_INSP);
        t.press(0);
        t.tick(1_000);
        t.press(1_000); // start nested arming
        assert_eq!(t.phase, Phase::Arming);
        t.release(1_100);
        t.tick(1_100 + RELEASE_DEBOUNCE_MS + 1);
        assert_eq!(t.phase, Phase::Inspecting);
        assert!(t.in_inspection());
        t.tick(2_000);
        assert_eq!(t.inspection_ms(), 2_000, "inspection kept counting");
    }

    #[test]
    fn inspection_cancel_with_esc_semantics() {
        let mut t = Timer::with_config(ENH_INSP);
        t.press(0);
        t.tick(3_000);
        t.cancel();
        assert_eq!(t.phase, Phase::Idle);
        t.tick(20_000);
        assert_eq!(t.phase, Phase::Idle, "no DNF after cancel");
    }

    #[test]
    fn late_start_at_16s_is_plus_two() {
        let mut t = Timer::with_config(ENH_INSP);
        t.press(0);
        t.tick(15_500);
        assert!(t.inspection_overtime());
        assert_eq!(t.inspection_seconds_left(), None);
        // Nested arm at 15.5 s, release at ~16 s.
        t.press(15_500);
        t.tick(15_500 + ARM_THRESHOLD_MS + 10);
        assert_eq!(t.phase, Phase::Ready);
        t.release(16_000);
        t.tick(16_000 + RELEASE_DEBOUNCE_MS + 1);
        assert_eq!(t.phase, Phase::Running);
        t.press(21_000);
        assert_eq!(t.result_ms(), 5_000);
        assert_eq!(t.result_penalty(), Penalty::PlusTwo);
    }

    #[test]
    fn start_at_exactly_15s_has_no_penalty_and_fallback_16s_is_plus_two() {
        let mut t = Timer::with_config(FB_INSP);
        t.press(0);
        assert_eq!(t.phase, Phase::Inspecting);
        t.tick(14_000);
        t.press(15_000);
        assert_eq!(t.phase, Phase::Running);
        t.press(18_000);
        assert_eq!(t.result_penalty(), Penalty::Ok);

        let mut t = Timer::with_config(FB_INSP);
        t.press(0);
        t.tick(16_000);
        t.press(16_000);
        t.press(19_000);
        assert_eq!(t.result_ms(), 3_000);
        assert_eq!(t.result_penalty(), Penalty::PlusTwo);
    }

    #[test]
    fn inspection_timeout_at_17s_is_dnf_with_zero_raw() {
        let mut t = Timer::with_config(ENH_INSP);
        t.press(0);
        t.tick(16_999);
        assert_eq!(t.phase, Phase::Inspecting);
        t.tick(17_000);
        assert!(t.has_result());
        assert_eq!(t.phase, Phase::Stopped);
        assert_eq!(t.result_ms(), 0);
        assert_eq!(t.result_penalty(), Penalty::Dnf);
        t.consume_result();
        assert_eq!(t.phase, Phase::Idle);
    }

    #[test]
    fn inspection_timeout_also_applies_in_fallback_and_while_holding() {
        let mut t = Timer::with_config(FB_INSP);
        t.press(0);
        t.tick(17_500);
        assert_eq!(t.result_penalty(), Penalty::Dnf);

        let mut t = Timer::with_config(ENH_INSP);
        t.press(0);
        t.press(16_000);
        t.tick(16_000 + ARM_THRESHOLD_MS + 1); // Ready
        t.tick(17_000);
        assert!(t.has_result());
        assert_eq!(t.result_penalty(), Penalty::Dnf);
    }

    #[test]
    fn inspection_cues_at_8s_and_12s() {
        let mut t = Timer::with_config(ENH_INSP);
        t.press(0);
        t.tick(7_999);
        assert!(!t.cue_8s() && !t.cue_12s());
        t.tick(8_000);
        assert!(t.cue_8s() && !t.cue_12s());
        t.tick(12_000);
        assert!(t.cue_8s() && t.cue_12s());
        t.cancel();
        assert!(!t.cue_8s());
    }

    #[test]
    fn inspection_countdown_seconds() {
        let mut t = Timer::with_config(ENH_INSP);
        t.press(0);
        assert_eq!(t.inspection_seconds_left(), Some(15));
        t.tick(1);
        assert_eq!(t.inspection_seconds_left(), Some(15));
        t.tick(1_000);
        assert_eq!(t.inspection_seconds_left(), Some(14));
        t.tick(14_999);
        assert_eq!(t.inspection_seconds_left(), Some(1));
        t.tick(15_000);
        assert_eq!(t.inspection_seconds_left(), Some(0));
    }

    #[test]
    fn format_ms_examples() {
        assert_eq!(format_ms(0), "0.00");
        assert_eq!(format_ms(5230), "5.23");
        assert_eq!(format_ms(12340), "12.34");
        assert_eq!(format_ms(65000), "1:05.00");
        assert_eq!(format_ms(125990), "2:05.99");
    }
}
