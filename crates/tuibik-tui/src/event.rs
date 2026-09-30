//! Translate raw crossterm events into high-level [`Input`]s.
//!
//! Classification depends on a [`KeyContext`]: while the timer is active every
//! key except Space/Esc/Ctrl+C is just "some key" (it stops a running solve),
//! and while typing a name printable keys become characters.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use store::Penalty;

/// A classified input event that the app reduces into state changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// `q`: quit (only honoured on the idle dashboard).
    Quit,
    /// `Ctrl+C`: quit from any state.
    ForceQuit,
    /// `Esc`: close / cancel.
    Cancel,
    /// `Enter`: confirm, or open the selected solve on the dashboard.
    Confirm,
    /// `n`: new scramble (doubles as "no" in confirmation dialogs).
    NewScramble,
    /// The hold key (Space) was pressed.
    HoldPress,
    /// The hold key (Space) auto-repeated while held (not a fresh press).
    HoldRepeat,
    /// The hold key (Space) was released.
    HoldRelease,
    /// Some other key while timing (stops a running solve).
    OtherKey,
    Up,
    Down,
    Left,
    Right,
    /// `Home` / `g`: newest solve.
    JumpTop,
    /// `End` / `G`: oldest solve.
    JumpBottom,
    /// `d` / `Delete`.
    Delete,
    /// `y`.
    Yes,
    /// `1` / `2` / `3`.
    Penalty(Penalty),
    OpenSessions,
    /// `r`: rename (sessions overlay).
    Rename,
    TogglePreview,
    ToggleTheme,
    /// `?`.
    Help,
    /// `o`.
    Settings,
    /// A printable character while typing text.
    Char(char),
    /// Backspace while typing text.
    Backspace,
    /// Ignored / no-op.
    None,
}

/// What the keyboard is currently used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    /// Dashboard and overlays.
    Normal,
    /// The timer is arming, ready, inspecting or running.
    Timing,
    /// Typing a session name.
    TextEntry,
}

/// Context for [`classify_key`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyContext {
    /// Whether the terminal reports key-release events (Kitty protocol).
    pub enhanced: bool,
    pub mode: KeyMode,
}

#[cfg(test)]
impl KeyContext {
    pub fn normal(enhanced: bool) -> Self {
        KeyContext {
            enhanced,
            mode: KeyMode::Normal,
        }
    }
}

/// Map a key event to an [`Input`].
pub fn classify_key(key: KeyEvent, ctx: KeyContext) -> Input {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.kind {
        KeyEventKind::Press | KeyEventKind::Repeat => {
            let is_repeat = key.kind == KeyEventKind::Repeat;
            if ctrl && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('C')) {
                return Input::ForceQuit;
            }
            match ctx.mode {
                KeyMode::Timing => match key.code {
                    KeyCode::Esc => Input::Cancel,
                    KeyCode::Char(' ') => hold(is_repeat),
                    _ => Input::OtherKey,
                },
                KeyMode::TextEntry => match key.code {
                    KeyCode::Esc => Input::Cancel,
                    KeyCode::Enter => Input::Confirm,
                    KeyCode::Backspace => Input::Backspace,
                    KeyCode::Char(c) if !ctrl && !c.is_control() => Input::Char(c),
                    _ => Input::None,
                },
                KeyMode::Normal => normal_key(key.code, is_repeat),
            }
        }
        KeyEventKind::Release => {
            if ctx.enhanced && ctx.mode != KeyMode::TextEntry && key.code == KeyCode::Char(' ') {
                Input::HoldRelease
            } else {
                Input::None
            }
        }
    }
}

fn hold(is_repeat: bool) -> Input {
    if is_repeat {
        Input::HoldRepeat
    } else {
        Input::HoldPress
    }
}

fn normal_key(code: KeyCode, is_repeat: bool) -> Input {
    match code {
        KeyCode::Esc => Input::Cancel,
        KeyCode::Enter => Input::Confirm,
        KeyCode::Char(' ') => hold(is_repeat),
        KeyCode::Char('q') => Input::Quit,
        KeyCode::Char('n') => Input::NewScramble,
        KeyCode::Char('s') => Input::OpenSessions,
        KeyCode::Char('r') => Input::Rename,
        KeyCode::Char('p') => Input::TogglePreview,
        KeyCode::Char('t') => Input::ToggleTheme,
        KeyCode::Char('o') => Input::Settings,
        KeyCode::Char('?') => Input::Help,
        KeyCode::Char('y') => Input::Yes,
        KeyCode::Char('1') => Input::Penalty(Penalty::Ok),
        KeyCode::Char('2') => Input::Penalty(Penalty::PlusTwo),
        KeyCode::Char('3') => Input::Penalty(Penalty::Dnf),
        KeyCode::Left | KeyCode::Char('h') => Input::Left,
        KeyCode::Right | KeyCode::Char('l') => Input::Right,
        KeyCode::Up | KeyCode::Char('k') => Input::Up,
        KeyCode::Down | KeyCode::Char('j') => Input::Down,
        KeyCode::Home | KeyCode::Char('g') => Input::JumpTop,
        KeyCode::End | KeyCode::Char('G') => Input::JumpBottom,
        KeyCode::Char('d') | KeyCode::Delete => Input::Delete,
        _ => Input::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEventState;

    fn ev_mod(code: KeyCode, kind: KeyEventKind, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind,
            state: KeyEventState::NONE,
        }
    }

    fn ev(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        ev_mod(code, kind, KeyModifiers::NONE)
    }

    fn press(code: KeyCode) -> KeyEvent {
        ev(code, KeyEventKind::Press)
    }

    fn norm() -> KeyContext {
        KeyContext::normal(true)
    }

    fn timing() -> KeyContext {
        KeyContext {
            enhanced: true,
            mode: KeyMode::Timing,
        }
    }

    fn text() -> KeyContext {
        KeyContext {
            enhanced: true,
            mode: KeyMode::TextEntry,
        }
    }

    #[test]
    fn quit_and_scramble() {
        assert_eq!(classify_key(press(KeyCode::Char('q')), norm()), Input::Quit);
        assert_eq!(
            classify_key(press(KeyCode::Char('n')), norm()),
            Input::NewScramble
        );
    }

    #[test]
    fn esc_is_cancel_not_quit() {
        assert_eq!(classify_key(press(KeyCode::Esc), norm()), Input::Cancel);
        assert_eq!(classify_key(press(KeyCode::Esc), timing()), Input::Cancel);
        assert_eq!(classify_key(press(KeyCode::Esc), text()), Input::Cancel);
    }

    #[test]
    fn ctrl_c_force_quits_in_every_context() {
        for ctx in [norm(), timing(), text()] {
            let k = ev_mod(
                KeyCode::Char('c'),
                KeyEventKind::Press,
                KeyModifiers::CONTROL,
            );
            assert_eq!(classify_key(k, ctx), Input::ForceQuit);
        }
    }

    #[test]
    fn enter_confirms_and_no_longer_holds() {
        assert_eq!(classify_key(press(KeyCode::Enter), norm()), Input::Confirm);
        assert_eq!(classify_key(press(KeyCode::Enter), text()), Input::Confirm);
        // While timing, Enter is just "some key".
        assert_eq!(
            classify_key(press(KeyCode::Enter), timing()),
            Input::OtherKey
        );
    }

    #[test]
    fn space_press_is_hold() {
        assert_eq!(
            classify_key(press(KeyCode::Char(' ')), norm()),
            Input::HoldPress
        );
        assert_eq!(
            classify_key(press(KeyCode::Char(' ')), timing()),
            Input::HoldPress
        );
    }

    #[test]
    fn space_repeat_is_hold_repeat() {
        assert_eq!(
            classify_key(ev(KeyCode::Char(' '), KeyEventKind::Repeat), norm()),
            Input::HoldRepeat
        );
    }

    #[test]
    fn space_release_only_when_enhanced() {
        let rel = ev(KeyCode::Char(' '), KeyEventKind::Release);
        assert_eq!(classify_key(rel, norm()), Input::HoldRelease);
        assert_eq!(classify_key(rel, KeyContext::normal(false)), Input::None);
        assert_eq!(classify_key(rel, text()), Input::None);
    }

    #[test]
    fn other_keys_while_timing_are_otherkey() {
        for c in ['z', 'n', 'q', 'd', 's', '?', '1'] {
            assert_eq!(
                classify_key(press(KeyCode::Char(c)), timing()),
                Input::OtherKey,
                "{c}"
            );
        }
        assert_eq!(classify_key(press(KeyCode::Up), timing()), Input::OtherKey);
    }

    #[test]
    fn navigation_keys() {
        assert_eq!(classify_key(press(KeyCode::Up), norm()), Input::Up);
        assert_eq!(classify_key(press(KeyCode::Char('k')), norm()), Input::Up);
        assert_eq!(classify_key(press(KeyCode::Down), norm()), Input::Down);
        assert_eq!(classify_key(press(KeyCode::Char('j')), norm()), Input::Down);
        assert_eq!(classify_key(press(KeyCode::Left), norm()), Input::Left);
        assert_eq!(
            classify_key(press(KeyCode::Char('l')), norm()),
            Input::Right
        );
        assert_eq!(classify_key(press(KeyCode::Home), norm()), Input::JumpTop);
        assert_eq!(
            classify_key(press(KeyCode::Char('g')), norm()),
            Input::JumpTop
        );
        assert_eq!(classify_key(press(KeyCode::End), norm()), Input::JumpBottom);
        assert_eq!(
            classify_key(press(KeyCode::Char('G')), norm()),
            Input::JumpBottom
        );
    }

    #[test]
    fn dashboard_action_keys() {
        assert_eq!(
            classify_key(press(KeyCode::Char('d')), norm()),
            Input::Delete
        );
        assert_eq!(classify_key(press(KeyCode::Delete), norm()), Input::Delete);
        assert_eq!(classify_key(press(KeyCode::Char('?')), norm()), Input::Help);
        assert_eq!(
            classify_key(press(KeyCode::Char('o')), norm()),
            Input::Settings
        );
        assert_eq!(classify_key(press(KeyCode::Char('y')), norm()), Input::Yes);
        assert_eq!(
            classify_key(press(KeyCode::Char('2')), norm()),
            Input::Penalty(Penalty::PlusTwo)
        );
        assert_eq!(
            classify_key(press(KeyCode::Char('3')), norm()),
            Input::Penalty(Penalty::Dnf)
        );
        assert_eq!(
            classify_key(press(KeyCode::Char('1')), norm()),
            Input::Penalty(Penalty::Ok)
        );
    }

    #[test]
    fn text_entry_turns_printables_into_chars() {
        assert_eq!(
            classify_key(press(KeyCode::Char('q')), text()),
            Input::Char('q')
        );
        assert_eq!(
            classify_key(press(KeyCode::Char(' ')), text()),
            Input::Char(' ')
        );
        assert_eq!(
            classify_key(press(KeyCode::Char('D')), text()),
            Input::Char('D')
        );
        assert_eq!(
            classify_key(press(KeyCode::Backspace), text()),
            Input::Backspace
        );
        assert_eq!(classify_key(press(KeyCode::Up), text()), Input::None);
    }
}
